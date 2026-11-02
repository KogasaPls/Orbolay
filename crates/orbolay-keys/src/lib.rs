#[derive(Debug, Clone)]
pub enum KeyEvent {
  ToggleOverlay,
}

use rdev::Key;

pub mod keybind;

pub use keybind::{KeyState, Keybind, process};

pub fn string_to_key(string: impl AsRef<str>) -> Option<Key> {
  let s = string.as_ref();
  serde_json::from_str::<Key>(s)
    .or_else(|_| serde_json::from_str::<Key>(&format!("\"{s}\"")))
    .ok()
}

pub fn key_to_string(key: &Key) -> String {
  serde_json::to_string(key)
    .unwrap_or_default()
    .trim_matches('"')
    .to_owned()
}

pub fn keys_to_strings(keys: &[Key]) -> Vec<String> {
  keys.iter().map(key_to_string).collect()
}

pub fn strings_to_keys(strings: &[impl AsRef<str>]) -> Vec<Key> {
  strings.iter().filter_map(string_to_key).collect()
}
