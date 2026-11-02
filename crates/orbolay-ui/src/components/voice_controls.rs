use freya::prelude::*;
use orbolay_core::{app_state::AppHandle, protocol::BridgeCommand, user::User};

use crate::util::{
  scale::{GapsScaleExt, UiScale},
  theme::{self, Theme},
};

static DEAFENED_SVG: &[u8] = include_bytes!("../../../../assets/deafened.svg");
static DEAFEN_SVG: &[u8] = include_bytes!("../../../../assets/deafen.svg");
static MUTED_SVG: &[u8] = include_bytes!("../../../../assets/muted.svg");
static MUTE_SVG: &[u8] = include_bytes!("../../../../assets/mute.svg");
static DISCONNECT_SVG: &[u8] = include_bytes!("../../../../assets/disconnect.svg");
static STOP_STREAM_SVG: &[u8] = include_bytes!("../../../../assets/stopstream.svg");
static SOUNDBOARD_SVG: &[u8] = include_bytes!("../../../../assets/speaker.svg");

#[derive(PartialEq)]
struct ControlButton {
  icon: &'static [u8],
  is_red: bool,
  on_click: EventHandler<()>,
  theme: Theme,
  ui_scale: UiScale,
}

impl Component for ControlButton {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let mut hovered = use_state(|| false);
    let is_red = self.is_red;
    let icon = self.icon;
    let on_click = self.on_click.clone();

    rect()
      .direction(Direction::Vertical)
      .main_align(Alignment::Center)
      .cross_align(Alignment::Center)
      .height(Size::fill())
      .width(Size::percent(20.0_f32))
      .margin(Gaps::new_all(6.0_f32).scaled(scale))
      .padding(Gaps::new_all(6.0_f32).scaled(scale))
      .corner_radius(CornerRadius::new_all(self.theme.border_radius))
      .background(if *hovered.read() {
        if is_red {
          theme::RED_GRAY
        } else {
          self.theme.light_gray
        }
      } else {
        Color::TRANSPARENT
      })
      .cursor(CursorIcon::Pointer)
      .on_press(move |_| on_click.call(()))
      .on_pointer_enter(move |_| {
        *hovered.write() = true;
      })
      .on_pointer_leave(move |_| {
        *hovered.write() = false;
      })
      .child(
        SvgViewer::new(icon)
          .width(Size::px(scale.px(24.0)))
          .height(Size::px(scale.px(24.0))),
      )
  }
}

#[derive(PartialEq)]
pub struct VoiceControls {
  pub user: User,
  pub app: AppHandle,
  pub soundboard_open: State<bool>,
  pub theme: Theme,
  pub ui_scale: UiScale,
}

impl Component for VoiceControls {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let send = |command: BridgeCommand| -> EventHandler<()> {
      let app = self.app.clone();
      (move |()| app.send(command.clone())).into()
    };
    let mut soundboard_open = self.soundboard_open;
    let is_muted = self.user.muted;
    let is_deafened = self.user.deafened;
    let is_streaming = self.user.streaming;

    rect()
      .direction(Direction::Horizontal)
      .main_align(Alignment::Center)
      .cross_align(Alignment::Center)
      .height(Size::auto())
      .max_height(Size::px(scale.px(60.0)))
      .max_width(Size::px(scale.px(400.0)))
      .background(self.theme.gray)
      .corner_radius(CornerRadius::new_all(self.theme.border_radius))
      .child(ControlButton {
        icon: if is_muted || is_deafened {
          MUTED_SVG
        } else {
          MUTE_SVG
        },
        is_red: is_muted || is_deafened,
        on_click: send(BridgeCommand::ToggleMute),
        theme: self.theme,
        ui_scale: scale,
      })
      .child(ControlButton {
        icon: if is_deafened {
          DEAFENED_SVG
        } else {
          DEAFEN_SVG
        },
        is_red: is_deafened,
        on_click: send(BridgeCommand::ToggleDeaf),
        theme: self.theme,
        ui_scale: scale,
      })
      .child(ControlButton {
        icon: SOUNDBOARD_SVG,
        is_red: false,
        on_click: (move |()| {
          let is_open = *soundboard_open.read();
          soundboard_open.set(!is_open);
        })
        .into(),
        theme: self.theme,
        ui_scale: scale,
      })
      .child(ControlButton {
        icon: DISCONNECT_SVG,
        is_red: true,
        on_click: send(BridgeCommand::Disconnect),
        theme: self.theme,
        ui_scale: scale,
      })
      .maybe(is_streaming, |el| {
        el.child(ControlButton {
          icon: STOP_STREAM_SVG,
          is_red: true,
          on_click: send(BridgeCommand::StopStream),
          theme: self.theme,
          ui_scale: scale,
        })
      })
  }
}
