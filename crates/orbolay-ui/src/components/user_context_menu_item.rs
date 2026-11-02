use freya::prelude::*;
use orbolay_core::{app_state::AppHandle, protocol::BridgeCommand, user::User};

use crate::util::{
  scale::{GapsScaleExt, UiScale},
  theme::Theme,
};

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
    let mut slider_value = use_state(|| f64::from(self.user.volume.clamp(0., 200.) / 2.));

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
                  .text(format!("{:.0}%", slider_value.read().round() * 2.0)),
              ),
          )
          .child(
            Slider::new(move |value: f64| {
              let volume = (value * 2.0).clamp(0.0, 200.0);
              slider_value.set(value);
              app.send(BridgeCommand::SetUserVolume {
                user_id: user_id.clone(),
                volume,
              });
            })
            .theme(slider_theme)
            .value(*slider_value.read())
            .size(Size::fill()),
          ),
      )
  }
}
