//! Messages exchanged with the `OrbolayBridge` plugin over the websocket.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

use std::time::Instant;

/// A voice participant as the bridge reports it. Speaking updates carry only `user_id` and
/// `speaking`; full updates carry the remaining fields.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStatePayload {
  pub user_id: String,
  pub username: Option<String>,
  pub avatar_url: Option<String>,
  /// `None` when the field is absent, `Some(None)` when the user left voice.
  #[serde(default, deserialize_with = "present")]
  pub channel_id: Option<Option<String>>,
  pub mute: Option<bool>,
  pub deaf: Option<bool>,
  pub speaking: Option<bool>,
  pub streaming: Option<bool>,
  pub camera: Option<bool>,
  /// Local playback volume, 0-200 with 100 as the default.
  pub volume: Option<f32>,
}

fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
  D: Deserializer<'de>,
  T: Deserialize<'de>,
{
  T::deserialize(deserializer).map(Some)
}

/// Messages the bridge sends to Orbolay.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(
  tag = "cmd",
  rename_all = "SCREAMING_SNAKE_CASE",
  rename_all_fields = "camelCase"
)]
pub enum BridgeEvent {
  RegisterConfig {
    user_id: String,
    #[serde(default)]
    premium_type: Option<u64>,
  },
  ChannelJoined {
    states: Vec<VoiceStatePayload>,
    #[serde(default)]
    guild_id: Option<String>,
    /// The joined channel; older bridges omit it and only list members.
    #[serde(default)]
    channel_id: Option<String>,
  },
  VoiceStateUpdate {
    state: VoiceStatePayload,
  },
  ChannelLeft,
  MessageNotification {
    message: Notification,
  },
  StreamerMode {
    enabled: bool,
  },
  SoundboardUpdate {
    sounds: Vec<SoundboardSoundPayload>,
    #[serde(default)]
    guild_names: HashMap<String, String>,
  },
}

/// Messages Orbolay sends to the bridge.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "cmd", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BridgeCommand {
  ToggleMute,
  ToggleDeaf,
  Disconnect,
  StopStream,
  Navigate {
    guild_id: Option<String>,
    channel_id: Option<String>,
    message_id: Option<String>,
  },
  PlaySoundboardSound {
    sound_id: String,
    source_guild_id: Option<String>,
  },
  SetUserVolume {
    user_id: String,
    volume: f64,
  },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
  pub title: String,
  pub body: String,
  pub icon: String,
  pub guild_id: Option<String>,
  pub channel_id: Option<String>,
  pub message_id: Option<String>,

  /// When the notification disappears, set once it is shown.
  #[serde(skip)]
  pub expires_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SoundboardSoundPayload {
  pub sound_id: String,
  pub name: String,
  pub volume: f64,
  // Custom emoji
  pub emoji_id: Option<String>,
  // Standard emoji, this is the emoji itself (not a name, even though it says that)
  pub emoji_name: Option<String>,
  pub guild_id: Option<String>,
  // Can this be used?
  pub available: bool,
}

#[cfg(test)]
mod tests {
  use super::{BridgeCommand, BridgeEvent, VoiceStatePayload};
  use serde_json::json;

  fn event(value: serde_json::Value) -> BridgeEvent {
    serde_json::from_value(value).unwrap()
  }

  #[test]
  fn speaking_updates_omit_the_channel() {
    let BridgeEvent::VoiceStateUpdate { state } = event(json!({
      "cmd": "VOICE_STATE_UPDATE",
      "state": { "userId": "1", "speaking": true }
    })) else {
      panic!("wrong variant");
    };
    assert_eq!(
      state,
      VoiceStatePayload {
        user_id: "1".into(),
        speaking: Some(true),
        ..VoiceStatePayload::default()
      }
    );
  }

  #[test]
  fn a_null_channel_means_the_user_left() {
    let BridgeEvent::VoiceStateUpdate { state } = event(json!({
      "cmd": "VOICE_STATE_UPDATE",
      "state": { "userId": "1", "channelId": null }
    })) else {
      panic!("wrong variant");
    };
    assert_eq!(state.channel_id, Some(None));
  }

  #[test]
  fn events_without_fields_parse() {
    assert_eq!(
      event(json!({"cmd": "CHANNEL_LEFT"})),
      BridgeEvent::ChannelLeft
    );
  }

  #[test]
  fn register_config_accepts_the_current_bridge_payload() {
    assert_eq!(
      event(json!({"cmd": "REGISTER_CONFIG", "userId": "7"})),
      BridgeEvent::RegisterConfig {
        user_id: "7".into(),
        premium_type: None
      }
    );
  }

  #[test]
  fn unknown_commands_are_errors() {
    assert!(serde_json::from_value::<BridgeEvent>(json!({"cmd": "NOPE"})).is_err());
  }

  #[test]
  fn commands_use_the_bridge_field_names() {
    assert_eq!(
      serde_json::to_value(BridgeCommand::ToggleDeaf).unwrap(),
      json!({"cmd": "TOGGLE_DEAF"})
    );
    assert_eq!(
      serde_json::to_value(BridgeCommand::Navigate {
        guild_id: Some("g".into()),
        channel_id: Some("c".into()),
        message_id: None,
      })
      .unwrap(),
      json!({"cmd": "NAVIGATE", "guild_id": "g", "channel_id": "c", "message_id": null})
    );
    assert_eq!(
      serde_json::to_value(BridgeCommand::PlaySoundboardSound {
        sound_id: "s".into(),
        source_guild_id: None,
      })
      .unwrap(),
      json!({"cmd": "PLAY_SOUNDBOARD_SOUND", "sound_id": "s", "source_guild_id": null})
    );
  }
}
