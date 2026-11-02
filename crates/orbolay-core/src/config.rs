use std::{
  io,
  path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT: u16 = 6888;
pub const MAX_SPEAKING_GRACE_MS: u32 = 5000;
pub const UI_SCALE_RANGE: std::ops::RangeInclusive<f32> = 0.5..=2.0;

/// A screen anchor for the voice list, notifications, or the settings button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Corner {
  TopLeft,
  TopRight,
  BottomLeft,
  BottomRight,
  TopCenter,
  BottomCenter,
  CenterLeft,
  CenterRight,
}

/// Where along one axis an element is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
  Start,
  Center,
  End,
}

impl Corner {
  pub const ALL: [Self; 8] = [
    Self::TopLeft,
    Self::TopRight,
    Self::BottomLeft,
    Self::BottomRight,
    Self::TopCenter,
    Self::BottomCenter,
    Self::CenterLeft,
    Self::CenterRight,
  ];

  pub const fn label(self) -> &'static str {
    match self {
      Self::TopLeft => "topleft",
      Self::TopRight => "topright",
      Self::BottomLeft => "bottomleft",
      Self::BottomRight => "bottomright",
      Self::TopCenter => "topcenter",
      Self::BottomCenter => "bottomcenter",
      Self::CenterLeft => "centerleft",
      Self::CenterRight => "centerright",
    }
  }

  /// Horizontal and vertical anchoring.
  pub const fn axes(self) -> (Axis, Axis) {
    match self {
      Self::TopLeft => (Axis::Start, Axis::Start),
      Self::TopRight => (Axis::End, Axis::Start),
      Self::BottomLeft => (Axis::Start, Axis::End),
      Self::BottomRight => (Axis::End, Axis::End),
      Self::TopCenter => (Axis::Center, Axis::Start),
      Self::BottomCenter => (Axis::Center, Axis::End),
      Self::CenterLeft => (Axis::Start, Axis::Center),
      Self::CenterRight => (Axis::End, Axis::Center),
    }
  }

  /// A screen corner other than `a` and `b`, preferring the bottom right.
  pub fn unused(a: Self, b: Self) -> Self {
    // Two corners cannot cover the first three candidates, so the third is always free.
    [Self::BottomRight, Self::BottomLeft]
      .into_iter()
      .find(|corner| *corner != a && *corner != b)
      .unwrap_or(Self::TopRight)
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum DisplayVoiceMembers {
  Always,
  #[default]
  AlwaysSemiTransparent,
  WhenSpeaking,
}

impl DisplayVoiceMembers {
  pub const ALL: [Self; 3] = [
    Self::Always,
    Self::AlwaysSemiTransparent,
    Self::WhenSpeaking,
  ];

  pub const fn label(self) -> &'static str {
    match self {
      Self::Always => "always",
      Self::AlwaysSemiTransparent => "always (semi-transparent)",
      Self::WhenSpeaking => "only when speaking",
    }
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SpeakingGraceStyle {
  #[default]
  AsSpeaking,
  Dimmed,
}

impl SpeakingGraceStyle {
  pub const ALL: [Self; 2] = [Self::AsSpeaking, Self::Dimmed];

  pub const fn label(self) -> &'static str {
    match self {
      Self::AsSpeaking => "as speaking",
      Self::Dimmed => "dimmed",
    }
  }
}

pub type Rgb = (u8, u8, u8);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", from = "RawConfig")]
#[expect(
  clippy::struct_excessive_bools,
  reason = "independent user settings, stored as JSON booleans"
)]
pub struct Config {
  pub overlay_keybind: Vec<String>,
  #[serde(rename = "isKeybindEnabled")]
  pub keybind_enabled: bool,
  /// Index into the monitor list, only used by configs saved before display names existed.
  pub display_idx: Option<usize>,
  pub display_name: Option<String>,
  pub port: u16,
  pub enable_message_notifications: bool,
  pub message_alignment: Corner,
  pub user_alignment: Corner,
  pub message_offset_x: i32,
  pub message_offset_y: i32,
  pub user_offset_x: i32,
  pub user_offset_y: i32,
  pub display_voice_members: DisplayVoiceMembers,
  pub speaking_grace_ms: u32,
  pub speaking_grace_style: SpeakingGraceStyle,
  pub messages_semitransparent: bool,
  pub software_rendering: bool,
  pub xwayland: bool,
  pub accent: Rgb,
  pub text_color: Rgb,
  pub border_radius: f32,
  pub ui_scale: f32,
  /// Name of a user template in the templates folder; `None` selects the built-in template.
  pub user_template: Option<String>,
  /// Name of a notification template in the templates folder; `None` selects the built-in one.
  pub notification_template: Option<String>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      overlay_keybind: vec!["ControlLeft".into(), "BackQuote".into()],
      keybind_enabled: true,
      display_idx: None,
      display_name: None,
      port: DEFAULT_PORT,
      enable_message_notifications: true,
      message_alignment: Corner::TopRight,
      user_alignment: Corner::TopLeft,
      message_offset_x: 0,
      message_offset_y: 0,
      user_offset_x: 0,
      user_offset_y: 0,
      display_voice_members: DisplayVoiceMembers::default(),
      speaking_grace_ms: 300,
      speaking_grace_style: SpeakingGraceStyle::default(),
      messages_semitransparent: false,
      software_rendering: false,
      xwayland: true,
      accent: (30, 31, 35),
      text_color: (255, 255, 255),
      border_radius: 10.,
      ui_scale: 1.0,
      user_template: None,
      notification_template: None,
    }
  }
}

/// The on-disk format. A missing field, a `null` written by older versions, or a value of the wrong
/// type or range falls back to that field's default without affecting the others.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RawConfig {
  overlay_keybind: Lenient<Vec<String>>,
  is_keybind_enabled: Lenient<bool>,
  display_idx: Lenient<usize>,
  display_name: Lenient<String>,
  port: Lenient<u16>,
  enable_message_notifications: Lenient<bool>,
  message_alignment: Lenient<Corner>,
  user_alignment: Lenient<Corner>,
  message_offset_x: Lenient<i32>,
  message_offset_y: Lenient<i32>,
  user_offset_x: Lenient<i32>,
  user_offset_y: Lenient<i32>,
  display_voice_members: Lenient<DisplayVoiceMembers>,
  speaking_grace_ms: Lenient<u32>,
  speaking_grace_style: Lenient<SpeakingGraceStyle>,
  messages_semitransparent: Lenient<bool>,
  software_rendering: Lenient<bool>,
  xwayland: Lenient<bool>,
  accent: Lenient<Rgb>,
  text_color: Lenient<Rgb>,
  border_radius: Lenient<f32>,
  ui_scale: Lenient<f32>,
  user_template: Lenient<String>,
  notification_template: Lenient<String>,
}

/// A value that reads as `None` when it is `null` or does not parse as `T`.
struct Lenient<T>(Option<T>);

impl<T> Default for Lenient<T> {
  fn default() -> Self {
    Self(None)
  }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Lenient<T> {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(Self(T::deserialize(value).ok()))
  }
}

impl From<RawConfig> for Config {
  fn from(raw: RawConfig) -> Self {
    let d = Self::default();
    Self {
      overlay_keybind: raw.overlay_keybind.0.unwrap_or(d.overlay_keybind),
      keybind_enabled: raw.is_keybind_enabled.0.unwrap_or(d.keybind_enabled),
      display_idx: raw.display_idx.0,
      display_name: raw.display_name.0,
      port: raw.port.0.unwrap_or(d.port),
      enable_message_notifications: raw
        .enable_message_notifications
        .0
        .unwrap_or(d.enable_message_notifications),
      message_alignment: raw.message_alignment.0.unwrap_or(d.message_alignment),
      user_alignment: raw.user_alignment.0.unwrap_or(d.user_alignment),
      message_offset_x: raw.message_offset_x.0.unwrap_or_default(),
      message_offset_y: raw.message_offset_y.0.unwrap_or_default(),
      user_offset_x: raw.user_offset_x.0.unwrap_or_default(),
      user_offset_y: raw.user_offset_y.0.unwrap_or_default(),
      display_voice_members: raw
        .display_voice_members
        .0
        .unwrap_or(d.display_voice_members),
      speaking_grace_ms: raw.speaking_grace_ms.0.unwrap_or(d.speaking_grace_ms),
      speaking_grace_style: raw.speaking_grace_style.0.unwrap_or_default(),
      messages_semitransparent: raw.messages_semitransparent.0.unwrap_or_default(),
      software_rendering: raw.software_rendering.0.unwrap_or_default(),
      xwayland: raw.xwayland.0.unwrap_or(d.xwayland),
      accent: raw.accent.0.unwrap_or(d.accent),
      text_color: raw.text_color.0.unwrap_or(d.text_color),
      border_radius: raw.border_radius.0.unwrap_or(d.border_radius),
      ui_scale: raw.ui_scale.0.unwrap_or(d.ui_scale),
      user_template: raw.user_template.0,
      notification_template: raw.notification_template.0,
    }
    .normalized()
  }
}

impl Config {
  /// Brings out-of-range values back into the ranges the overlay supports.
  #[must_use]
  pub fn normalized(mut self) -> Self {
    let d = Self::default();
    self.speaking_grace_ms = self.speaking_grace_ms.min(MAX_SPEAKING_GRACE_MS);
    self.ui_scale = if self.ui_scale.is_finite() {
      self
        .ui_scale
        .clamp(*UI_SCALE_RANGE.start(), *UI_SCALE_RANGE.end())
    } else {
      d.ui_scale
    };
    self.border_radius = if self.border_radius.is_finite() {
      self.border_radius.max(0.)
    } else {
      d.border_radius
    };
    self
  }

  /// Position of the saved display among `names` (monitor names in Winit order): the native
  /// name when one is saved, otherwise the legacy index. `None` when it is not connected.
  pub fn saved_display(&self, names: &[Option<String>]) -> Option<usize> {
    match self.display_name.as_deref() {
      Some(name) => names
        .iter()
        .position(|candidate| candidate.as_deref() == Some(name)),
      None => self.display_idx.filter(|idx| *idx < names.len()),
    }
  }

  /// The display the overlay uses: the saved one, else `primary`, else the first. `None` only
  /// when no display is connected.
  pub fn select_display(&self, names: &[Option<String>], primary: Option<usize>) -> Option<usize> {
    if names.is_empty() {
      return None;
    }
    Some(self.saved_display(names).or(primary).unwrap_or(0))
  }
}

#[derive(Debug)]
pub enum LoadError {
  /// There is no config file yet.
  Missing,
  Io(io::Error),
  Parse(serde_json::Error),
}

impl std::fmt::Display for LoadError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Missing => write!(f, "config.json does not exist"),
      Self::Io(e) => write!(f, "could not read config.json: {e}"),
      Self::Parse(e) => write!(f, "could not parse config.json: {e}"),
    }
  }
}

pub fn config_dir() -> Option<PathBuf> {
  Some(dirs::config_dir()?.join("orbolay"))
}

pub const CONFIG_FILE: &str = "config.json";

pub fn config_path() -> Option<PathBuf> {
  config_dir().map(|dir| dir.join(CONFIG_FILE))
}

/// Writes the config through a temporary file so readers never see a partial file.
pub fn save_config(config: &Config) -> io::Result<()> {
  ensure_config_dirs();
  let path = config_path().ok_or_else(|| io::Error::other("no config directory"))?;
  write_atomically(&path, serde_json::to_string_pretty(config)?.as_bytes())
}

fn write_atomically(path: &Path, contents: &[u8]) -> io::Result<()> {
  let tmp = path.with_extension("json.tmp");
  std::fs::write(&tmp, contents)?;
  std::fs::rename(&tmp, path)
}

pub fn load_config() -> Result<Config, LoadError> {
  let path = config_path().ok_or(LoadError::Missing)?;
  let json = std::fs::read_to_string(path).map_err(|e| match e.kind() {
    io::ErrorKind::NotFound => LoadError::Missing,
    _ => LoadError::Io(e),
  })?;
  serde_json::from_str(&json).map_err(LoadError::Parse)
}

pub const TEMPLATES_DIR: &str = "templates";
pub const USER_TEMPLATES: &str = "users";
pub const NOTIFICATION_TEMPLATES: &str = "notifications";

pub fn ensure_config_dirs() {
  let Some(dir) = config_dir() else { return };

  let templates_dir = dir.join(TEMPLATES_DIR);
  let users_dir = templates_dir.join(USER_TEMPLATES);
  let notifications_dir = templates_dir.join(NOTIFICATION_TEMPLATES);

  for sub in [&dir, &templates_dir, &users_dir, &notifications_dir] {
    if !sub.exists() {
      let _ = std::fs::create_dir_all(sub);
    }
  }
}

#[cfg(test)]
mod tests {
  use serde_json::json;

  use super::{Config, Corner, DisplayVoiceMembers, SpeakingGraceStyle};

  fn names(displays: &[&str]) -> Vec<Option<String>> {
    displays
      .iter()
      .map(|name| Some((*name).to_owned()))
      .collect()
  }

  fn parse(value: serde_json::Value) -> Config {
    serde_json::from_value(value).unwrap()
  }

  #[test]
  fn named_display_survives_monitor_reordering() {
    let config = parse(json!({ "displayName": "DP-1", "displayIdx": 1 }));

    assert_eq!(
      config.saved_display(&names(&["HDMI-A-1", "DP-1", "DP-2"])),
      Some(1)
    );
    assert_eq!(
      config.saved_display(&names(&["HDMI-A-1", "DP-2", "DP-1"])),
      Some(2)
    );
  }

  #[test]
  fn missing_named_display_does_not_use_a_stale_index() {
    let config = parse(json!({ "displayName": "DP-1", "displayIdx": 1 }));
    assert_eq!(config.saved_display(&names(&["HDMI-A-1", "DP-2"])), None);
  }

  #[test]
  fn display_name_can_be_configured_without_an_index() {
    let config = parse(json!({"displayName": "DP-1"}));
    assert_eq!(config.saved_display(&names(&["DP-2", "DP-1"])), Some(1));
  }

  #[test]
  fn legacy_display_index_is_preserved() {
    let config = parse(json!({"displayIdx": 1}));
    assert_eq!(config.saved_display(&names(&["DP-2", "DP-1"])), Some(1));
  }

  #[test]
  fn a_missing_saved_display_falls_back_to_the_primary_then_the_first() {
    let config = parse(json!({"displayName": "DP-1"}));
    assert_eq!(
      config.select_display(&names(&["HDMI-A-1", "DP-2"]), Some(1)),
      Some(1)
    );
    assert_eq!(
      config.select_display(&names(&["HDMI-A-1", "DP-2"]), None),
      Some(0)
    );
    assert_eq!(config.select_display(&names(&[]), Some(0)), None);
    let legacy = parse(json!({"displayIdx": 5}));
    assert_eq!(
      legacy.select_display(&names(&["HDMI-A-1", "DP-2"]), Some(1)),
      Some(1)
    );
  }

  #[test]
  fn display_name_survives_config_round_trip() {
    let config = parse(json!({ "displayName": "DP-1", "displayIdx": 1 }));
    let saved = serde_json::to_value(&config).unwrap();
    assert_eq!(saved["displayName"], "DP-1");
  }

  #[test]
  fn empty_file_reads_as_defaults() {
    assert_eq!(parse(json!({})), Config::default());
  }

  #[test]
  fn nulls_written_by_older_versions_read_as_defaults() {
    let config = parse(json!({
      "overlayKeybind": null,
      "isKeybindEnabled": null,
      "port": null,
      "userAlignment": null,
      "messageAlignment": null,
      "displayVoiceMembers": null,
      "softwareRendering": null
    }));
    assert_eq!(config, Config::default());
  }

  #[test]
  fn a_mistyped_value_resets_only_its_own_field() {
    let config = parse(json!({ "port": 70000, "uiScale": "1.5", "speakingGraceMs": 900 }));
    assert_eq!(config.port, Config::default().port);
    assert!((config.ui_scale - Config::default().ui_scale).abs() < f32::EPSILON);
    assert_eq!(config.speaking_grace_ms, 900);
  }

  #[test]
  fn unknown_enum_values_fall_back_to_defaults() {
    let config = parse(json!({
      "userAlignment": "middle",
      "displayVoiceMembers": "sometimes",
      "speakingGraceStyle": 3
    }));
    assert_eq!(config.user_alignment, Corner::TopLeft);
    assert_eq!(
      config.display_voice_members,
      DisplayVoiceMembers::AlwaysSemiTransparent
    );
    assert_eq!(config.speaking_grace_style, SpeakingGraceStyle::AsSpeaking);
  }

  #[test]
  fn saved_files_keep_the_existing_keys_and_values() {
    let config = Config {
      user_alignment: Corner::BottomRight,
      display_voice_members: DisplayVoiceMembers::WhenSpeaking,
      keybind_enabled: false,
      ..Config::default()
    };
    let saved = serde_json::to_value(&config).unwrap();
    assert_eq!(saved["userAlignment"], "bottomright");
    assert_eq!(saved["displayVoiceMembers"], "whenSpeaking");
    assert_eq!(saved["isKeybindEnabled"], false);
    assert_eq!(saved["port"], 6888);
    assert_eq!(serde_json::from_value::<Config>(saved).unwrap(), config);
  }

  #[test]
  fn out_of_range_values_are_brought_into_range() {
    let config = parse(json!({"speakingGraceMs": 60000, "uiScale": 9.0, "borderRadius": -4.0}));
    assert_eq!(config.speaking_grace_ms, 5000);
    assert!((config.ui_scale - 2.0).abs() < f32::EPSILON);
    assert!(config.border_radius.abs() < f32::EPSILON);
  }

  #[test]
  fn unused_corner_avoids_both_occupied_ones() {
    assert_eq!(
      Corner::unused(Corner::BottomRight, Corner::BottomLeft),
      Corner::TopRight
    );
  }

  #[test]
  fn unused_corner_skips_occupied_ones() {
    assert_eq!(
      Corner::unused(Corner::BottomRight, Corner::TopLeft),
      Corner::BottomLeft
    );
    assert_eq!(
      Corner::unused(Corner::TopCenter, Corner::TopLeft),
      Corner::BottomRight
    );
  }
}
