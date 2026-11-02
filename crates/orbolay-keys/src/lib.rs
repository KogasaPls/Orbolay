//! Global keybinds: the key names stored in the config, and matching pressed keys against binds.

use std::time::{Duration, Instant};

use evdev::KeyCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
  ToggleOverlay,
}

macro_rules! keys {
  ($($key:ident = $code:ident,)*) => {
    /// A keyboard key, serialized with the names earlier versions stored in the config.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub enum Key {
      $($key,)*
    }

    impl Key {
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

pub struct Keybind {
  pub keys: Vec<Key>,
  pub event: KeyEvent,
  active: bool,
}

impl Keybind {
  pub const fn new(keys: Vec<Key>, event: KeyEvent) -> Self {
    Self {
      keys,
      event,
      active: false,
    }
  }

  fn matches(&self, pressed: &[Key]) -> bool {
    !self.keys.is_empty()
      && pressed.len() == self.keys.len()
      && self.keys.iter().all(|k| pressed.contains(k))
  }
}

/// Keys currently held across all devices.
pub struct KeyState {
  pressed: Vec<Key>,
  last_update: Instant,
}

impl Default for KeyState {
  fn default() -> Self {
    Self {
      pressed: Vec::new(),
      last_update: Instant::now(),
    }
  }
}

impl KeyState {
  /// Records a press or release and returns the binds that just became fully held.
  pub fn process(
    &mut self,
    key: Key,
    pressed: bool,
    keybinds: &mut [Keybind],
    now: Instant,
  ) -> Vec<KeyEvent> {
    if now.duration_since(self.last_update) > STALE_TIMEOUT {
      self.pressed.clear();
      for bind in &mut *keybinds {
        bind.active = false;
      }
    }
    self.last_update = now;

    if pressed {
      if !self.pressed.contains(&key) {
        self.pressed.push(key);
      }
    } else {
      self.pressed.retain(|&k| k != key);
    }

    let mut fired = Vec::new();
    for bind in keybinds {
      let matches = bind.matches(&self.pressed);
      if matches && !bind.active {
        fired.push(bind.event);
      }
      bind.active = matches;
    }
    fired
  }
}

#[cfg(test)]
mod tests {
  use std::time::{Duration, Instant};

  use evdev::KeyCode;

  use super::{Key, KeyEvent, KeyState, Keybind, keys_to_strings, strings_to_keys};

  const NONE: Vec<KeyEvent> = Vec::new();

  fn toggle() -> Vec<Keybind> {
    vec![Keybind::new(
      vec![Key::ControlLeft, Key::BackQuote],
      KeyEvent::ToggleOverlay,
    )]
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
    let mut binds = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    assert_eq!(state.process(Key::ControlLeft, true, &mut binds, now), NONE);
    assert_eq!(
      state.process(Key::BackQuote, true, &mut binds, now),
      [KeyEvent::ToggleOverlay]
    );
    assert_eq!(state.process(Key::BackQuote, true, &mut binds, now), NONE);
    state.process(Key::BackQuote, false, &mut binds, now);
    assert_eq!(
      state.process(Key::BackQuote, true, &mut binds, now),
      [KeyEvent::ToggleOverlay]
    );
  }

  #[test]
  fn extra_held_keys_prevent_a_match() {
    let mut binds = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    state.process(Key::ShiftLeft, true, &mut binds, now);
    state.process(Key::ControlLeft, true, &mut binds, now);
    assert_eq!(state.process(Key::BackQuote, true, &mut binds, now), NONE);
  }

  #[test]
  fn stale_keys_are_forgotten() {
    let mut binds = toggle();
    let mut state = KeyState::default();
    let now = Instant::now();
    state.process(Key::ShiftLeft, true, &mut binds, now);
    let later = now + Duration::from_secs(6);
    state.process(Key::ControlLeft, true, &mut binds, later);
    assert_eq!(
      state.process(Key::BackQuote, true, &mut binds, later),
      [KeyEvent::ToggleOverlay]
    );
  }

  #[test]
  fn an_empty_bind_never_fires() {
    let mut binds = vec![Keybind::new(Vec::new(), KeyEvent::ToggleOverlay)];
    let mut state = KeyState::default();
    let now = Instant::now();
    state.process(Key::KeyA, true, &mut binds, now);
    assert_eq!(state.process(Key::KeyA, false, &mut binds, now), NONE);
  }
}
