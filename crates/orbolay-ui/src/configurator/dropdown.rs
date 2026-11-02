use freya::prelude::*;

#[derive(PartialEq, Clone)]
pub struct DropdownControl {
  options: Vec<String>,
  selected: usize,
  on_change: EventHandler<usize>,
}

impl DropdownControl {
  pub fn new(options: Vec<String>, selected: usize, on_change: EventHandler<usize>) -> Self {
    Self {
      options,
      selected,
      on_change,
    }
  }
}

impl Component for DropdownControl {
  fn render(&self) -> impl IntoElement {
    let selected = self.selected;
    let selected_label = self.options.get(selected).cloned().unwrap_or_default();

    Select::new()
      .selected_item(selected_label)
      .children(
        self
          .options
          .iter()
          .enumerate()
          .map(|(i, option)| -> Element {
            let on_change = self.on_change.clone();
            MenuItem::new()
              .selected(selected == i)
              .on_press(move |_| on_change.call(i))
              .child(option.clone())
              .into()
          }),
      )
  }
}
