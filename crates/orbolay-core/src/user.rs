use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum PremiumType {
  #[default]
  None,
  NitroClassic,
  Nitro,
  NitroBasic,
}

impl PremiumType {
  pub fn has_nitro(&self) -> bool {
    !matches!(self, Self::None)
  }
}

impl TryFrom<u64> for PremiumType {
  type Error = String;

  fn try_from(value: u64) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(Self::None),
      1 => Ok(Self::NitroClassic),
      2 => Ok(Self::Nitro),
      3 => Ok(Self::NitroBasic),
      _ => Err(format!("Unknown premium type: {value}")),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserVoiceState {
  Speaking,
  NotSpeaking,
  Muted,
  Deafened,
}

#[derive(Clone, Debug, PartialEq)]
pub struct User {
  pub name: String,
  pub id: String,
  pub avatar: String,
  pub voice_state: UserVoiceState,
  pub streaming: bool,
  pub camera: bool,
  pub volume: f32,
  pub grace_until: Option<Instant>,
}

use crate::payloads::WsVoiceState;
use crate::payloads::ipc::RpcVoiceState;

impl From<WsVoiceState> for User {
  fn from(val: WsVoiceState) -> Self {
    let voice_state = UserVoiceState::from(&val);

    Self {
      name: val.username.unwrap_or_else(|| "Unknown".to_owned()),
      id: val.user_id,
      avatar: val.avatar_url.unwrap_or_default(),
      voice_state,
      streaming: val.streaming.unwrap_or_default(),
      camera: val.camera.unwrap_or_default(),
      volume: 100.,
      grace_until: None,
    }
  }
}

impl From<WsVoiceState> for UserVoiceState {
  fn from(val: WsVoiceState) -> Self {
    match (val.mute, val.deaf, val.speaking) {
      (_, Some(true), _) => Self::Deafened,
      (Some(true), _, _) => Self::Muted,
      (_, _, Some(true)) => Self::Speaking,
      _ => Self::NotSpeaking,
    }
  }
}

impl From<&WsVoiceState> for UserVoiceState {
  fn from(val: &WsVoiceState) -> Self {
    match (val.mute, val.deaf, val.speaking) {
      (_, Some(true), _) => Self::Deafened,
      (Some(true), _, _) => Self::Muted,
      (_, _, Some(true)) => Self::Speaking,
      _ => Self::NotSpeaking,
    }
  }
}

impl From<RpcVoiceState> for User {
  fn from(val: RpcVoiceState) -> Self {
    let voice_state = UserVoiceState::from(&val);

    Self {
      name: val
        .nick
        .or(val.user.global_name)
        .unwrap_or(val.user.username),
      id: val.user.id,
      avatar: val.user.avatar.unwrap_or_default(),
      voice_state,
      streaming: false,
      camera: false,
      volume: val.volume,
      grace_until: None,
    }
  }
}

impl From<RpcVoiceState> for UserVoiceState {
  fn from(val: RpcVoiceState) -> Self {
    if val.voice_state.deaf || val.voice_state.self_deaf {
      Self::Deafened
    } else if val.voice_state.mute || val.voice_state.self_mute {
      Self::Muted
    } else {
      Self::NotSpeaking
    }
  }
}

impl From<&RpcVoiceState> for UserVoiceState {
  fn from(val: &RpcVoiceState) -> Self {
    if val.voice_state.deaf || val.voice_state.self_deaf {
      Self::Deafened
    } else if val.voice_state.mute || val.voice_state.self_mute {
      Self::Muted
    } else {
      Self::NotSpeaking
    }
  }
}
