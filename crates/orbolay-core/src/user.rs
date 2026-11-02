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
  pub avatar: String,
  pub muted: bool,
  pub deafened: bool,
  /// Stays true during the speaking grace period, until `grace_until` passes.
  pub speaking: bool,
  pub streaming: bool,
  pub camera: bool,
  pub volume: f32,
  pub grace_until: Option<Instant>,
}

impl User {
  pub const fn voice_state(&self) -> UserVoiceState {
    if self.deafened {
      UserVoiceState::Deafened
    } else if self.muted {
      UserVoiceState::Muted
    } else if self.speaking {
      UserVoiceState::Speaking
    } else {
      UserVoiceState::NotSpeaking
    }
  }
}

use crate::protocol::VoiceStatePayload;

impl From<VoiceStatePayload> for User {
  fn from(val: VoiceStatePayload) -> Self {
    Self {
      name: val.username.unwrap_or_else(|| "Unknown".to_owned()),
      id: val.user_id,
      avatar: val.avatar_url.unwrap_or_default(),
      muted: val.mute.unwrap_or_default(),
      deafened: val.deaf.unwrap_or_default(),
      speaking: val.speaking.unwrap_or_default(),
      streaming: val.streaming.unwrap_or_default(),
      camera: val.camera.unwrap_or_default(),
      volume: val.volume.unwrap_or(100.),
      grace_until: None,
    }
  }
}
