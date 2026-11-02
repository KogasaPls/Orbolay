use std::time::Instant;

use serde::Deserialize;

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
