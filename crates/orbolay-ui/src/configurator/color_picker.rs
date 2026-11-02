use freya::prelude::*;

use crate::{configurator::commit::use_commit_on_blur, util::theme};

#[derive(PartialEq, Clone)]
pub struct ColorPickerControl {
  value: Color,
  on_change: EventHandler<Color>,
}

impl ColorPickerControl {
  pub fn new(value: Color, on_change: EventHandler<Color>) -> Self {
    Self { value, on_change }
  }
}

impl Component for ColorPickerControl {
  fn render(&self) -> impl IntoElement {
    let input_id = use_a11y();
    let on_change = self.on_change.clone();
    let value = self.value;
    let mut hex_value = use_state(|| theme::to_hex(value));

    use_side_effect_with_deps(&value, move |value| {
      hex_value.set_if_modified(theme::to_hex(*value));
    });
    use_commit_on_blur(input_id, hex_value, {
      let on_change = on_change.clone();
      move |hex| {
        if let Some(color) = theme::from_hex(&hex)
          && color != value
        {
          on_change.call(color);
        }
      }
    });

    rect()
      .direction(Direction::Horizontal)
      .cross_align(Alignment::Center)
      .child(ContextMenuViewer::new())
      .child(
        rect()
          .border(Border::new().fill(theme::MUTED_GRAY).width(1.))
          .corner_radius(5.)
          .child(
            rect()
              .margin(Gaps::new_all(1.))
              .child(ColorPicker::new(move |color| on_change.call(color)).value(value)),
          ),
      )
      .child(
        rect()
          .padding(Gaps::new(0., 0., 0., 10.))
          .child(Input::new(hex_value).a11y_id(input_id)),
      )
  }
}
