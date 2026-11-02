use std::cell::Cell;

thread_local! {
  pub static DEFAULT_OVERLAY_TOGGLE: Cell<[&str; 2]> =
    const { Cell::new(["ControlLeft", "BackQuote"]) };
}

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

pub fn default_keybinds() -> Vec<Keybind> {
  vec![Keybind::new(
    strings_to_keys(&DEFAULT_OVERLAY_TOGGLE.get()),
    KeyEvent::ToggleOverlay,
  )]
}
