use serde::Deserialize;

fn default_timeout_secs() -> i64 {
  5
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
  pub title: String,
  pub body: String,
  pub icon: String,
  pub guild_id: Option<String>,
  pub channel_id: Option<String>,
  pub message_id: Option<String>,

  // Stuff not part of the Discord payload
  #[serde(default, skip_deserializing)]
  pub timestamp: Option<i64>,
  #[serde(default = "default_timeout_secs", skip_deserializing)]
  pub timeout_secs: i64,
}

impl Default for Notification {
  fn default() -> Self {
    Self {
      title: String::new(),
      body: String::new(),
      icon: String::new(),
      guild_id: None,
      channel_id: None,
      message_id: None,
      timestamp: Some(chrono::Utc::now().timestamp()),
      timeout_secs: 5,
    }
  }
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

pub mod ws;

pub use ws::{ChannelJoinPayload, NotificationPayload, UpdatePayload, WsVoiceState};
