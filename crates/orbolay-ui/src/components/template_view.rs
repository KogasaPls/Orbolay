use freya::{
  html::{HtmlSource, HtmlViewer, use_html},
  prelude::*,
};

use crate::util::scale::UiScale;

/// Renders a filled HTML template at its design size, scaled by the UI scale.
#[derive(PartialEq)]
pub struct TemplateView {
  pub html: String,
  pub size: (u32, u32),
  pub ui_scale: UiScale,
}

impl TemplateView {
  /// The space the scaled template occupies in the layout.
  pub fn scaled_size(size: (u32, u32), ui_scale: UiScale) -> (Size, Size) {
    (
      Size::px(ui_scale.px(size.0 as f32)),
      Size::px(ui_scale.px(size.1 as f32)),
    )
  }
}

impl Component for TemplateView {
  fn render(&self) -> impl IntoElement {
    let handle = use_html(|| HtmlSource::html(String::new()));
    use_side_effect_with_deps(&self.html, move |html| {
      let mut handle = handle;
      handle.replace_html(html.clone());
    });

    rect()
      .width(Size::px(self.size.0 as f32))
      .height(Size::px(self.size.1 as f32))
      .scale(self.ui_scale.factor())
      .child(HtmlViewer::new(handle))
  }
}
