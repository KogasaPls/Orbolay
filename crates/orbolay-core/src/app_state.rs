use std::{
  cmp::Reverse,
  collections::{BinaryHeap, HashMap},
  sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
  time::{Duration, Instant},
};

use crate::{
  config::Config,
  protocol::{BridgeCommand, BridgeEvent, MemberState, Notification, SoundboardSoundPayload},
  user::{Activity, Speech, User},
};

/// How long a notification stays on screen.
pub const NOTIFICATION_TIMEOUT: Duration = Duration::from_secs(5);
/// How many notifications are shown at once.
pub const MAX_NOTIFICATIONS: usize = 3;

/// Soundboard sounds grouped by guild ID (`None` for Discord's default sounds), with guild
/// display names.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Soundboard {
  pub sounds: HashMap<Option<String>, Vec<SoundboardSoundPayload>>,
  pub guild_names: HashMap<String, String>,
}

/// A notification on screen and when it disappears.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
  pub notification: Notification,
  pub expires_at: Instant,
}

/// The voice channel the user is in.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
  pub channel_id: String,
  /// `None` for direct message and group calls.
  pub guild_id: Option<String>,
  /// Sorted by user ID, so the overlay lists members in a stable order.
  pub members: Vec<User>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppState {
  pub config: Config,
  /// The Discord user running the bridge, known once it registers.
  pub user_id: Option<String>,
  /// Nitro lets the soundboard play sounds from other servers.
  pub has_nitro: bool,
  pub call: Option<Call>,
  pub is_censor: bool,
  pub messages: Vec<Shown>,
  pub soundboard: Arc<Soundboard>,
}

/// Follow-up work a state transition schedules: [`AppState::fire`] runs it at `at`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timer {
  pub at: Instant,
  pub kind: TimerKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimerKind {
  /// End the grace period of a member's activity that was set to expire at the timer's `at`.
  GraceEnd {
    user_id: String,
    activity: Activity,
  },
  ExpireMessages,
}

impl AppState {
  pub fn new(config: Config) -> Self {
    Self {
      config,
      ..Self::default()
    }
  }

  /// Members of the current call sorted by user ID, empty outside a call.
  pub fn members(&self) -> &[User] {
    self.call.as_ref().map_or(&[], |call| &call.members)
  }

  /// The user running the bridge, while they are in a call.
  pub fn me(&self) -> Option<&User> {
    let user_id = self.user_id.as_deref()?;
    self.members().iter().find(|user| user.id == user_id)
  }

  /// Applies a bridge event and returns the timer it needs, if any.
  pub fn apply(&mut self, event: BridgeEvent, now: Instant) -> Option<Timer> {
    match event {
      BridgeEvent::RegisterConfig {
        user_id,
        premium_type,
      } => {
        self.user_id = Some(user_id);
        if let Some(premium_type) = premium_type {
          self.has_nitro = premium_type != 0;
        }
      }
      BridgeEvent::ChannelJoined {
        states,
        guild_id,
        channel_id,
      } => {
        let mut members: Vec<_> = states.into_iter().map(User::joined).collect();
        members.sort_by(|a, b| a.id.cmp(&b.id));
        self.call = Some(Call {
          channel_id,
          guild_id,
          members,
        });
      }
      BridgeEvent::VoiceStateUpdate { state } => self.update_member(state),
      BridgeEvent::Speaking { user_id, speaking } => {
        return self.set_active(&user_id, Activity::Speech, speaking, now);
      }
      BridgeEvent::SoundboardPlaying { user_id, playing } => {
        return self.set_active(&user_id, Activity::Sound, playing, now);
      }
      BridgeEvent::ChannelLeft => self.call = None,
      BridgeEvent::MessageNotification { mut message } => {
        message.icon = message.icon.replace(".webp", ".png");
        return Some(self.notify(message, now));
      }
      BridgeEvent::StreamerMode { enabled } => self.is_censor = enabled,
      BridgeEvent::SoundboardUpdate {
        sounds,
        guild_names,
      } => {
        let mut by_guild: HashMap<Option<String>, Vec<_>> = HashMap::new();
        for sound in sounds {
          by_guild
            .entry(sound.guild_id.clone())
            .or_default()
            .push(sound);
        }
        let soundboard = Arc::make_mut(&mut self.soundboard);
        soundboard.sounds = by_guild;
        soundboard.guild_names = guild_names;
      }
    }
    None
  }

  fn update_member(&mut self, state: MemberState) {
    let Some(call) = &mut self.call else { return };
    if state.channel_id.as_ref() != Some(&call.channel_id) {
      call.members.retain(|user| user.id != state.user_id);
      return;
    }
    match call
      .members
      .binary_search_by(|user| user.id.as_str().cmp(&state.user_id))
    {
      Ok(at) => call.members[at].update(state),
      Err(at) => call.members.insert(at, User::joined(state)),
    }
  }

  fn member_mut(&mut self, user_id: &str) -> Option<&mut User> {
    let call = self.call.as_mut()?;
    call.members.iter_mut().find(|user| user.id == user_id)
  }

  /// Starts or stops a member's activity. Stopping keeps it shown for the grace period, ended by
  /// the returned timer.
  fn set_active(
    &mut self,
    user_id: &str,
    activity: Activity,
    active: bool,
    now: Instant,
  ) -> Option<Timer> {
    let grace = Duration::from_millis(u64::from(self.config.speaking_grace_ms));
    let user = self.member_mut(user_id)?;
    let state = user.activity_mut(activity);
    match (active, *state) {
      (true, _) => {
        *state = Speech::Speaking;
        None
      }
      (false, Speech::Speaking) if grace.is_zero() => {
        *state = Speech::Silent;
        None
      }
      (false, Speech::Speaking) => {
        let until = now + grace;
        *state = Speech::Grace { until };
        Some(Timer {
          at: until,
          kind: TimerKind::GraceEnd {
            user_id: user.id.clone(),
            activity,
          },
        })
      }
      (false, _) => None,
    }
  }

  /// Shows a notification, dropping the oldest beyond [`MAX_NOTIFICATIONS`].
  pub fn notify(&mut self, notification: Notification, now: Instant) -> Timer {
    let at = now + NOTIFICATION_TIMEOUT;
    self.messages.push(Shown {
      notification,
      expires_at: at,
    });
    let excess = self.messages.len().saturating_sub(MAX_NOTIFICATIONS);
    self.messages.drain(..excess);
    Timer {
      at,
      kind: TimerKind::ExpireMessages,
    }
  }

  /// Applies a due timer and reports whether anything visible changed.
  pub fn fire(&mut self, timer: &Timer, now: Instant) -> bool {
    match &timer.kind {
      TimerKind::GraceEnd { user_id, activity } => {
        let Some(state) = self
          .member_mut(user_id)
          .map(|user| user.activity_mut(*activity))
          .filter(|state| **state == Speech::Grace { until: timer.at })
        else {
          return false;
        };
        *state = Speech::Silent;
        true
      }
      TimerKind::ExpireMessages => {
        let before = self.messages.len();
        self.messages.retain(|shown| shown.expires_at > now);
        before != self.messages.len()
      }
    }
  }
}

/// Owns the shared state. Background threads change it through [`AppHandle::apply`],
/// [`AppHandle::notify`], and [`AppHandle::set_config`]; each change signals `redraw_tx`, which the UI drains before taking one
/// snapshot, so a burst of updates costs a single render.
#[derive(Clone)]
pub struct AppHandle {
  shared: Arc<RwLock<AppState>>,
  redraw_tx: flume::Sender<()>,
  timer_tx: flume::Sender<Timer>,
  command_tx: flume::Sender<BridgeCommand>,
  recording_keybind: Arc<AtomicBool>,
  config_subscribers: Arc<Mutex<Vec<flume::Sender<Config>>>>,
}

impl AppHandle {
  /// Creates the handle and its timer thread. Returns the receiver for commands to the bridge.
  pub fn new(
    state: AppState,
    redraw_tx: flume::Sender<()>,
  ) -> (Self, flume::Receiver<BridgeCommand>) {
    let (timer_tx, timer_rx) = flume::unbounded();
    let (command_tx, command_rx) = flume::unbounded();
    let app = Self {
      shared: Arc::new(RwLock::new(state)),
      redraw_tx,
      timer_tx,
      command_tx,
      recording_keybind: Arc::new(AtomicBool::new(false)),
      config_subscribers: Arc::default(),
    };

    let timer_app = app.clone();
    std::thread::Builder::new()
      .name("orbolay-timers".into())
      .spawn(move || timer_app.run_timers(&timer_rx))
      .expect("failed to spawn the timer thread");

    (app, command_rx)
  }

  /// Sleeps until the earliest pending timer or the next new one, firing timers as they come due.
  fn run_timers(&self, timer_rx: &flume::Receiver<Timer>) {
    let mut pending: BinaryHeap<Reverse<Timer>> = BinaryHeap::new();
    loop {
      let received = match pending.peek() {
        Some(Reverse(next)) => timer_rx.recv_deadline(next.at),
        None => timer_rx
          .recv()
          .map_err(|_| flume::RecvTimeoutError::Disconnected),
      };
      match received {
        Ok(timer) => pending.push(Reverse(timer)),
        Err(flume::RecvTimeoutError::Timeout) => {}
        Err(flume::RecvTimeoutError::Disconnected) => return,
      }

      let now = Instant::now();
      while pending.peek().is_some_and(|Reverse(timer)| timer.at <= now) {
        if let Some(Reverse(timer)) = pending.pop() {
          self.update_if_changed(|state| state.fire(&timer, now));
        }
      }
    }
  }

  pub fn recording_keybind(&self) -> &Arc<AtomicBool> {
    &self.recording_keybind
  }

  pub fn read<T>(&self, f: impl FnOnce(&AppState) -> T) -> T {
    let state = self.shared.read().unwrap();
    f(&state)
  }

  fn update<T>(&self, f: impl FnOnce(&mut AppState) -> T) -> T {
    let result = {
      let mut state = self.shared.write().unwrap();
      f(&mut state)
    };
    self.redraw();
    result
  }

  fn update_if_changed(&self, f: impl FnOnce(&mut AppState) -> bool) {
    let changed = {
      let mut state = self.shared.write().unwrap();
      f(&mut state)
    };

    if changed {
      self.redraw();
    }
  }

  /// Changes the config and, if it changed, sends the new config to every subscriber.
  pub fn set_config(&self, change: impl FnOnce(&mut Config)) -> Config {
    let (config, changed) = {
      let mut state = self.shared.write().unwrap();
      let before = state.config.clone();
      change(&mut state.config);
      state.config = std::mem::take(&mut state.config).normalized();
      (state.config.clone(), state.config != before)
    };
    if changed {
      self.redraw();
      self
        .config_subscribers
        .lock()
        .unwrap()
        .retain(|subscriber| subscriber.send(config.clone()).is_ok());
    }
    config
  }

  /// Receives every config change made through [`AppHandle::set_config`].
  pub fn subscribe_config(&self) -> flume::Receiver<Config> {
    let (tx, rx) = flume::unbounded();
    self.config_subscribers.lock().unwrap().push(tx);
    rx
  }

  pub fn snapshot(&self) -> AppState {
    self.read(Clone::clone)
  }

  /// Applies a bridge event and schedules the timer it returns.
  pub fn apply(&self, event: BridgeEvent) {
    if let Some(timer) = self.update(|state| state.apply(event, Instant::now())) {
      let _ = self.timer_tx.send(timer);
    }
  }

  pub fn notify(&self, notification: Notification) {
    let timer = self.update(|state| state.notify(notification, Instant::now()));
    let _ = self.timer_tx.send(timer);
  }

  pub fn send(&self, command: BridgeCommand) {
    let _ = self.command_tx.send(command);
  }

  /// Asks Discord to play a member at `volume` and shows it right away, since Discord does not
  /// report local volume changes back.
  pub fn set_user_volume(&self, user_id: String, volume: f32) {
    self.update_if_changed(|state| {
      state
        .member_mut(&user_id)
        .map(|user| user.set_volume(volume))
        .is_some()
    });
    self.send(BridgeCommand::SetUserVolume {
      user_id,
      volume: f64::from(volume),
    });
  }

  /// Requests a redraw; a full channel already has one pending.
  fn redraw(&self) {
    let _ = self.redraw_tx.try_send(());
  }
}

impl PartialEq for AppHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.shared, &other.shared)
  }
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, Instant};

  use super::{AppState, MAX_NOTIFICATIONS, Timer, TimerKind};
  use crate::{
    config::Config,
    protocol::{BridgeCommand, BridgeEvent, MemberState, Notification, SoundboardSoundPayload},
    user::{Activity, Speech, User, UserVoiceState},
  };

  const GRACE: Duration = Duration::from_millis(300);

  fn member(user_id: &str) -> MemberState {
    MemberState {
      user_id: user_id.into(),
      username: Some(format!("user {user_id}")),
      channel_id: Some("voice".into()),
      ..MemberState::default()
    }
  }

  fn update(state: MemberState) -> BridgeEvent {
    BridgeEvent::VoiceStateUpdate { state }
  }

  fn speaking(user_id: &str, speaking: bool) -> BridgeEvent {
    BridgeEvent::Speaking {
      user_id: user_id.into(),
      speaking,
    }
  }

  fn joined() -> AppState {
    let mut state = AppState::new(Config::default());
    let now = Instant::now();
    state.apply(
      BridgeEvent::RegisterConfig {
        user_id: "me".into(),
        premium_type: None,
      },
      now,
    );
    state.apply(
      BridgeEvent::ChannelJoined {
        states: vec![member("me"), member("peer")],
        guild_id: Some("guild".into()),
        channel_id: "voice".into(),
      },
      now,
    );
    state
  }

  fn voice_state(state: &AppState, user_id: &str) -> Option<UserVoiceState> {
    state
      .members()
      .iter()
      .find(|u| u.id == user_id)
      .map(User::voice_state)
  }

  fn grace_end(at: Instant) -> Timer {
    Timer {
      at,
      kind: TimerKind::GraceEnd {
        user_id: "peer".into(),
        activity: Activity::Speech,
      },
    }
  }

  #[test]
  fn joining_records_the_channel_guild_and_members() {
    let state = joined();
    let call = state.call.as_ref().unwrap();
    assert_eq!(call.channel_id, "voice");
    assert_eq!(call.guild_id.as_deref(), Some("guild"));
    assert_eq!(call.members.len(), 2);
    assert_eq!(state.me().map(|me| me.id.as_str()), Some("me"));
  }

  #[test]
  fn the_joined_channel_does_not_depend_on_listing_yourself() {
    let mut state = AppState::default();
    let now = Instant::now();
    state.apply(
      BridgeEvent::ChannelJoined {
        states: vec![member("peer")],
        guild_id: None,
        channel_id: "voice".into(),
      },
      now,
    );
    state.apply(update(member("peer")), now);
    assert!(voice_state(&state, "peer").is_some());
  }

  #[test]
  fn stopping_speaking_keeps_the_speaker_visible_until_the_grace_period_ends() {
    let mut state = joined();
    let now = Instant::now();
    assert_eq!(state.apply(speaking("peer", true), now), None);
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Speaking));

    let effects = state.apply(speaking("peer", false), now);
    assert_eq!(effects, Some(grace_end(now + GRACE)));
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Speaking));

    assert!(state.fire(&grace_end(now + GRACE), now + GRACE));
    assert_eq!(
      voice_state(&state, "peer"),
      Some(UserVoiceState::NotSpeaking)
    );
  }

  #[test]
  fn speaking_again_cancels_the_pending_grace_end() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(speaking("peer", true), now);
    state.apply(speaking("peer", false), now);
    state.apply(speaking("peer", true), now + GRACE / 2);

    assert!(!state.fire(&grace_end(now + GRACE), now + GRACE));
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Speaking));
  }

  #[test]
  fn repeated_silence_does_not_restart_the_grace_period() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(speaking("peer", true), now);
    assert!(state.apply(speaking("peer", false), now).is_some());
    assert_eq!(state.apply(speaking("peer", false), now + GRACE / 2), None);
  }

  #[test]
  fn a_zero_grace_period_stops_speaking_immediately() {
    let mut state = joined();
    state.config.speaking_grace_ms = 0;
    let now = Instant::now();
    state.apply(speaking("peer", true), now);
    assert_eq!(state.apply(speaking("peer", false), now), None);
    assert_eq!(
      voice_state(&state, "peer"),
      Some(UserVoiceState::NotSpeaking)
    );
  }

  #[test]
  fn muting_overrides_speaking_without_a_grace_period() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(speaking("peer", true), now);
    let effects = state.apply(
      update(MemberState {
        mute: true,
        ..member("peer")
      }),
      now,
    );
    assert_eq!(effects, None);
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Muted));
  }

  #[test]
  fn silence_after_muting_keeps_the_mute() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(speaking("peer", true), now);
    state.apply(
      update(MemberState {
        mute: true,
        ..member("peer")
      }),
      now,
    );
    state.apply(speaking("peer", false), now);
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Muted));
    state.fire(&grace_end(now + GRACE), now + GRACE);

    state.apply(
      update(MemberState {
        mute: false,
        ..member("peer")
      }),
      now,
    );
    assert_eq!(
      voice_state(&state, "peer"),
      Some(UserVoiceState::NotSpeaking)
    );
  }

  #[test]
  fn full_updates_do_not_change_speaking() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(speaking("peer", true), now);
    state.apply(update(member("peer")), now);
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Speaking));
  }

  #[test]
  fn leaving_or_moving_away_removes_the_member() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(
      update(MemberState {
        channel_id: None,
        ..member("peer")
      }),
      now,
    );
    assert_eq!(voice_state(&state, "peer"), None);

    state.apply(update(member("new")), now);
    assert!(voice_state(&state, "new").is_some());
    state.apply(
      update(MemberState {
        channel_id: Some("elsewhere".into()),
        ..member("new")
      }),
      now,
    );
    assert_eq!(voice_state(&state, "new"), None);
  }

  #[test]
  fn full_updates_carry_the_camera() {
    let mut state = joined();
    state.apply(
      update(MemberState {
        camera: true,
        ..member("peer")
      }),
      Instant::now(),
    );
    assert!(state.members().iter().any(|u| u.id == "peer" && u.camera));
  }

  #[test]
  fn full_updates_carry_the_local_volume() {
    let mut state = joined();
    state.apply(
      update(MemberState {
        volume: Some(150.),
        ..member("peer")
      }),
      Instant::now(),
    );
    let peer = state.members().iter().find(|u| u.id == "peer").unwrap();
    assert!((peer.volume - 150.).abs() < f32::EPSILON);
  }

  #[test]
  fn soundboard_playback_lingers_like_speech() {
    let mut state = joined();
    let now = Instant::now();
    let playing = |user_id: &str, playing| BridgeEvent::SoundboardPlaying {
      user_id: user_id.into(),
      playing,
    };
    let sound = |state: &AppState| {
      state
        .members()
        .iter()
        .find(|u| u.id == "peer")
        .unwrap()
        .sound
    };
    let end = Timer {
      at: now + GRACE,
      kind: TimerKind::GraceEnd {
        user_id: "peer".into(),
        activity: Activity::Sound,
      },
    };

    assert_eq!(state.apply(playing("peer", true), now), None);
    assert_eq!(sound(&state), Speech::Speaking);
    assert_eq!(state.apply(playing("peer", false), now), Some(end.clone()));
    assert_eq!(sound(&state), Speech::Grace { until: now + GRACE });
    assert!(
      !state.fire(&grace_end(now + GRACE), now + GRACE),
      "speech timers leave sounds alone"
    );
    assert!(state.fire(&end, now + GRACE));
    assert_eq!(sound(&state), Speech::Silent);
    assert_eq!(state.apply(playing("stranger", true), now), None);
  }

  #[test]
  fn members_stay_sorted_by_id() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(update(member("alpha")), now);
    state.apply(update(member("zulu")), now);
    let ids: Vec<_> = state.members().iter().map(|u| u.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "me", "peer", "zulu"]);
  }

  #[test]
  fn speaking_updates_for_unknown_users_are_ignored() {
    let mut state = joined();
    state.apply(speaking("stranger", true), Instant::now());
    assert_eq!(voice_state(&state, "stranger"), None);
  }

  #[test]
  fn notifications_are_capped_and_expire() {
    let mut state = AppState::default();
    let now = Instant::now();
    for i in 0..=MAX_NOTIFICATIONS {
      state.notify(
        Notification {
          title: i.to_string(),
          ..Notification::default()
        },
        now + Duration::from_secs(u64::try_from(i).unwrap()),
      );
    }
    let titles = |state: &AppState| -> Vec<String> {
      state
        .messages
        .iter()
        .map(|shown| shown.notification.title.clone())
        .collect()
    };
    assert_eq!(titles(&state), ["1", "2", "3"]);

    let expire = Timer {
      at: now,
      kind: TimerKind::ExpireMessages,
    };
    assert!(state.fire(&expire, now + Duration::from_secs(6)));
    assert_eq!(titles(&state), ["2", "3"]);
  }

  #[test]
  fn leaving_the_channel_clears_members() {
    let mut state = joined();
    state.apply(BridgeEvent::ChannelLeft, Instant::now());
    assert_eq!(state.call, None);
    assert_eq!(state.members(), []);
  }

  #[test]
  fn volume_changes_show_before_discord_confirms_them() {
    let (redraw_tx, _redraw_rx) = flume::bounded(1);
    let (app, commands) = super::AppHandle::new(joined(), redraw_tx);

    app.set_user_volume("peer".into(), 150.);

    let volume = app.read(|state| {
      state
        .members()
        .iter()
        .find(|u| u.id == "peer")
        .unwrap()
        .volume
    });
    assert!((volume - 150.).abs() < f32::EPSILON);
    assert_eq!(
      commands.try_recv(),
      Ok(BridgeCommand::SetUserVolume {
        user_id: "peer".into(),
        volume: 150.
      })
    );
  }

  #[test]
  fn config_subscribers_only_hear_real_changes() {
    let (redraw_tx, _redraw_rx) = flume::bounded(1);
    let (app, _commands) = super::AppHandle::new(AppState::default(), redraw_tx);
    let changes = app.subscribe_config();

    app.set_config(|config| config.port = 7000);
    app.set_config(|config| config.port = 7000);

    assert_eq!(changes.try_recv().map(|c| c.port), Ok(7000));
    assert!(changes.try_recv().is_err());
  }

  #[test]
  fn each_soundboard_update_replaces_the_last() {
    let sound = |id: &str, guild: Option<&str>| SoundboardSoundPayload {
      sound_id: id.into(),
      name: id.into(),
      volume: 1.,
      emoji_id: None,
      emoji_name: None,
      guild_id: guild.map(Into::into),
      available: true,
    };
    let mut state = AppState::default();
    let now = Instant::now();
    state.apply(
      BridgeEvent::SoundboardUpdate {
        sounds: vec![sound("a", None), sound("b", Some("g"))],
        guild_names: [("g".to_string(), "Guild".to_string())].into(),
      },
      now,
    );
    state.apply(
      BridgeEvent::SoundboardUpdate {
        sounds: vec![sound("c", Some("g"))],
        guild_names: std::collections::HashMap::new(),
      },
      now,
    );
    assert_eq!(state.soundboard.sounds.len(), 1);
    assert_eq!(state.soundboard.sounds[&Some("g".into())][0].sound_id, "c");
    assert!(state.soundboard.guild_names.is_empty());
  }
}
