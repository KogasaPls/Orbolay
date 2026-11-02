use std::time::Instant;

/// What a member's voice status looks like: deafened outranks muted, which outranks speaking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserVoiceState {
  Speaking,
  NotSpeaking,
  Muted,
  Deafened,
}

#[derive(Clone, Debug, PartialEq)]
#[expect(
  clippy::struct_excessive_bools,
  reason = "independent voice flags reported by Discord"
)]
pub struct User {
  pub name: String,
  pub id: String,
  /// Discord avatar hash; `None` uses the default avatar.
  pub avatar: Option<String>,
  pub muted: bool,
  pub deafened: bool,
  pub speech: Speech,
  pub streaming: bool,
  pub camera: bool,
  pub volume: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Speech {
  #[default]
  Silent,
  Speaking,
  /// Stopped speaking but still shown as speaking until `until`.
  Grace {
    until: Instant,
  },
}

impl User {
  pub const fn voice_state(&self) -> UserVoiceState {
    if self.deafened {
      UserVoiceState::Deafened
    } else if self.muted {
      UserVoiceState::Muted
    } else if !matches!(self.speech, Speech::Silent) {
      UserVoiceState::Speaking
    } else {
      UserVoiceState::NotSpeaking
    }
  }
}

use crate::protocol::MemberState;

/// Discord local volume range is 0 to this, with 100 as the default.
pub const MAX_VOLUME: f32 = 200.;

fn local_volume(volume: f32) -> f32 {
  volume.clamp(0., MAX_VOLUME)
}

impl User {
  /// A member who just appeared, not yet speaking.
  pub fn joined(state: MemberState) -> Self {
    Self {
      name: state.username.unwrap_or_else(|| "Unknown".to_owned()),
      id: state.user_id,
      avatar: state.avatar_url,
      muted: state.mute,
      deafened: state.deaf,
      speech: Speech::Silent,
      streaming: state.streaming,
      camera: state.camera,
      volume: state.volume.map_or(100., local_volume),
    }
  }

  /// Applies a full state update, keeping what it does not carry: speech, and the name, avatar,
  /// and volume when they are missing.
  pub fn update(&mut self, state: MemberState) {
    if let Some(name) = state.username {
      self.name = name;
    }
    if state.avatar_url.is_some() {
      self.avatar = state.avatar_url;
    }
    if let Some(volume) = state.volume {
      self.volume = local_volume(volume);
    }
    self.muted = state.mute;
    self.deafened = state.deaf;
    self.streaming = state.streaming;
    self.camera = state.camera;
  }
}
