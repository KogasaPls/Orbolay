use std::sync::Arc;

use freya::prelude::*;
use orbolay_core::{
  app_state::{AppHandle, Soundboard as SoundboardState},
  protocol::{BridgeCommand, SoundboardSoundPayload},
};

use crate::util::{
  scale::{GapsScaleExt, UiScale},
  theme::Theme,
};

/// The current server's sounds first, then Discord's default sounds, then other servers'.
fn guild_order(guild_id: Option<&str>, current: Option<&str>) -> u8 {
  match guild_id {
    Some(_) if guild_id == current => 0,
    None => 1,
    Some(_) => 2,
  }
}

/// Sounds from other servers need Nitro.
fn usable(sound: &SoundboardSoundPayload, has_nitro: bool, current_guild_id: Option<&str>) -> bool {
  sound.available
    && (has_nitro || sound.guild_id.is_none() || sound.guild_id.as_deref() == current_guild_id)
}

#[derive(PartialEq)]
struct SoundButton {
  sound: SoundboardSoundPayload,
  available: bool,
  app: AppHandle,
  theme: Theme,
  ui_scale: UiScale,
}

impl Component for SoundButton {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let app = self.app.clone();
    let mut hovered = use_state(|| false);

    let sound_id = self.sound.sound_id.clone();
    let source_guild_id = self.sound.guild_id.clone();
    let available = self.available;
    let text = self.sound.emoji_name.clone().unwrap_or_else(|| "?".into());
    let name = &self.sound.name;

    rect()
      .direction(Direction::Horizontal)
      .cross_align(Alignment::Center)
      .main_align(Alignment::Center)
      .width(Size::percent(33.3_f32))
      .height(Size::px(scale.px(40.0)))
      .margin(Gaps::new_all(2.).scaled(scale))
      .corner_radius(CornerRadius::new_all(self.theme.border_radius))
      .maybe(!available, |el| el.opacity(0.4_f32))
      .background(if *hovered.read() {
        self.theme.light_gray
      } else {
        self.theme.darkish_gray
      })
      .on_press(move |_| {
        if available {
          app.send(BridgeCommand::PlaySoundboardSound {
            sound_id: sound_id.clone(),
            source_guild_id: source_guild_id.clone(),
          });
        }
      })
      .cursor(CursorIcon::Pointer)
      .on_pointer_enter(move |_| {
        *hovered.write() = true;
      })
      .on_pointer_leave(move |_| {
        *hovered.write() = false;
      })
      .overflow(Overflow::Clip)
      .child(
        rect()
          .direction(Direction::Horizontal)
          .main_align(Alignment::Center)
          .cross_align(Alignment::Center)
          .content(Content::wrap())
          .padding(Gaps::new_symmetric(4., 2.).scaled(scale))
          .child(
            rect()
              .padding(Gaps::new(0., 4., 0., 0.).scaled(scale))
              .child(
                label()
                  .color(self.theme.text_color)
                  .font_size(scale.px(14.))
                  .text(text),
              ),
          )
          .child(
            label()
              .font_size(scale.px(11.))
              .color(self.theme.text_color)
              .max_width(Size::fill())
              .max_lines(1)
              .text(name.clone())
              .text_overflow(TextOverflow::Ellipsis),
          ),
      )
  }
}

#[derive(PartialEq)]
struct GuildLabel {
  name: String,
  theme: Theme,
  ui_scale: UiScale,
}

impl Component for GuildLabel {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    label()
      .font_size(scale.px(11.))
      .width(Size::fill())
      .color(self.theme.text_color)
      .text(self.name.clone())
  }
}

#[derive(PartialEq)]
pub struct Soundboard {
  pub app: AppHandle,
  pub soundboard: Arc<SoundboardState>,
  pub current_guild_id: Option<String>,
  pub has_nitro: bool,
  pub theme: Theme,
  pub ui_scale: UiScale,
}

impl Component for Soundboard {
  fn render(&self) -> impl IntoElement {
    let scale = self.ui_scale;
    let current_guild_id = self.current_guild_id.as_deref();

    let mut guilds: Vec<_> = self.soundboard.sounds.iter().collect();
    guilds.sort_by(|(a, _), (b, _)| {
      guild_order(a.as_deref(), current_guild_id)
        .cmp(&guild_order(b.as_deref(), current_guild_id))
        .then(b.cmp(a))
    });

    let panel = rect()
      .direction(Direction::Vertical)
      .background(self.theme.gray)
      .corner_radius(CornerRadius::new_all(self.theme.border_radius))
      .max_width(Size::px(scale.px(400.0)))
      .margin(Gaps::new(0., 0., 8., 0.).scaled(scale));

    if guilds.is_empty() {
      return panel
        .padding(Gaps::new_all(16.).scaled(scale))
        .main_align(Alignment::Center)
        .cross_align(Alignment::Center)
        .child(
          label()
            .font_size(scale.px(13.))
            .color(self.theme.text_color)
            .text("No sounds available"),
        );
    }

    let list = guilds.into_iter().fold(
      rect()
        .direction(Direction::Vertical)
        .width(Size::fill())
        .padding(Gaps::new_all(16.).scaled(scale)),
      |col, (guild_id, sounds)| {
        let name = match guild_id {
          None => "Default".to_string(),
          Some(guild_id) => self
            .soundboard
            .guild_names
            .get(guild_id)
            .cloned()
            .unwrap_or_else(|| guild_id.clone()),
        };
        let buttons = sounds.iter().fold(
          rect()
            .direction(Direction::Horizontal)
            .content(Content::wrap())
            .width(Size::fill())
            .padding(Gaps::new(2., 0., 6., 0.).scaled(scale)),
          |row, sound| {
            row.child(SoundButton {
              available: usable(sound, self.has_nitro, current_guild_id),
              sound: sound.clone(),
              app: self.app.clone(),
              theme: self.theme,
              ui_scale: scale,
            })
          },
        );
        col
          .child(GuildLabel {
            name,
            theme: self.theme,
            ui_scale: scale,
          })
          .child(buttons)
      },
    );

    panel.height(Size::px(scale.px(220.0))).child(
      ScrollView::new()
        .width(Size::fill())
        .height(Size::fill())
        .child(list),
    )
  }
}

#[cfg(test)]
mod tests {
  use orbolay_core::protocol::SoundboardSoundPayload;

  use super::{guild_order, usable};

  fn sound(guild_id: Option<&str>, available: bool) -> SoundboardSoundPayload {
    SoundboardSoundPayload {
      sound_id: "s".into(),
      name: "s".into(),
      volume: 1.,
      emoji_id: None,
      emoji_name: None,
      guild_id: guild_id.map(Into::into),
      available,
    }
  }

  #[test]
  fn other_servers_sounds_need_nitro() {
    assert!(usable(&sound(None, true), false, Some("here")));
    assert!(usable(&sound(Some("here"), true), false, Some("here")));
    assert!(!usable(
      &sound(Some("elsewhere"), true),
      false,
      Some("here")
    ));
    assert!(usable(&sound(Some("elsewhere"), true), true, Some("here")));
    assert!(!usable(&sound(Some("here"), false), true, Some("here")));
  }

  #[test]
  fn the_current_server_sorts_first_then_default_sounds() {
    let here = Some("here");
    assert!(guild_order(here, here) < guild_order(None, here));
    assert!(guild_order(None, here) < guild_order(Some("elsewhere"), here));
    assert_eq!(guild_order(None, None), guild_order(None, here));
    assert_eq!(guild_order(Some("elsewhere"), None), 2);
  }
}
