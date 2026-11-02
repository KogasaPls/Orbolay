use std::sync::Arc;

use freya::prelude::*;
use orbolay_core::{
  app_state::{AppHandle, AppState},
  config::Corner,
};

use crate::{
  components::{MessagesSection, SettingsButton, Soundboard, VoiceControls, VoiceSection},
  open_configurator,
  util::{scale::UiScale, theme},
  view::OverlayView,
};

/// Everything drawn on the overlay window for one state snapshot.
#[derive(PartialEq)]
pub struct Overlay {
  pub app: AppHandle,
  pub state: Arc<AppState>,
  pub view: State<OverlayView>,
}

impl Component for Overlay {
  fn render(&self) -> impl IntoElement {
    let state = &*self.state;
    let mut view = self.view;
    let app = &self.app;
    let config = &state.config;
    let open = view().is_open();
    let soundboard_open = view() == OverlayView::Soundboard;
    let ui_scale = UiScale::new(config.ui_scale);
    let theme = theme::Theme::from_values(
      theme::from_tuple(config.accent),
      theme::from_tuple(config.text_color),
      config.border_radius,
    );
    let current_user = state.me().cloned();

    let settings_alignment = Corner::unused(config.user_alignment, config.message_alignment);
    let settings_app = app.clone();
    let open_settings: EventHandler<()> = (move |()| {
      view.set(OverlayView::Closed);
      open_configurator(&settings_app);
    })
    .into();

    let voice = VoiceSection {
      app: app.clone(),
      voice_users: state.members().to_vec(),
      self_id: state.user_id.clone(),
      is_open: open,
      is_censor: state.is_censor,
      user_alignment: config.user_alignment,
      user_offset_x: config.user_offset_x,
      user_offset_y: config.user_offset_y,
      display_voice_members: config.display_voice_members,
      speaking_grace_style: config.speaking_grace_style,
      theme,
      ui_scale,
      user_template: config.user_template.clone(),
    };
    let messages = config
      .enable_message_notifications
      .then(|| MessagesSection {
        app: app.clone(),
        messages: state
          .messages
          .iter()
          .map(|shown| shown.notification.clone())
          .collect(),
        is_open: open,
        is_censor: state.is_censor,
        message_alignment: config.message_alignment,
        message_offset_x: config.message_offset_x,
        message_offset_y: config.message_offset_y,
        messages_semitransparent: config.messages_semitransparent,
        notification_template: config.notification_template.clone(),
        theme,
        ui_scale,
      });
    let soundboard = Soundboard {
      app: app.clone(),
      soundboard: state.soundboard.clone(),
      current_guild_id: state.call.as_ref().and_then(|call| call.guild_id.clone()),
      has_nitro: state.has_nitro,
      theme,
      ui_scale,
    };

    rect()
      .width(Size::fill())
      .height(Size::fill())
      // Background overlay
      .child(
        rect()
          .position(Position::new_absolute().top(0.).left(0.))
          .background(if open {
            theme.transparent_gray
          } else {
            Color::TRANSPARENT
          })
          .width(Size::fill())
          .height(Size::fill())
          .on_press(move |_| view.set(OverlayView::Closed)),
      )
      .child(voice)
      .maybe_child(messages)
      // Voice controls + soundboard
      .maybe(open, |el| {
        el
          // Transparent backdrop that catches clicks outside the popup to dismiss it
          .maybe(soundboard_open, |el| {
            el.child(
              rect()
                .position(Position::new_absolute().top(0.).left(0.))
                .width(Size::fill())
                .height(Size::fill())
                .on_press(move |_| view.set(OverlayView::Open)),
            )
          })
          .maybe_child(current_user.map(|user| {
            rect()
              .position(Position::new_absolute().top(0.).left(0.))
              .direction(Direction::Vertical)
              .main_align(Alignment::End)
              .cross_align(Alignment::Center)
              .height(Size::percent(90.0_f32))
              .width(Size::fill())
              .maybe(soundboard_open, |el| el.child(soundboard))
              .child(VoiceControls {
                user,
                app: app.clone(),
                view,
                theme,
                ui_scale,
              })
          }))
      })
      // Settings button
      .maybe(open, |el| {
        el.child(SettingsButton {
          on_click: open_settings.clone(),
          alignment: settings_alignment,
          ui_scale,
        })
      })
  }
}
