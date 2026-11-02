use std::sync::Arc;

use freya::prelude::*;

use orbolay_core::{
  app_state::{AppHandle, Soundboard as SoundboardState},
  payloads::SoundboardSoundPayload,
  protocol::BridgeCommand,
};

use crate::util::{
  scale::{GapsScaleExt, UiScale},
  theme::Theme,
};

fn guild_order(guild_id: &str, current: &str) -> u8 {
  if !current.is_empty() && guild_id == current {
    0
  } else if guild_id.is_empty() {
    1
  } else {
    2
  }
}

/// Sounds from other servers need Nitro.
fn usable(sound: &SoundboardSoundPayload, has_nitro: bool, current_guild_id: &str) -> bool {
  sound.available
    && (has_nitro
      || sound
        .guild_id
        .as_deref()
        .is_none_or(|guild_id| guild_id == current_guild_id))
}

#[derive(PartialEq)]
struct SoundButton {
  sound: SoundboardSoundPayload,
  available: bool,
  app: AppHandle,
  theme: Theme,
  ui_scale: f32,
}

impl Component for SoundButton {
  fn render(&self) -> impl IntoElement {
    let scale = UiScale::new(self.ui_scale);
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
      .margin(Gaps::new_all(2.).scaled(scale.factor()))
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
          .padding(Gaps::new_symmetric(4., 2.).scaled(scale.factor()))
          .child(
            rect()
              .padding(Gaps::new(0., 4., 0., 0.).scaled(scale.factor()))
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
  ui_scale: f32,
}

impl Component for GuildLabel {
  fn render(&self) -> impl IntoElement {
    let scale = UiScale::new(self.ui_scale);
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
  pub current_guild_id: String,
  pub has_nitro: bool,
  pub theme: Theme,
  pub ui_scale: f32,
}

impl Component for Soundboard {
  fn render(&self) -> impl IntoElement {
    let scale = UiScale::new(self.ui_scale);
    let current_guild_id = &self.current_guild_id;

    let mut guilds: Vec<_> = self.soundboard.sounds.iter().collect();
    guilds.sort_by(|(a, _), (b, _)| {
      guild_order(a, current_guild_id)
        .cmp(&guild_order(b, current_guild_id))
        .then(b.cmp(a))
    });

    let panel = rect()
      .direction(Direction::Vertical)
      .background(self.theme.gray)
      .corner_radius(CornerRadius::new_all(self.theme.border_radius))
      .max_width(Size::px(scale.px(400.0)))
      .margin(Gaps::new(0., 0., 8., 0.).scaled(scale.factor()));

    if guilds.is_empty() {
      return panel
        .padding(Gaps::new_all(16.).scaled(scale.factor()))
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
        .padding(Gaps::new_all(16.).scaled(scale.factor())),
      |col, (guild_id, sounds)| {
        let name = if guild_id.is_empty() {
          "Default".to_string()
        } else {
          self
            .soundboard
            .guild_names
            .get(guild_id)
            .cloned()
            .unwrap_or_else(|| guild_id.clone())
        };
        let buttons = sounds.iter().fold(
          rect()
            .direction(Direction::Horizontal)
            .content(Content::wrap())
            .width(Size::fill())
            .padding(Gaps::new(2., 0., 6., 0.).scaled(scale.factor())),
          |row, sound| {
            row.child(SoundButton {
              available: usable(sound, self.has_nitro, current_guild_id),
              sound: sound.clone(),
              app: self.app.clone(),
              theme: self.theme,
              ui_scale: scale.factor(),
            })
          },
        );
        col
          .child(GuildLabel {
            name,
            theme: self.theme,
            ui_scale: scale.factor(),
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
