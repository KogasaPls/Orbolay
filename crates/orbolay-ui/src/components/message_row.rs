use freya::{
  html::{HtmlSource, HtmlViewer, use_html},
  prelude::*,
};

use orbolay_core::{app_state::AppHandle, protocol::BridgeCommand, protocol::Notification};

use crate::util::{
  notification_template::{load_notification_template, render_notification},
  scale::{GapsScaleExt, UiScale},
  theme::Theme,
};

#[derive(PartialEq)]
pub struct MessageRow {
  pub app: AppHandle,
  pub message: Notification,
  pub theme: Theme,
  pub box_size: (u32, u32),
  pub notification_template: Option<String>,
  pub ui_scale: UiScale,
}

impl Component for MessageRow {
  fn render_key(&self) -> DiffKey {
    if let Some(message_id) = &self.message.message_id {
      DiffKey::from(message_id)
    } else {
      DiffKey::from(&self.message.title)
    }
  }

  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let width = scale.px(self.box_size.0 as f32);
    let height = scale.px(self.box_size.1 as f32);
    let message = self.message.clone();
    let app = self.app.clone();

    let handle = use_html(|| HtmlSource::html(String::new()));
    let deps = (
      self.message.clone(),
      self.theme,
      self.notification_template.clone(),
    );

    use_side_effect_with_deps(&deps, move |deps| {
      let (message, theme, notification_template) = deps.clone();
      let template = load_notification_template(notification_template.as_deref());
      let mut handle = handle;
      handle.replace_html(render_notification(&template, &message, &theme));
    });

    rect()
      .direction(Direction::Horizontal)
      .main_align(Alignment::Center)
      .cross_align(Alignment::Center)
      .width(Size::px(width))
      .height(Size::px(height))
      .margin(Gaps::new_all(6.).scaled(scale))
      .cursor(CursorIcon::Pointer)
      .child(
        rect()
          .width(Size::px(self.box_size.0 as f32))
          .height(Size::px(self.box_size.1 as f32))
          .scale(self.ui_scale.factor())
          .child(HtmlViewer::new(handle)),
      )
      .on_press(move |_| {
        app.send(BridgeCommand::Navigate {
          guild_id: message.guild_id.clone(),
          channel_id: message.channel_id.clone(),
          message_id: message.message_id.clone(),
        });
      })
  }
}
