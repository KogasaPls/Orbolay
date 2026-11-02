use std::time::Duration;

use freya::prelude::*;
use orbolay_core::{
  app_state::AppHandle,
  protocol::BridgeCommand,
  user::{MAX_VOLUME, User},
};

use crate::util::{
  scale::{GapsScaleExt, UiScale},
  theme::Theme,
};

/// How long the volume slider must rest before the new volume is sent to Discord.
const VOLUME_SEND_DELAY: Duration = Duration::from_millis(150);

#[derive(PartialEq)]
pub struct UserContextMenuItem {
  pub user: User,
  pub theme: Theme,
  pub app: AppHandle,
  pub ui_scale: UiScale,
}

impl Component for UserContextMenuItem {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let app = self.app.clone();
    let user_id = self.user.id.clone();
    // The slider runs 0-100 over Discord's 0-MAX_VOLUME range
    let to_slider = |volume: f32| f64::from(volume / MAX_VOLUME * 100.);
    let mut slider_value = use_state(|| to_slider(self.user.volume));
    let mut pending_send = use_state(|| None::<TaskHandle>);

    let menu_item_theme = MenuItemThemePartial {
      background: Some(Preference::Specific(self.theme.darkish_gray)),
      hover_background: Some(Preference::Specific(self.theme.darkish_gray)),
      select_background: Some(Preference::Specific(self.theme.darkish_gray)),
      border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
      focus_border_fill: Some(Preference::Specific(Color::TRANSPARENT)),
      corner_radius: Some(Preference::Specific(CornerRadius::new_all(
        self.theme.border_radius,
      ))),
      color: Some(Preference::Specific(self.theme.text_color)),
      select_color: Some(Preference::Specific(self.theme.text_color)),
    };

    let slider_theme = SliderThemePartial {
      background: Some(Preference::Specific(self.theme.muted_gray)),
      thumb_background: Some(Preference::Specific(self.theme.text_color)),
      thumb_inner_background: Some(Preference::Specific(self.theme.text_color)),
      border_fill: Some(Preference::Specific(self.theme.gray)),
    };

    MenuItem::new()
      .theme(menu_item_theme)
      .padding(Gaps::new_all(4.).scaled(scale))
      .child(
        rect()
          .direction(Direction::Vertical)
          .cross_align(Alignment::Start)
          .width(Size::px(scale.px(160.0)))
          .child(
            rect()
              .direction(Direction::Horizontal)
              .main_align(Alignment::SpaceBetween)
              .cross_align(Alignment::Center)
              .width(Size::fill())
              .margin(Gaps::new(0., 0., 8., 0.).scaled(scale))
              .child(
                label()
                  .font_size(scale.px(12.))
                  .color(self.theme.text_color)
                  .text("User Volume"),
              )
              .child(
                label()
                  .font_size(scale.px(12.))
                  .color(self.theme.text_color)
                  .text(format!(
                    "{:.0}%",
                    *slider_value.read() / 100. * f64::from(MAX_VOLUME)
                  )),
              ),
          )
          .child(
            Slider::new(move |value: f64| {
              slider_value.set(value);
              // Send once the slider settles instead of on every drag step
              if let Some(task) = pending_send.write().take() {
                task.try_cancel();
              }
              let (app, user_id) = (app.clone(), user_id.clone());
              pending_send.set(Some(spawn_in_window(async move {
                timer(VOLUME_SEND_DELAY).await;
                app.send(BridgeCommand::SetUserVolume {
                  user_id,
                  volume: value / 100. * f64::from(MAX_VOLUME),
                });
              })));
            })
            .theme(slider_theme)
            .value(*slider_value.read())
            .size(Size::fill()),
          ),
      )
  }
}
