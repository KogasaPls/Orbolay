use freya::prelude::*;

use crate::configurator::commit::use_commit_on_blur;

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
    let mut text = use_state(|| self.value.clone());

    // Show the saved value again when it changes elsewhere, for example after a reset
    use_side_effect_with_deps(&self.value, move |value| {
      text.set_if_modified(value.clone());
    });
    use_commit_on_blur(id, text, move |text| on_change.call(text));

    Input::new(text).a11y_id(id).enabled(self.enabled)
  }
}
