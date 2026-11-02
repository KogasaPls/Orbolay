//! Global keybinds: the key names stored in the config, and matching pressed keys against binds.

use std::time::{Duration, Instant};

use evdev::KeyCode;
use serde::{Deserialize, Serialize};

macro_rules! keys {
  ($($key:ident = $code:ident,)*) => {
    /// A keyboard key, serialized with the names earlier versions stored in the config.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Key {
      $($key,)*
    }

    impl Key {
      pub const ALL: &[Self] = &[$(Self::$key,)*];

      pub const fn code(self) -> KeyCode {
        match self {
          $(Self::$key => KeyCode::$code,)*
        }
      }

      pub fn from_code(code: KeyCode) -> Option<Self> {
        match code {
          $(KeyCode::$code => Some(Self::$key),)*
          _ => None,
        }
      }
    }
  };
}

keys! {
  Alt = KEY_LEFTALT,
  AltGr = KEY_RIGHTALT,
  Backspace = KEY_BACKSPACE,
  CapsLock = KEY_CAPSLOCK,
  ControlLeft = KEY_LEFTCTRL,
  ControlRight = KEY_RIGHTCTRL,
  Delete = KEY_DELETE,
  DownArrow = KEY_DOWN,
  End = KEY_END,
  Escape = KEY_ESC,
  F1 = KEY_F1,
  F2 = KEY_F2,
  F3 = KEY_F3,
  F4 = KEY_F4,
  F5 = KEY_F5,
  F6 = KEY_F6,
  F7 = KEY_F7,
  F8 = KEY_F8,
  F9 = KEY_F9,
  F10 = KEY_F10,
  F11 = KEY_F11,
  F12 = KEY_F12,
  Home = KEY_HOME,
  LeftArrow = KEY_LEFT,
  MetaLeft = KEY_LEFTMETA,
  MetaRight = KEY_RIGHTMETA,
  PageDown = KEY_PAGEDOWN,
  PageUp = KEY_PAGEUP,
  Return = KEY_ENTER,
  RightArrow = KEY_RIGHT,
  ShiftLeft = KEY_LEFTSHIFT,
  ShiftRight = KEY_RIGHTSHIFT,
  Space = KEY_SPACE,
  Tab = KEY_TAB,
  UpArrow = KEY_UP,
  PrintScreen = KEY_SYSRQ,
  ScrollLock = KEY_SCROLLLOCK,
  Pause = KEY_PAUSE,
  NumLock = KEY_NUMLOCK,
  BackQuote = KEY_GRAVE,
  Num1 = KEY_1,
  Num2 = KEY_2,
  Num3 = KEY_3,
  Num4 = KEY_4,
  Num5 = KEY_5,
  Num6 = KEY_6,
  Num7 = KEY_7,
  Num8 = KEY_8,
  Num9 = KEY_9,
  Num0 = KEY_0,
  Minus = KEY_MINUS,
  Equal = KEY_EQUAL,
  KeyQ = KEY_Q,
  KeyW = KEY_W,
  KeyE = KEY_E,
  KeyR = KEY_R,
  KeyT = KEY_T,
  KeyY = KEY_Y,
  KeyU = KEY_U,
  KeyI = KEY_I,
  KeyO = KEY_O,
  KeyP = KEY_P,
  LeftBracket = KEY_LEFTBRACE,
  RightBracket = KEY_RIGHTBRACE,
  KeyA = KEY_A,
  KeyS = KEY_S,
  KeyD = KEY_D,
  KeyF = KEY_F,
  KeyG = KEY_G,
  KeyH = KEY_H,
  KeyJ = KEY_J,
  KeyK = KEY_K,
  KeyL = KEY_L,
  SemiColon = KEY_SEMICOLON,
  Quote = KEY_APOSTROPHE,
  BackSlash = KEY_BACKSLASH,
  IntlBackslash = KEY_102ND,
  KeyZ = KEY_Z,
  KeyX = KEY_X,
  KeyC = KEY_C,
  KeyV = KEY_V,
  KeyB = KEY_B,
  KeyN = KEY_N,
  KeyM = KEY_M,
  Comma = KEY_COMMA,
  Dot = KEY_DOT,
  Slash = KEY_SLASH,
  Insert = KEY_INSERT,
  KpReturn = KEY_KPENTER,
  KpMinus = KEY_KPMINUS,
  KpPlus = KEY_KPPLUS,
  KpMultiply = KEY_KPASTERISK,
  KpDivide = KEY_KPSLASH,
  Kp0 = KEY_KP0,
  Kp1 = KEY_KP1,
  Kp2 = KEY_KP2,
  Kp3 = KEY_KP3,
  Kp4 = KEY_KP4,
  Kp5 = KEY_KP5,
  Kp6 = KEY_KP6,
  Kp7 = KEY_KP7,
  Kp8 = KEY_KP8,
  Kp9 = KEY_KP9,
  KpDelete = KEY_KPDOT,
  Function = KEY_FN,
}

pub fn key_to_string(key: &Key) -> String {
  serde_json::to_value(key)
    .ok()
    .and_then(|value| value.as_str().map(str::to_owned))
    .unwrap_or_default()
}

pub fn keys_to_strings(keys: &[Key]) -> Vec<String> {
  keys.iter().map(key_to_string).collect()
}

/// Parses config key names, skipping names this version does not know.
pub fn strings_to_keys(strings: &[impl AsRef<str>]) -> Vec<Key> {
  strings
    .iter()
    .filter_map(|name| serde_json::from_value(name.as_ref().into()).ok())
    .collect()
}

/// After this long without a key change, held keys are assumed released, in case a release
/// was missed.
const STALE_TIMEOUT: Duration = Duration::from_secs(5);

/// A non-empty set of keys that fires when exactly those keys are held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keybind(Vec<Key>);

impl Keybind {
  pub fn new(keys: Vec<Key>) -> Option<Self> {
    (!keys.is_empty()).then_some(Self(keys))
  }

  pub fn keys(&self) -> &[Key] {
    &self.0
  }

  fn matches(&self, pressed: &[Key]) -> bool {
    pressed.len() == self.0.len() && self.0.iter().all(|k| pressed.contains(k))
  }
}

/// Keys currently held across all devices, tracked against one bind.
pub struct KeyState {
  pressed: Vec<Key>,
  last_update: Instant,
  /// Whether the bind was fully held after the previous change, so holding it fires once.
  active: bool,
}

impl Default for KeyState {
  fn default() -> Self {
    Self {
      pressed: Vec::new(),
      last_update: Instant::now(),
      active: false,
    }
  }
}

impl KeyState {
  /// Records a press or release and returns whether `bind` just became fully held.
  pub fn process(&mut self, bind: &Keybind, key: Key, pressed: bool, now: Instant) -> bool {
    if now.duration_since(self.last_update) > STALE_TIMEOUT {
      self.pressed.clear();
      self.active = false;
    }
    self.last_update = now;

    if pressed {
      if !self.pressed.contains(&key) {
        self.pressed.push(key);
      }
    } else {
      self.pressed.retain(|&k| k != key);
    }

    let was_active = std::mem::replace(&mut self.active, bind.matches(&self.pressed));
    self.active && !was_active
  }
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, Instant};

  use evdev::KeyCode;

  use super::{Key, KeyState, Keybind, keys_to_strings, strings_to_keys};

  fn toggle() -> Keybind {
    Keybind::new(vec![Key::ControlLeft, Key::BackQuote]).unwrap()
  }

  #[test]
  fn config_names_round_trip() {
    let names = ["ControlLeft", "BackQuote", "KpDelete"];
    let keys = strings_to_keys(&names);
    assert_eq!(keys, [Key::ControlLeft, Key::BackQuote, Key::KpDelete]);
    assert_eq!(keys_to_strings(&keys), names);
  }

  #[test]
  fn unknown_names_are_skipped() {
    assert_eq!(strings_to_keys(&["Unknown(42)", "KeyA"]), [Key::KeyA]);
  }

  #[test]
  fn evdev_codes_map_both_ways() {
    assert_eq!(Key::from_code(KeyCode::KEY_GRAVE), Some(Key::BackQuote));
    assert_eq!(Key::ControlLeft.code(), KeyCode::KEY_LEFTCTRL);
    assert_eq!(Key::from_code(KeyCode::BTN_LEFT), None);
  }

  #[test]
  fn a_bind_fires_once_per_press() {
    let bind = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    assert!(!state.process(&bind, Key::ControlLeft, true, now));
    assert!(state.process(&bind, Key::BackQuote, true, now));
    assert!(!state.process(&bind, Key::BackQuote, true, now));
    state.process(&bind, Key::BackQuote, false, now);
    assert!(state.process(&bind, Key::BackQuote, true, now));
  }

  #[test]
  fn extra_held_keys_prevent_a_match() {
    let bind = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    state.process(&bind, Key::ShiftLeft, true, now);
    state.process(&bind, Key::ControlLeft, true, now);
    assert!(!state.process(&bind, Key::BackQuote, true, now));
  }

  #[test]
  fn stale_keys_are_forgotten() {
    let bind = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    state.process(&bind, Key::ShiftLeft, true, now);
    let later = now + Duration::from_secs(6);
    state.process(&bind, Key::ControlLeft, true, later);
    assert!(state.process(&bind, Key::BackQuote, true, later));
  }

  #[test]
  fn repeats_keep_a_held_key_from_going_stale() {
    let bind = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    state.process(&bind, Key::ControlLeft, true, now);
    state.process(&bind, Key::ControlLeft, true, now + Duration::from_secs(4));
    let later = now + Duration::from_secs(8);
    state.process(&bind, Key::ControlLeft, true, later);
    assert!(state.process(&bind, Key::BackQuote, true, later));
  }

  #[test]
  fn an_empty_bind_cannot_be_built() {
    assert_eq!(Keybind::new(Vec::new()), None);
  }
}
