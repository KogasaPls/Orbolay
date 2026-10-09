#[derive(Clone, Debug, PartialEq, Default)]
pub enum PremiumType {
  #[default]
  None,
  NitroClassic,
  Nitro,
  NitroBasic,
}

impl PremiumType {
  pub fn has_nitro(&self) -> bool {
    !matches!(self, PremiumType::None)
  }
}

impl TryFrom<u64> for PremiumType {
  type Error = String;

  fn try_from(value: u64) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(PremiumType::None),
      1 => Ok(PremiumType::NitroClassic),
      2 => Ok(PremiumType::Nitro),
      3 => Ok(PremiumType::NitroBasic),
      _ => Err(format!("Unknown premium type: {}", value)),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
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
  pub speaking_epoch: u64,
  pub lingering: bool,
}

impl User {
  /// Returns the speaking epoch when this change ends a speaking period, so the
  /// caller can end the linger that follows it
  pub fn set_voice_state(&mut self, voice_state: UserVoiceState) -> Option<u64> {
    let was_speaking = self.voice_state == UserVoiceState::Speaking;
    let is_speaking = voice_state == UserVoiceState::Speaking;
    self.voice_state = voice_state;

    if is_speaking && !was_speaking {
      self.speaking_epoch += 1;
    }

    if was_speaking && !is_speaking {
      self.lingering = true;
      return Some(self.speaking_epoch);
    }

    None
  }
}

use crate::payloads::WsVoiceState;
use crate::payloads::ipc::RpcVoiceState;

impl From<WsVoiceState> for User {
  fn from(val: WsVoiceState) -> Self {
    let voice_state = UserVoiceState::from(&val);

    User {
      name: val.username.unwrap_or("Unknown".to_string()),
      id: val.user_id,
      avatar: val.avatar_url.unwrap_or_default(),
      voice_state,
      streaming: val.streaming.unwrap_or_default(),
      camera: val.camera.unwrap_or_default(),
      volume: 100.,
      speaking_epoch: 0,
      lingering: false,
    }
  }
}

impl From<WsVoiceState> for UserVoiceState {
  fn from(val: WsVoiceState) -> Self {
    match (val.mute, val.deaf, val.speaking) {
      (_, Some(true), _) => UserVoiceState::Deafened,
      (Some(true), _, _) => UserVoiceState::Muted,
      (_, _, Some(true)) => UserVoiceState::Speaking,
      _ => UserVoiceState::NotSpeaking,
    }
  }
}

impl From<&WsVoiceState> for UserVoiceState {
  fn from(val: &WsVoiceState) -> Self {
    match (val.mute, val.deaf, val.speaking) {
      (_, Some(true), _) => UserVoiceState::Deafened,
      (Some(true), _, _) => UserVoiceState::Muted,
      (_, _, Some(true)) => UserVoiceState::Speaking,
      _ => UserVoiceState::NotSpeaking,
    }
  }
}

impl From<RpcVoiceState> for User {
  fn from(val: RpcVoiceState) -> Self {
    let voice_state = UserVoiceState::from(&val);

    User {
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
      speaking_epoch: 0,
      lingering: false,
    }
  }
}

impl From<RpcVoiceState> for UserVoiceState {
  fn from(val: RpcVoiceState) -> Self {
    if val.voice_state.deaf || val.voice_state.self_deaf {
      UserVoiceState::Deafened
    } else if val.voice_state.mute || val.voice_state.self_mute {
      UserVoiceState::Muted
    } else {
      UserVoiceState::NotSpeaking
    }
  }
}

impl From<&RpcVoiceState> for UserVoiceState {
  fn from(val: &RpcVoiceState) -> Self {
    if val.voice_state.deaf || val.voice_state.self_deaf {
      UserVoiceState::Deafened
    } else if val.voice_state.mute || val.voice_state.self_mute {
      UserVoiceState::Muted
    } else {
      UserVoiceState::NotSpeaking
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn user(voice_state: UserVoiceState) -> User {
    User {
      name: "user".into(),
      id: "1".into(),
      avatar: String::new(),
      voice_state,
      streaming: false,
      camera: false,
      volume: 100.,
      speaking_epoch: 0,
      lingering: false,
    }
  }

  #[test]
  fn linger_starts_only_when_speaking_ends() {
    let mut u = user(UserVoiceState::NotSpeaking);

    assert_eq!(u.set_voice_state(UserVoiceState::Speaking), None);
    assert_eq!(u.speaking_epoch, 1);
    assert_eq!(u.set_voice_state(UserVoiceState::Speaking), None);
    assert_eq!(u.speaking_epoch, 1);
    assert!(!u.lingering);

    assert_eq!(u.set_voice_state(UserVoiceState::Muted), Some(1));
    assert!(u.lingering);

    u.lingering = false;
    assert_eq!(u.set_voice_state(UserVoiceState::NotSpeaking), None);
    assert!(!u.lingering);
  }
}
