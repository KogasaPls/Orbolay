use std::{cell::Cell, sync::atomic::Ordering, time::Duration};

use freya::{prelude::*, winit::window::WindowId};
use log::warn;
use orbolay_core::{
  app_state::{AppHandle, AppState},
  config::{Config, load_config, save_config},
};
use sections::{Section, Settings};
use sidebar::sidebar;

use crate::{
  monitors::populate_monitor_cache,
  util::theme::{GRAY, MUTED_GRAY},
};

mod buttons;
mod color_picker;
mod commit;
mod dropdown;
mod input;
mod keybind;
mod sections;
mod setting;
mod sidebar;
mod toggle;

/// How long config changes must settle before they are saved, so dragging a slider or color
/// picker writes config.json once.
const SAVE_DELAY: Duration = Duration::from_millis(300);

const WIDTH: f32 = 760.;
const HEIGHT: f32 = 600.;

#[derive(Clone, Copy)]
enum ConfiguratorWindow {
  Opening,
  Open(WindowId),
}

thread_local! {
  static CONFIGURATOR: Cell<Option<ConfiguratorWindow>> = const { Cell::new(None) };
}

pub fn open_configurator(app: &AppHandle) {
  match CONFIGURATOR.get() {
    Some(ConfiguratorWindow::Open(id)) => show_window(id),
    Some(ConfiguratorWindow::Opening) => {}
    None => {
      CONFIGURATOR.set(Some(ConfiguratorWindow::Opening));

      let closing = app.clone();
      let window = configurator_window(app.clone(), false).with_on_close(move |_, _| {
        save_now(&closing);
        CONFIGURATOR.set(None);
        CloseDecision::Close
      });

      spawn_global(async move {
        let id = Platform::get().launch_window(window).await;

        if matches!(CONFIGURATOR.get(), Some(ConfiguratorWindow::Opening)) {
          CONFIGURATOR.set(Some(ConfiguratorWindow::Open(id)));
          show_window(id);
        }
      });
    }
  }
}

fn show_window(id: WindowId) {
  Platform::get().with_window(id, |window| {
    window.set_minimized(false);
    window.focus_window();
  });
}

pub fn open_configurator_standalone() {
  // Basically a blocking, standalone version of open_configurator
  let (redraw_tx, _) = flume::bounded(1);
  let state = AppState::new(load_config().unwrap_or_default());
  let (app, _commands) = AppHandle::new(state, redraw_tx);
  save_config_changes(&app);

  launch(LaunchConfig::new().with_window(configurator_window(app.clone(), true)));
  save_now(&app);
}

/// Saves config changes to config.json once they settle, off the UI thread.
pub fn save_config_changes(app: &AppHandle) {
  let changes = app.subscribe_config();
  let spawned = std::thread::Builder::new()
    .name("orbolay-config-saver".into())
    .spawn(move || {
      while let Ok(mut config) = changes.recv() {
        while let Ok(newer) = changes.recv_timeout(SAVE_DELAY) {
          config = newer;
        }
        save(&config);
      }
    });
  if let Err(e) = spawned {
    warn!("Could not start the config saver; settings will not be saved: {e}");
  }
}

/// Saves a change still waiting out [`SAVE_DELAY`], for when the settings window closes.
fn save_now(app: &AppHandle) {
  save(&app.read(|state| state.config.clone()));
}

fn save(config: &Config) {
  if let Err(e) = save_config(config) {
    warn!("Failed to save config.json: {e}");
  }
}

fn configurator_window(app: AppHandle, standalone: bool) -> WindowConfig {
  WindowConfig::new(move || configurator(app.clone(), standalone))
    .with_background(GRAY)
    .with_size(f64::from(WIDTH), f64::from(HEIGHT))
    .with_min_size(f64::from(WIDTH), 400.)
    .with_title("Orbolay Configurator")
    .with_window_attributes(|window, event_loop| {
      let monitors: Vec<_> = event_loop.available_monitors().collect();
      let primary = event_loop.primary_monitor();
      populate_monitor_cache(primary.as_ref(), &monitors);
      window
    })
}

fn configurator(app: AppHandle, standalone: bool) -> impl IntoElement {
  use_init_theme(dark_theme);

  // Make the recording flag available to KeybindControl
  let recording_flag = app.recording_keybind().clone();
  let recording = recording_flag.clone();
  use_provide_context(move || recording_flag);

  // The window shows the shared config and follows every change to it, including edits made
  // to config.json while the window is open
  let mut config = use_state(|| app.read(|state| state.config.clone()));
  use_hook({
    let app = app.clone();
    move || {
      let changes = app.subscribe_config();
      spawn(async move {
        while let Ok(changed) = changes.recv_async().await {
          config.set(changed);
        }
      });
    }
  });
  let section = use_state(|| Section::General);
  let window_id = use_hook(Platform::window_id);

  // `close_window` skips the `with_on_close` hook, so forget the window here.
  let closing = app.clone();
  let close = EventHandler::new(move |()| {
    save_now(&closing);
    CONFIGURATOR.set(None);
    Platform::get().close_window(window_id);
  });
  let close_shortcut = close.clone();

  let on_reset = EventHandler::new({
    let app = app.clone();
    move |()| {
      app.set_config(|cfg| *cfg = Config::default());
    }
  });

  let current = section();
  let settings = Settings {
    app,
    config: config.read().clone(),
    standalone,
  };

  let page = rect()
    .width(Size::fill())
    .padding(Gaps::new(24., 24., 4., 24.))
    .child(
      label()
        .text(current.title())
        .color(Color::WHITE)
        .font_size(22.)
        .font_weight(FontWeight::BOLD),
    )
    .child(
      label()
        .text(current.description())
        .color(MUTED_GRAY)
        .font_size(13.)
        .margin(Gaps::new(2., 0., 20., 0.)),
    )
    .children(settings.groups(current));

  rect()
    .width(Size::fill())
    .height(Size::fill())
    .background(GRAY)
    .direction(Direction::Horizontal)
    .content(Content::Flex)
    .on_global_key_down(move |e: Event<KeyboardEventData>| {
      let is_close = e.modifiers.contains(Modifiers::ctrl_or_meta()) && e.code == Code::KeyW;
      if is_close && !recording.load(Ordering::Relaxed) {
        close_shortcut.call(());
      }
    })
    .child(sidebar(section, on_reset, close))
    .child(
      ScrollView::new()
        .key(current)
        .width(Size::flex(1.))
        .child(page),
    )
}
