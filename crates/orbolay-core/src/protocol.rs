//! Messages exchanged with the `OrbolayBridge` plugin over the websocket.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

/// A voice member's full state, sent when they join, leave, or change mute, deafen, or stream.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
#[expect(
  clippy::struct_excessive_bools,
  reason = "independent voice flags reported by Discord"
)]
pub struct MemberState {
  pub user_id: String,
  pub username: Option<String>,
  /// Discord avatar hash.
  pub avatar: Option<String>,
  /// The member's voice channel; `None` once they left voice.
  pub channel_id: Option<String>,
  pub mute: bool,
  pub deaf: bool,
  pub streaming: bool,
  pub camera: bool,
  /// Local playback volume, 0-200 with 100 as the default.
  pub volume: Option<f32>,
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
    states: Vec<MemberState>,
    #[serde(default)]
    guild_id: Option<String>,
    channel_id: String,
  },
  VoiceStateUpdate {
    state: MemberState,
  },
  Speaking {
    user_id: String,
    speaking: bool,
  },
  /// A member started or stopped playing soundboard sounds in the current channel.
  SoundboardPlaying {
    user_id: String,
    playing: bool,
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

/// Reads a missing or null field as its default.
fn nullable<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
  deserializer: D,
) -> Result<T, D::Error> {
  Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
  #[serde(default, deserialize_with = "nullable")]
  pub title: String,
  #[serde(default, deserialize_with = "nullable")]
  pub body: String,
  #[serde(default, deserialize_with = "nullable")]
  pub icon: String,
  pub guild_id: Option<String>,
  pub channel_id: Option<String>,
  pub message_id: Option<String>,
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
  use serde_json::json;

  use super::{BridgeCommand, BridgeEvent, MemberState};

  fn event(value: serde_json::Value) -> BridgeEvent {
    serde_json::from_value(value).unwrap()
  }

  #[test]
  fn speaking_is_its_own_event() {
    assert_eq!(
      event(json!({"cmd": "SPEAKING", "userId": "1", "speaking": true})),
      BridgeEvent::Speaking {
        user_id: "1".into(),
        speaking: true
      }
    );
  }

  #[test]
  fn soundboard_playing_names_the_member() {
    assert_eq!(
      event(json!({"cmd": "SOUNDBOARD_PLAYING", "userId": "1", "playing": true})),
      BridgeEvent::SoundboardPlaying {
        user_id: "1".into(),
        playing: true
      }
    );
  }

  #[test]
  fn member_states_default_missing_flags_and_a_null_channel_means_they_left() {
    let BridgeEvent::VoiceStateUpdate { state } = event(json!({
      "cmd": "VOICE_STATE_UPDATE",
      "state": { "userId": "1", "channelId": null, "mute": true }
    })) else {
      panic!("wrong variant");
    };
    assert_eq!(
      state,
      MemberState {
        user_id: "1".into(),
        mute: true,
        ..MemberState::default()
      }
    );
  }

  #[test]
  fn notifications_tolerate_missing_and_null_text() {
    let BridgeEvent::MessageNotification { message } = event(json!({
      "cmd": "MESSAGE_NOTIFICATION",
      "message": { "title": "T", "body": null, "channelId": "1" }
    })) else {
      panic!("wrong variant");
    };
    assert_eq!(
      (
        message.title.as_str(),
        message.body.as_str(),
        message.icon.as_str()
      ),
      ("T", "", "")
    );
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
