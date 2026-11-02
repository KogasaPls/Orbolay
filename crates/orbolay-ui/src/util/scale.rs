use freya::prelude::Gaps;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiScale(f32);

impl UiScale {
  /// `scale` comes from a normalized config, so it is finite and positive.
  pub const fn new(scale: f32) -> Self {
    Self(scale)
  }

  pub fn factor(self) -> f32 {
    self.0
  }

  pub fn px(self, value: f32) -> f32 {
    value * self.0
  }

  #[expect(
    clippy::cast_possible_truncation,
    reason = "UI pixel sizes are far below the f32 and i32 limits"
  )]
  pub fn int(self, value: i32) -> i32 {
    (value as f32 * self.0).round() as i32
  }
}

pub trait GapsScaleExt {
  #[must_use]
  fn scaled(self, scale: UiScale) -> Self;
}

impl GapsScaleExt for Gaps {
  fn scaled(self, scale: UiScale) -> Self {
    let scale = scale.factor();
    Self::new(
      self.top() * scale,
      self.right() * scale,
      self.bottom() * scale,
      self.left() * scale,
    )
  }
}
