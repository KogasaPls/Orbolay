use freya::prelude::*;

use orbolay_core::{app_state::AppState, config::Corner, payloads::Notification};

use crate::{
  components::MessageRow,
  config::{corner_gaps, to_alignment},
  util::{scale::UiScale, theme::Theme},
};

#[derive(PartialEq)]
pub struct MessagesSection {
  pub messages: Vec<Notification>,
  pub is_open: bool,
  pub is_censor: bool,
  pub message_alignment: Corner,
  pub message_offset_x: i32,
  pub message_offset_y: i32,
  pub messages_semitransparent: bool,
  pub notification_template: Option<String>,
  pub app_state: State<AppState>,
  pub theme: Theme,
  pub ui_scale: f32,
}

impl Component for MessagesSection {
  fn render(&self) -> impl IntoElement {
    let scale = UiScale::new(self.ui_scale);
    let (x, y) = self.message_alignment.axes();
    let gaps = corner_gaps(
      self.message_alignment,
      scale.int(self.message_offset_x) as f32,
      scale.int(self.message_offset_y) as f32,
    );
    let opacity = if self.messages_semitransparent && !self.is_open {
      0.5_f32
    } else {
      1.0_f32
    };

    self.messages.iter().fold(
      rect()
        .direction(Direction::Vertical)
        .cross_align(to_alignment(x))
        .main_align(to_alignment(y))
        .position(Position::new_absolute().top(0.).left(0.))
        .background(Color::TRANSPARENT)
        .height(Size::fill())
        .width(Size::fill())
        .padding(gaps)
        .opacity(opacity),
      |el, message| {
        if self.is_censor {
          el
        } else {
          let has_actions = message.actions.as_ref().is_some_and(|a| !a.is_empty());

          el.child(MessageRow {
            app_state: self.app_state,
            message: message.clone(),
            theme: self.theme,
            box_size: if has_actions { (400, 100) } else { (400, 66) },
            notification_template: self.notification_template.clone(),
            ui_scale: scale.factor(),
          })
        }
      },
    )
  }
}
