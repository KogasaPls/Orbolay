use std::{cell::Cell, sync::atomic::Ordering};

use freya::{prelude::*, winit::window::WindowId};
use orbolay_core::{
  app_state::{AppHandle, AppState},
  config::{Config, load_config},
};
use sections::{Section, Settings, apply_config};
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

      let window = configurator_window(app.clone(), false).with_on_close(|_, _| {
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

  launch(LaunchConfig::new().with_window(configurator_window(app, true)));
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

  let close = EventHandler::new(move |()| Platform::get().close_window(window_id));
  let close_shortcut = close.clone();

  let on_reset = EventHandler::new({
    let app = app.clone();
    move |()| apply_config(&app, |cfg| *cfg = Config::default())
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
