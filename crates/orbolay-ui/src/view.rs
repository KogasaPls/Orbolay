/// What the overlay shows: closed and click-through, open, or open with the soundboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayView {
  #[default]
  Closed,
  Open,
  Soundboard,
}

impl OverlayView {
  pub fn is_open(self) -> bool {
    self != Self::Closed
  }

  #[must_use]
  pub fn toggled(self) -> Self {
    if self.is_open() {
      Self::Closed
    } else {
      Self::Open
    }
  }

  #[must_use]
  pub fn soundboard_toggled(self) -> Self {
    if self == Self::Soundboard {
      Self::Open
    } else {
      Self::Soundboard
    }
  }
}

#[cfg(test)]
mod tests {
  use super::OverlayView::{Closed, Open, Soundboard};

  #[test]
  fn toggling_closes_the_soundboard_with_the_overlay() {
    assert_eq!(Closed.toggled(), Open);
    assert_eq!(Open.toggled(), Closed);
    assert_eq!(Soundboard.toggled(), Closed);
    assert_eq!(Open.soundboard_toggled(), Soundboard);
    assert_eq!(Soundboard.soundboard_toggled(), Open);
  }
}
