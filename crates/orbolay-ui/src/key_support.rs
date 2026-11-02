use freya::prelude::Code;
use orbolay_keys::Key;

/// Keyboard codes the settings window can record, and the config key each one stores.
const CODES: &[(Code, Key)] = &[
  (Code::AltLeft, Key::Alt),
  (Code::AltRight, Key::AltGr),
  (Code::Backspace, Key::Backspace),
  (Code::CapsLock, Key::CapsLock),
  (Code::ControlLeft, Key::ControlLeft),
  (Code::ControlRight, Key::ControlRight),
  (Code::Delete, Key::Delete),
  (Code::ArrowDown, Key::DownArrow),
  (Code::End, Key::End),
  (Code::Escape, Key::Escape),
  (Code::F1, Key::F1),
  (Code::F2, Key::F2),
  (Code::F3, Key::F3),
  (Code::F4, Key::F4),
  (Code::F5, Key::F5),
  (Code::F6, Key::F6),
  (Code::F7, Key::F7),
  (Code::F8, Key::F8),
  (Code::F9, Key::F9),
  (Code::F10, Key::F10),
  (Code::F11, Key::F11),
  (Code::F12, Key::F12),
  (Code::Home, Key::Home),
  (Code::ArrowLeft, Key::LeftArrow),
  (Code::MetaLeft, Key::MetaLeft),
  (Code::MetaRight, Key::MetaRight),
  (Code::PageDown, Key::PageDown),
  (Code::PageUp, Key::PageUp),
  (Code::Enter, Key::Return),
  (Code::ArrowRight, Key::RightArrow),
  (Code::ShiftLeft, Key::ShiftLeft),
  (Code::ShiftRight, Key::ShiftRight),
  (Code::Space, Key::Space),
  (Code::Tab, Key::Tab),
  (Code::ArrowUp, Key::UpArrow),
  (Code::Backquote, Key::BackQuote),
  (Code::Minus, Key::Minus),
  (Code::Equal, Key::Equal),
  (Code::BracketLeft, Key::LeftBracket),
  (Code::BracketRight, Key::RightBracket),
  (Code::Backslash, Key::BackSlash),
  (Code::Semicolon, Key::SemiColon),
  (Code::Quote, Key::Quote),
  (Code::Comma, Key::Comma),
  (Code::Period, Key::Dot),
  (Code::Slash, Key::Slash),
  (Code::KeyA, Key::KeyA),
  (Code::KeyB, Key::KeyB),
  (Code::KeyC, Key::KeyC),
  (Code::KeyD, Key::KeyD),
  (Code::KeyE, Key::KeyE),
  (Code::KeyF, Key::KeyF),
  (Code::KeyG, Key::KeyG),
  (Code::KeyH, Key::KeyH),
  (Code::KeyI, Key::KeyI),
  (Code::KeyJ, Key::KeyJ),
  (Code::KeyK, Key::KeyK),
  (Code::KeyL, Key::KeyL),
  (Code::KeyM, Key::KeyM),
  (Code::KeyN, Key::KeyN),
  (Code::KeyO, Key::KeyO),
  (Code::KeyP, Key::KeyP),
  (Code::KeyQ, Key::KeyQ),
  (Code::KeyR, Key::KeyR),
  (Code::KeyS, Key::KeyS),
  (Code::KeyT, Key::KeyT),
  (Code::KeyU, Key::KeyU),
  (Code::KeyV, Key::KeyV),
  (Code::KeyW, Key::KeyW),
  (Code::KeyX, Key::KeyX),
  (Code::KeyY, Key::KeyY),
  (Code::KeyZ, Key::KeyZ),
  (Code::Digit0, Key::Num0),
  (Code::Digit1, Key::Num1),
  (Code::Digit2, Key::Num2),
  (Code::Digit3, Key::Num3),
  (Code::Digit4, Key::Num4),
  (Code::Digit5, Key::Num5),
  (Code::Digit6, Key::Num6),
  (Code::Digit7, Key::Num7),
  (Code::Digit8, Key::Num8),
  (Code::Digit9, Key::Num9),
  (Code::Insert, Key::Insert),
  (Code::PrintScreen, Key::PrintScreen),
  (Code::ScrollLock, Key::ScrollLock),
  (Code::Pause, Key::Pause),
  (Code::NumLock, Key::NumLock),
  (Code::IntlBackslash, Key::IntlBackslash),
  (Code::NumpadEnter, Key::KpReturn),
  (Code::NumpadSubtract, Key::KpMinus),
  (Code::NumpadAdd, Key::KpPlus),
  (Code::NumpadMultiply, Key::KpMultiply),
  (Code::NumpadDivide, Key::KpDivide),
  (Code::Numpad0, Key::Kp0),
  (Code::Numpad1, Key::Kp1),
  (Code::Numpad2, Key::Kp2),
  (Code::Numpad3, Key::Kp3),
  (Code::Numpad4, Key::Kp4),
  (Code::Numpad5, Key::Kp5),
  (Code::Numpad6, Key::Kp6),
  (Code::Numpad7, Key::Kp7),
  (Code::Numpad8, Key::Kp8),
  (Code::Numpad9, Key::Kp9),
  (Code::NumpadDecimal, Key::KpDelete),
];

pub fn code_to_key(code: Code) -> Option<Key> {
  CODES
    .iter()
    .find_map(|(candidate, key)| (*candidate == code).then_some(*key))
}

#[cfg(test)]
mod tests {
  use orbolay_keys::Key;

  use super::CODES;

  #[test]
  fn every_bindable_key_can_be_recorded() {
    let missing: Vec<_> = Key::ALL
      .iter()
      .filter(|key| **key != Key::Function && !CODES.iter().any(|(_, k)| k == *key))
      .collect();
    assert!(
      missing.is_empty(),
      "keys without a keyboard code: {missing:?}"
    );
  }
}
