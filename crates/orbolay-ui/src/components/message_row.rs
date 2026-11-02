use freya::prelude::*;
use orbolay_core::{
  app_state::AppHandle,
  protocol::{BridgeCommand, Notification},
};

use crate::{
  components::TemplateView,
  util::{
    notification_template::{self, load_notification_template, render_notification},
    scale::{GapsScaleExt, UiScale},
    theme::Theme,
  },
};

#[derive(PartialEq)]
pub struct MessageRow {
  pub app: AppHandle,
  pub message: Notification,
  pub theme: Theme,
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
    let (width, height) = TemplateView::scaled_size(notification_template::SIZE, scale);
    let message = self.message.clone();
    let app = self.app.clone();
    let template = load_notification_template(self.notification_template.as_deref());
    let html = render_notification(&template, &self.message, &self.theme);

    rect()
      .direction(Direction::Horizontal)
      .main_align(Alignment::Center)
      .cross_align(Alignment::Center)
      .width(width)
      .height(height)
      .margin(Gaps::new_all(6.).scaled(scale))
      .cursor(CursorIcon::Pointer)
      .child(TemplateView {
        html,
        size: notification_template::SIZE,
        ui_scale: scale,
      })
      .on_press(move |_| {
        app.send(BridgeCommand::Navigate {
          guild_id: message.guild_id.clone(),
          channel_id: message.channel_id.clone(),
          message_id: message.message_id.clone(),
        });
      })
  }
}
