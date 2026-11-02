use std::str::FromStr;

use freya::prelude::*;
use log::warn;
use orbolay_core::{
  app_state::AppHandle,
  config::{
    Config, Corner, DisplayVoiceMembers, SpeakingGraceStyle, TEMPLATES_DIR, config_dir,
    ensure_config_dirs, save_config,
  },
  protocol::Notification,
};
use orbolay_keys::{keys_to_strings, strings_to_keys};

use super::{
  buttons::wide_button, color_picker::ColorPickerControl, dropdown::DropdownControl,
  input::InputControl, keybind::KeybindControl, setting::SettingRow, toggle::ToggleControl,
};
use crate::{
  monitors::monitors_list,
  util::{
    notification_template::list_notification_templates,
    theme::{DARKISH_GRAY, LIGHT_GRAY, SUPERLIGHT_GRAY, from_tuple, to_tuple},
    user_template::list_user_templates,
  },
};

const UI_SCALE_OPTIONS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];

const DEFAULT_TEMPLATE: &str = "Default";

fn ui_scale_label(scale: f32) -> String {
  format!("{:.0}%", scale * 100.0)
}

/// Dropdown labels for every value of a settings enum and the position of `current`.
fn choices<T: Copy + PartialEq>(
  all: &[T],
  current: T,
  label: fn(T) -> &'static str,
) -> (Vec<String>, usize) {
  (
    all.iter().map(|value| label(*value).to_string()).collect(),
    all.iter().position(|value| *value == current).unwrap_or(0),
  )
}

fn open_templates_folder() {
  let Some(dir) = config_dir() else {
    warn!("Could not resolve the config directory");
    return;
  };
  let templates_dir = dir.join(TEMPLATES_DIR);

  ensure_config_dirs();

  if let Err(err) = open::that(&templates_dir) {
    warn!("Failed to open templates folder: {err}");
  }
}

/// Changes the shared config, which the open settings window follows, and saves it.
pub fn apply_config(app: &AppHandle, update: impl FnOnce(&mut Config)) {
  let updated = app.set_config(update);
  if let Err(err) = save_config(&updated) {
    warn!("Failed to save config.json: {err}");
  }
}

fn divider() -> impl IntoElement {
  rect()
    .width(Size::fill())
    .height(Size::px(1.0_f32))
    .background(LIGHT_GRAY)
}

fn card(rows: impl IntoIterator<Item = Element>) -> Element {
  let mut body = rect()
    .width(Size::fill())
    .margin(Gaps::new(0., 0., 20., 0.))
    .corner_radius(10.)
    .background(DARKISH_GRAY);

  for (idx, row) in rows.into_iter().enumerate() {
    if idx > 0 {
      body = body.child(divider());
    }
    body = body.child(row);
  }

  body.into_element()
}

fn group(title: &'static str, rows: impl IntoIterator<Item = Element>) -> Element {
  rect()
    .width(Size::fill())
    .child(
      label()
        .text(title)
        .color(SUPERLIGHT_GRAY)
        .font_size(12.)
        .font_weight(FontWeight::BOLD)
        .margin(Gaps::new(0., 0., 8., 4.)),
    )
    .child(card(rows))
    .into_element()
}

fn row(
  name: &str,
  description: Option<&str>,
  control: impl Into<Element>,
  disabled: bool,
) -> Element {
  SettingRow {
    name: name.into(),
    description: description.map(Into::into),
    control: control.into(),
    disabled,
  }
  .into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
  General,
  Voice,
  Notifications,
  Appearance,
  Connection,
  Advanced,
}

impl Section {
  pub const ALL: [Self; 6] = [
    Self::General,
    Self::Voice,
    Self::Notifications,
    Self::Appearance,
    Self::Connection,
    Self::Advanced,
  ];

  pub fn title(self) -> &'static str {
    match self {
      Self::General => "General",
      Self::Voice => "Voice",
      Self::Notifications => "Notifications",
      Self::Appearance => "Appearance",
      Self::Connection => "Connection",
      Self::Advanced => "Advanced",
    }
  }

  pub fn description(self) -> &'static str {
    match self {
      Self::General => "How the overlay opens and where it appears",
      Self::Voice => "Who is shown in the voice list, and where",
      Self::Notifications => "Popups for incoming messages",
      Self::Appearance => "Colors, shapes and templates",
      Self::Connection => "How Orbolay connects to Discord",
      Self::Advanced => "Rendering options",
    }
  }
}

pub struct Settings {
  pub app: AppHandle,
  pub config: Config,
  pub standalone: bool,
}

impl Settings {
  pub fn groups(&self, section: Section) -> Vec<Element> {
    match section {
      Section::General => self.general(),
      Section::Voice => self.voice(),
      Section::Notifications => self.notifications(),
      Section::Appearance => self.appearance(),
      Section::Connection => self.connection(),
      Section::Advanced => self.advanced(),
    }
  }

  fn updater<T: 'static>(&self, update: impl Fn(&mut Config, T) + 'static) -> EventHandler<T> {
    let app = self.app.clone();
    EventHandler::new(move |value| apply_config(&app, |cfg| update(cfg, value)))
  }

  fn toggle(
    &self,
    name: &str,
    description: &str,
    value: bool,
    update: impl Fn(&mut Config, bool) + 'static,
  ) -> Element {
    row(
      name,
      Some(description),
      ToggleControl::new(value, self.updater(update)),
      false,
    )
  }

  fn dropdown(
    &self,
    name: &str,
    description: &str,
    options: Vec<String>,
    selected: usize,
    update: impl Fn(&mut Config, usize) + 'static,
  ) -> Element {
    row(
      name,
      Some(description),
      DropdownControl::new(options, selected, self.updater(update)),
      false,
    )
  }

  fn number<T: FromStr + ToString + Copy + 'static>(
    &self,
    name: &str,
    description: Option<&str>,
    value: T,
    update: impl Fn(&mut Config, T) + 'static,
  ) -> Element {
    row(
      name,
      description,
      InputControl::new(
        value.to_string(),
        self.updater(move |cfg, text: String| {
          if let Ok(n) = text.trim().parse::<T>() {
            update(cfg, n);
          }
        }),
      ),
      false,
    )
  }

  fn color(
    &self,
    name: &str,
    description: &str,
    value: (u8, u8, u8),
    update: impl Fn(&mut Config, Color) + 'static,
  ) -> Element {
    row(
      name,
      Some(description),
      ColorPickerControl::new(from_tuple(value), self.updater(update)),
      false,
    )
  }

  fn choice<T: Copy + PartialEq + 'static>(
    &self,
    name: &str,
    description: &str,
    all: &'static [T],
    current: T,
    label: fn(T) -> &'static str,
    update: impl Fn(&mut Config, T) + 'static,
  ) -> Element {
    let (options, selected) = choices(all, current, label);
    self.dropdown(name, description, options, selected, move |cfg, idx| {
      update(cfg, all[idx]);
    })
  }

  fn alignment(
    &self,
    name: &str,
    description: &str,
    value: Corner,
    update: impl Fn(&mut Config, Corner) + 'static,
  ) -> Element {
    self.choice(
      name,
      description,
      &Corner::ALL,
      value,
      Corner::label,
      update,
    )
  }

  fn template(
    &self,
    name: &str,
    description: &str,
    templates: Vec<String>,
    value: Option<&str>,
    update: impl Fn(&mut Config, Option<String>) + 'static,
  ) -> Element {
    let selected = value
      .and_then(|value| templates.iter().position(|t| t == value))
      .map_or(0, |idx| idx + 1);
    let options = std::iter::once(DEFAULT_TEMPLATE.to_string())
      .chain(templates.iter().cloned())
      .collect();

    self.dropdown(name, description, options, selected, move |cfg, idx| {
      update(cfg, idx.checked_sub(1).map(|idx| templates[idx].clone()));
    })
  }

  fn keybind_group(&self) -> Element {
    let config = &self.config;
    let enabled = config.keybind_enabled;
    let keys = strings_to_keys(&config.overlay_keybind);

    group(
      "Keybind",
      [
        self.toggle(
          "Enable Keybind",
          "Toggle overlay visibility with a keybind",
          enabled,
          |cfg, value| cfg.keybind_enabled = value,
        ),
        row(
          "Overlay Keybind",
          Some("The keybind used to open the overlay"),
          KeybindControl::new(
            keys,
            self.updater(|cfg, keys: Vec<_>| cfg.overlay_keybind = keys_to_strings(&keys)),
            enabled,
          ),
          !enabled,
        ),
      ],
    )
  }

  fn general(&self) -> Vec<Element> {
    let config = &self.config;

    let all_displays = monitors_list();
    let display_names: Vec<String> = all_displays
      .iter()
      .map(|d| format!("{} ({}x{})", d.name, d.width, d.height))
      .collect();
    let names: Vec<_> = all_displays.iter().map(|d| d.native_name.clone()).collect();
    let primary = all_displays.iter().position(|d| d.is_primary);
    let selected_display = config.select_display(&names, primary).unwrap_or(0);
    let selected_scale = (0..UI_SCALE_OPTIONS.len())
      .min_by(|&a, &b| {
        let distance = |i: usize| (UI_SCALE_OPTIONS[i] - config.ui_scale).abs();
        distance(a).total_cmp(&distance(b))
      })
      .unwrap_or(0);

    Vec::from([
      self.keybind_group(),
      group(
        "Overlay",
        [
          self.dropdown(
            "Display",
            "The display to show the overlay on",
            display_names,
            selected_display,
            move |cfg, idx| {
              cfg.display_name.clone_from(&all_displays[idx].native_name);
              cfg.display_idx = cfg.display_name.is_none().then_some(idx);
            },
          ),
          self.dropdown(
            "UI Scale",
            "Scale of the overlay elements",
            UI_SCALE_OPTIONS
              .iter()
              .map(|scale| ui_scale_label(*scale))
              .collect(),
            selected_scale,
            |cfg, idx| cfg.ui_scale = UI_SCALE_OPTIONS[idx],
          ),
        ],
      ),
    ])
  }

  fn voice(&self) -> Vec<Element> {
    let config = &self.config;
    let (grace_styles, grace_style) = choices(
      &SpeakingGraceStyle::ALL,
      config.speaking_grace_style,
      SpeakingGraceStyle::label,
    );

    vec![
      group(
        "Visibility",
        [
          self.choice(
            "Display Voice Members",
            "Control when and how voice users are visible",
            &DisplayVoiceMembers::ALL,
            config.display_voice_members,
            DisplayVoiceMembers::label,
            |cfg, value| cfg.display_voice_members = value,
          ),
          self.number(
            "Speaking Grace Period (ms)",
            Some(
              "How long users keep showing after they stop talking, up to 5000. Values below 300 may flicker",
            ),
            config.speaking_grace_ms,
            |cfg, n: u32| cfg.speaking_grace_ms = n,
          ),
          row(
            "Grace Period Style",
            Some("How users look during the grace period. Only applies to \"only when speaking\""),
            DropdownControl::new(
              grace_styles,
              grace_style,
              self.updater(|cfg, idx: usize| {
                cfg.speaking_grace_style = SpeakingGraceStyle::ALL[idx];
              }),
            ),
            config.display_voice_members != DisplayVoiceMembers::WhenSpeaking,
          ),
        ],
      ),
      group(
        "Position",
        [
          self.alignment(
            "Voice Alignment",
            "Screen position for voice users",
            config.user_alignment,
            |cfg, v| cfg.user_alignment = v,
          ),
          self.number(
            "X Offset (px)",
            None,
            config.user_offset_x,
            |cfg, n| cfg.user_offset_x = n,
          ),
          self.number(
            "Y Offset (px)",
            None,
            config.user_offset_y,
            |cfg, n| cfg.user_offset_y = n,
          ),
        ],
      ),
    ]
  }

  fn notifications(&self) -> Vec<Element> {
    let config = &self.config;

    let mut behavior = vec![
      self.toggle(
        "Enable Notifications",
        "Show notifications for incoming messages",
        config.enable_message_notifications,
        |cfg, value| cfg.enable_message_notifications = value,
      ),
      self.toggle(
        "Semi-Transparent Notifications",
        "Fade notifications when the overlay is closed",
        config.messages_semitransparent,
        |cfg, value| cfg.messages_semitransparent = value,
      ),
    ];

    if !self.standalone {
      let app = self.app.clone();
      behavior.push(wide_button("Send Test Notification", move |_| {
        app.notify(Notification {
          title: "Test Notification".into(),
          body: "This is a test notification, triggered manually from the configuration window!"
            .into(),
          ..Default::default()
        });
      }));
    }

    vec![
      group("Behavior", behavior),
      group(
        "Position",
        [
          self.alignment(
            "Notification Alignment",
            "Screen position for notifications",
            config.message_alignment,
            |cfg, v| cfg.message_alignment = v,
          ),
          self.number("X Offset (px)", None, config.message_offset_x, |cfg, n| {
            cfg.message_offset_x = n;
          }),
          self.number("Y Offset (px)", None, config.message_offset_y, |cfg, n| {
            cfg.message_offset_y = n;
          }),
        ],
      ),
    ]
  }

  fn appearance(&self) -> Vec<Element> {
    let config = &self.config;

    vec![
      group(
        "Theme",
        [
          self.color(
            "Accent Color",
            "The accent color for the overlay",
            config.accent,
            |cfg, v| cfg.accent = to_tuple(v),
          ),
          self.color(
            "Text Color",
            "The text color for the overlay",
            config.text_color,
            |cfg, v| cfg.text_color = to_tuple(v),
          ),
          self.number(
            "Border Radius (px)",
            Some("Corner radius used by panels and buttons"),
            config.border_radius,
            |cfg, n: f32| cfg.border_radius = n,
          ),
        ],
      ),
      group(
        "Templates",
        [
          self.template(
            "User Template",
            "Template used to render voice users.",
            list_user_templates(),
            config.user_template.as_deref(),
            |cfg, v| cfg.user_template = v,
          ),
          self.template(
            "Notification Template",
            "Template used to render notifications.",
            list_notification_templates(),
            config.notification_template.as_deref(),
            |cfg, v| cfg.notification_template = v,
          ),
          wide_button("Open Templates Folder", |_| {
            open_templates_folder();
          }),
        ],
      ),
    ]
  }

  fn connection(&self) -> Vec<Element> {
    let config = &self.config;

    vec![card([self.number(
      "Websocket Port",
      Some("Port the websocket server listens on. Requires Restart."),
      config.port,
      |cfg, n: u16| cfg.port = n,
    )])]
  }

  fn advanced(&self) -> Vec<Element> {
    let config = &self.config;

    vec![card([
      self.toggle(
        "Force Software Renderer",
        "Use software rendering instead of hardware acceleration. Requires restart.",
        config.software_rendering,
        |cfg, value| cfg.software_rendering = value,
      ),
      self.toggle(
        "Run with XWayland",
        "Create the window under XWayland. Requires restart.",
        config.xwayland,
        |cfg, value| cfg.xwayland = value,
      ),
    ])]
  }
}
