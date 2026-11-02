use freya::prelude::*;

#[derive(PartialEq, Clone)]
pub struct ToggleControl {
  value: bool,
  on_change: EventHandler<bool>,
}

impl ToggleControl {
  pub fn new(value: bool, on_change: EventHandler<bool>) -> Self {
    Self { value, on_change }
  }
}

impl Component for ToggleControl {
  fn render(&self) -> impl IntoElement {
    Switch::new(WritableSetter::new(self.value, self.on_change.clone()))
  }
}
