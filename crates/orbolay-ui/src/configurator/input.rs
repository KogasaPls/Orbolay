use freya::prelude::*;

use crate::configurator::commit::use_setting_text;

#[derive(PartialEq, Clone)]
pub struct InputControl {
  value: String,
  on_change: EventHandler<String>,
  enabled: bool,
}

impl InputControl {
  pub fn new(value: String, on_change: EventHandler<String>, enabled: bool) -> Self {
    Self {
      value,
      on_change,
      enabled,
    }
  }
}

impl Component for InputControl {
  fn render(&self) -> impl IntoElement {
    let id = use_a11y();
    let on_change = self.on_change.clone();
    let text = use_setting_text(id, &self.value, move |text| on_change.call(text));

    Input::new(text).a11y_id(id).enabled(self.enabled)
  }
}
