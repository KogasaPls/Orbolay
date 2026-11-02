use std::{
  cmp::Reverse,
  collections::{BinaryHeap, HashMap},
  sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
  time::{Duration, Instant},
};

use crate::{
  config::Config,
  payloads::{Notification, SoundboardSoundPayload},
  protocol::{BridgeCommand, BridgeEvent, VoiceStatePayload},
  user::User,
};

/// How long a notification stays on screen.
pub const NOTIFICATION_TIMEOUT: Duration = Duration::from_secs(5);
/// How many notifications are shown at once.
pub const MAX_NOTIFICATIONS: usize = 3;

/// Thread-safe shared state for background threads.
pub type SharedAppState = Arc<RwLock<AppState>>;

/// Soundboard sounds grouped by guild ID (`""` for default sounds), with guild display names.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Soundboard {
  pub sounds: HashMap<String, Vec<SoundboardSoundPayload>>,
  pub guild_names: HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct AppState {
  pub config: Config,
  pub user_id: String,
  /// Nitro lets the soundboard play sounds from other servers.
  pub has_nitro: bool,
  pub current_channel: String,
  pub current_guild_id: String,
  pub is_censor: bool,
  pub voice_users: Vec<User>,
  pub messages: Vec<Notification>,
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
  /// End the speaking grace period that was set to expire at the timer's `at`.
  GraceEnd {
    user_id: String,
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

  /// Applies a bridge event and returns the timer it needs, if any.
  pub fn apply(&mut self, event: BridgeEvent, now: Instant) -> Option<Timer> {
    match event {
      BridgeEvent::RegisterConfig {
        user_id,
        premium_type,
      } => {
        self.user_id = user_id;
        if let Some(premium_type) = premium_type {
          self.has_nitro = premium_type != 0;
        }
      }
      BridgeEvent::ChannelJoined {
        states,
        guild_id,
        channel_id,
      } => {
        let own_channel = || {
          states
            .iter()
            .find(|state| state.user_id == self.user_id)
            .and_then(|own| own.channel_id.clone().flatten())
        };
        self.current_channel = channel_id.or_else(own_channel).unwrap_or_default();
        self.current_guild_id = guild_id.unwrap_or_default();
        self.voice_users = states.into_iter().map(User::from).collect();
      }
      BridgeEvent::VoiceStateUpdate { state } => {
        return self.update_voice_state(state, now);
      }
      BridgeEvent::ChannelLeft => {
        self.voice_users.clear();
        self.current_channel.clear();
        self.current_guild_id.clear();
      }
      BridgeEvent::MessageNotification { mut message } => {
        message.icon = message.icon.replace(".webp", ".png");
        return Some(self.notify(message, now));
      }
      BridgeEvent::StreamerMode { enabled } => self.is_censor = enabled,
      BridgeEvent::SoundboardUpdate {
        sounds,
        guild_names,
      } => {
        let mut by_guild: HashMap<String, Vec<_>> = HashMap::new();
        for sound in sounds {
          by_guild
            .entry(sound.guild_id.clone().unwrap_or_default())
            .or_default()
            .push(sound);
        }
        let soundboard = Arc::make_mut(&mut self.soundboard);
        soundboard.sounds.extend(by_guild);
        soundboard.guild_names.extend(guild_names);
      }
    }
    None
  }

  fn update_voice_state(&mut self, mut state: VoiceStatePayload, now: Instant) -> Option<Timer> {
    let leaves_channel = match &state.channel_id {
      Some(Some(channel_id)) => *channel_id != self.current_channel,
      Some(None) => true,
      None => false,
    };
    if leaves_channel {
      self.voice_users.retain(|user| user.id != state.user_id);
      return None;
    }

    let grace = Duration::from_millis(u64::from(self.config.speaking_grace_ms));
    let Some(user) = self.voice_users.iter_mut().find(|u| u.id == state.user_id) else {
      if state.channel_id.is_some() && state.username.is_some() {
        self.voice_users.push(state.into());
      }
      return None;
    };

    if let Some(username) = &state.username {
      user.name.clone_from(username);
    }
    if let Some(avatar) = &state.avatar_url {
      user.avatar.clone_from(avatar);
    }
    if let Some(streaming) = state.streaming.take() {
      user.streaming = streaming;
    }
    if let Some(volume) = state.volume {
      user.volume = volume;
    }

    if let Some(muted) = state.mute {
      user.muted = muted;
    }
    if let Some(deafened) = state.deaf {
      user.deafened = deafened;
    }
    // Full updates carry `speaking: false` regardless; only speaking updates change it.
    let is_speaking_update = state.mute.is_none() && state.deaf.is_none();
    match state.speaking {
      Some(true) if is_speaking_update => {
        user.speaking = true;
        user.grace_until = None;
        return None;
      }
      Some(false) if is_speaking_update && user.speaking => {}
      _ => return None,
    }
    if grace.is_zero() {
      user.speaking = false;
      return None;
    }
    if user.grace_until.is_some() {
      return None;
    }

    let until = now + grace;
    user.grace_until = Some(until);
    Some(Timer {
      at: until,
      kind: TimerKind::GraceEnd {
        user_id: user.id.clone(),
      },
    })
  }

  /// Shows a notification, dropping the oldest beyond [`MAX_NOTIFICATIONS`].
  pub fn notify(&mut self, mut notification: Notification, now: Instant) -> Timer {
    let at = now + NOTIFICATION_TIMEOUT;
    notification.expires_at = Some(at);
    self.messages.push(notification);
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
      TimerKind::GraceEnd { user_id } => {
        let Some(user) = self
          .voice_users
          .iter_mut()
          .find(|user| user.id == *user_id && user.grace_until == Some(timer.at))
        else {
          return false;
        };
        user.grace_until = None;
        user.speaking = false;
        true
      }
      TimerKind::ExpireMessages => {
        let before = self.messages.len();
        self
          .messages
          .retain(|message| message.expires_at.is_none_or(|at| at > now));
        before != self.messages.len()
      }
    }
  }
}

/// Owns the shared state. Background threads mutate it through [`AppHandle::update`] and
/// [`AppHandle::apply`]; each change signals `redraw_tx`, which the UI drains before taking one
/// snapshot, so a burst of updates costs a single render.
#[derive(Clone)]
pub struct AppHandle {
  shared: SharedAppState,
  redraw_tx: flume::Sender<()>,
  open_config_tx: flume::Sender<()>,
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
    open_config_tx: flume::Sender<()>,
  ) -> (Self, flume::Receiver<BridgeCommand>) {
    let (timer_tx, timer_rx) = flume::unbounded();
    let (command_tx, command_rx) = flume::unbounded();
    let app = Self {
      shared: Arc::new(RwLock::new(state)),
      redraw_tx,
      open_config_tx,
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

  pub fn shared(&self) -> &SharedAppState {
    &self.shared
  }

  pub fn recording_keybind(&self) -> &Arc<AtomicBool> {
    &self.recording_keybind
  }

  pub fn read<T>(&self, f: impl FnOnce(&AppState) -> T) -> T {
    let state = self.shared.read().unwrap();
    f(&state)
  }

  pub fn update<T>(&self, f: impl FnOnce(&mut AppState) -> T) -> T {
    let result = {
      let mut state = self.shared.write().unwrap();
      f(&mut state)
    };
    self.redraw();
    result
  }

  pub fn update_if_changed(&self, f: impl FnOnce(&mut AppState) -> bool) {
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

  /// Requests a redraw; a full channel already has one pending.
  pub fn redraw(&self) {
    let _ = self.redraw_tx.try_send(());
  }

  pub fn open_config(&self) -> Result<(), flume::SendError<()>> {
    self.open_config_tx.send(())
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
    payloads::{Notification, SoundboardSoundPayload},
    protocol::{BridgeEvent, VoiceStatePayload},
    user::{User, UserVoiceState},
  };

  const GRACE: Duration = Duration::from_millis(300);

  fn member(user_id: &str) -> VoiceStatePayload {
    VoiceStatePayload {
      user_id: user_id.into(),
      username: Some(format!("user {user_id}")),
      channel_id: Some(Some("voice".into())),
      mute: Some(false),
      deaf: Some(false),
      ..VoiceStatePayload::default()
    }
  }

  fn update(state: VoiceStatePayload) -> BridgeEvent {
    BridgeEvent::VoiceStateUpdate { state }
  }

  fn speaking(user_id: &str, speaking: bool) -> BridgeEvent {
    update(VoiceStatePayload {
      user_id: user_id.into(),
      speaking: Some(speaking),
      ..VoiceStatePayload::default()
    })
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
        channel_id: None,
      },
      now,
    );
    state
  }

  fn voice_state(state: &AppState, user_id: &str) -> Option<UserVoiceState> {
    state
      .voice_users
      .iter()
      .find(|u| u.id == user_id)
      .map(User::voice_state)
  }

  fn grace_end(at: Instant) -> Timer {
    Timer {
      at,
      kind: TimerKind::GraceEnd {
        user_id: "peer".into(),
      },
    }
  }

  #[test]
  fn joining_records_the_channel_guild_and_members() {
    let state = joined();
    assert_eq!(state.current_channel, "voice");
    assert_eq!(state.current_guild_id, "guild");
    assert_eq!(state.voice_users.len(), 2);
  }

  #[test]
  fn the_joined_channel_does_not_depend_on_listing_yourself() {
    let mut state = AppState::default();
    let now = Instant::now();
    state.apply(
      BridgeEvent::ChannelJoined {
        states: vec![member("peer")],
        guild_id: None,
        channel_id: Some("voice".into()),
      },
      now,
    );
    assert_eq!(state.current_channel, "voice");
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
      update(VoiceStatePayload {
        mute: Some(true),
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
      update(VoiceStatePayload {
        mute: Some(true),
        ..member("peer")
      }),
      now,
    );
    state.apply(speaking("peer", false), now);
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Muted));
    state.fire(&grace_end(now + GRACE), now + GRACE);

    state.apply(
      update(VoiceStatePayload {
        mute: Some(false),
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
    state.apply(
      update(VoiceStatePayload {
        speaking: Some(false),
        ..member("peer")
      }),
      now,
    );
    assert_eq!(voice_state(&state, "peer"), Some(UserVoiceState::Speaking));
  }

  #[test]
  fn leaving_or_moving_away_removes_the_member() {
    let mut state = joined();
    let now = Instant::now();
    state.apply(
      update(VoiceStatePayload {
        channel_id: Some(None),
        ..member("peer")
      }),
      now,
    );
    assert_eq!(voice_state(&state, "peer"), None);

    state.apply(update(member("new")), now);
    assert!(voice_state(&state, "new").is_some());
    state.apply(
      update(VoiceStatePayload {
        channel_id: Some(Some("elsewhere".into())),
        ..member("new")
      }),
      now,
    );
    assert_eq!(voice_state(&state, "new"), None);
  }

  #[test]
  fn full_updates_carry_the_local_volume() {
    let mut state = joined();
    state.apply(
      update(VoiceStatePayload {
        volume: Some(150.),
        ..member("peer")
      }),
      Instant::now(),
    );
    let peer = state.voice_users.iter().find(|u| u.id == "peer").unwrap();
    assert!((peer.volume - 150.).abs() < f32::EPSILON);
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
      state.messages.iter().map(|m| m.title.clone()).collect()
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
    assert_eq!(state.voice_users, Vec::new());
    assert_eq!(state.current_channel, "");
  }

  #[test]
  fn config_subscribers_only_hear_real_changes() {
    let (redraw_tx, _redraw_rx) = flume::bounded(1);
    let (open_config_tx, _open_config_rx) = flume::unbounded();
    let (app, _commands) = super::AppHandle::new(AppState::default(), redraw_tx, open_config_tx);
    let changes = app.subscribe_config();

    app.set_config(|config| config.port = 7000);
    app.set_config(|config| config.port = 7000);

    assert_eq!(changes.try_recv().map(|c| c.port), Ok(7000));
    assert!(changes.try_recv().is_err());
  }

  #[test]
  fn soundboard_updates_merge_by_guild() {
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
    assert_eq!(state.soundboard.sounds[""].len(), 1);
    assert_eq!(state.soundboard.sounds["g"][0].sound_id, "c");
    assert_eq!(state.soundboard.guild_names["g"], "Guild");
  }
}
