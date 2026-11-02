#![allow(clippy::borrow_interior_mutable_const)]
#![allow(clippy::declare_interior_mutable_const)]

use std::{collections::HashMap, sync::atomic::Ordering};

use freya::prelude::*;
use gumdrop::Options;
use log::{info, warn};
use native_dialog::{MessageDialogBuilder, MessageLevel};
use orbolay_control::Command;
use orbolay_core::{
  app_state::{AppHandle, AppState, SharedAppState},
  config::{Config, Corner, LoadError, load_config, save_config},
  dirs::ensure_config_dirs,
  util::bridge::BridgeMessage,
};
use orbolay_keys::KeyEvent;
use orbolay_transport::{create_transport_thread, start_config_watcher};
use orbolay_ui::{
  components::{MessagesSection, SettingsButton, Soundboard, VoiceControls, VoiceSection},
  open_configurator, open_configurator_standalone, populate_monitor_cache,
  util::{scale::UiScale, theme},
};
use winit::{
  platform::{
    wayland::WindowAttributesExtWayland,
    x11::{WindowAttributesExtX11, WindowType},
  },
  window::WindowLevel,
};

use crate::{
  display::{select_monitor, update_monitor, window_size_for_display},
  manager::OverlayManager,
  notifications::create_notification_thread,
};

mod display;
mod keys;
mod manager;
mod notifications;
mod target;
mod util;
mod window;

static TWEMOJI_FONT: &[u8] = include_bytes!("../assets/fonts/Twemoji.ttf");

const GIT_HASH: Option<&str> = option_env!("GIT_HASH");
const APP_NAME: Option<&str> = option_env!("CARGO_PKG_NAME");
const APP_VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Options)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct Args {
  #[options(help = "Display usage information")]
  help: bool,

  #[options(help = "The port to run the websocket server on (overrides the configured port)")]
  port: Option<u16>,

  #[options(help = "Print version information")]
  version: bool,

  #[options(help = "Enable various debugging features")]
  debug: bool,

  #[options(help = "Open the configuration window")]
  config: bool,

  #[options(help = "Toggle overlay visibility, then exit")]
  toggle_overlay: bool,

  #[options(help = "Toggle mute, then exit")]
  mute: bool,

  #[options(help = "Toggle deafen, then exit")]
  deafen: bool,

  #[options(help = "Target application to launch (if any)", free)]
  target: Vec<String>,
}

fn main() {
  let args = Args::parse_args_default_or_exit();

  if args.help_requested() {
    #[expect(clippy::print_stdout, reason = "command-line usage output")]
    {
      println!("{}", Args::usage());
    }
    std::process::exit(0);
  }

  if args.version {
    #[expect(clippy::print_stdout, reason = "command-line version output")]
    {
      println!(
        "{} version {} (rev {})",
        APP_NAME.unwrap_or("Unknown"),
        APP_VERSION.unwrap_or("0.0.0"),
        GIT_HASH.unwrap_or("unknown")
      );
    }
    std::process::exit(0);
  }

  init_logging();
  ensure_config_dirs();

  let (config, first_run) = match load_config() {
    Ok(config) => (config, false),
    Err(LoadError::Missing) => (Config::default(), true),
    Err(err) => {
      warn!("{err}; using the default config without overwriting it");
      (Config::default(), false)
    }
  };

  if config.software_rendering {
    unsafe { std::env::set_var("FREYA_RENDERER", "software") };
  }

  let wayland_display = std::env::var("WAYLAND_DISPLAY");

  if config.xwayland {
    unsafe { std::env::set_var("WAYLAND_DISPLAY", "") };
  }

  if args.config {
    open_configurator_standalone();
    std::process::exit(0);
  }

  if args.toggle_overlay || args.mute || args.deafen {
    let Ok(mut client) = orbolay_control::Client::connect() else {
      eprintln!("Could not connect to Orbolay. Is it running?");
      std::process::exit(1);
    };

    let commands = [
      args.toggle_overlay.then_some(Command::ToggleOverlay),
      args.mute.then_some(Command::Mute),
      args.deafen.then_some(Command::Deafen),
    ]
    .into_iter()
    .flatten();

    for cmd in commands {
      if let Err(e) = client.send(&cmd) {
        eprintln!("Failed to send command: {e}");
        std::process::exit(1);
      }
    }

    std::process::exit(0);
  }

  if util::process::is_already_running() {
    MessageDialogBuilder::default()
      .set_level(MessageLevel::Error)
      .set_title("Orbolay")
      .set_text("Orbolay is already running. Kill the existing process before starting a new one.")
      .alert()
      .show()
      .expect("Failed to show message dialog");
    std::process::exit(0);
  }

  if !config.xwayland && wayland_display.as_deref().is_ok_and(|d| !d.is_empty()) {
    warn!(
      "XWayland is disabled; the overlay may not stay on top or pass clicks through under Wayland"
    );
  }

  if !args.target.is_empty() {
    let env = HashMap::from([("WAYLAND_DISPLAY".to_string(), wayland_display.ok())]);

    info!("Launching child process: {:?}", args.target);

    target::launch_target(args.target, Some(env));
  }

  let startup = Startup {
    port: args.port.unwrap_or(config.port),
    debug: args.debug,
    first_run,
    config: config.clone(),
  };

  launch(
    LaunchConfig::new()
      .with_font("Twemoji", TWEMOJI_FONT)
      .with_fallback_font("Twemoji")
      .with_window(
        WindowConfig::new(move || app(startup.clone()))
          .with_title("orbolay")
          .with_decorations(false)
          .with_transparency(true)
          .with_background(Color::TRANSPARENT)
          .with_window_handle(|w| {
            *window::WINDOW_ID.lock().unwrap() = Some(w.id());
          })
          .with_window_attributes(move |mut w, event_loop| {
            let monitors: Vec<_> = event_loop.available_monitors().collect();
            let primary = event_loop.primary_monitor();

            populate_monitor_cache(primary.as_ref(), &monitors);

            if let Some(monitor) = select_monitor(&config, primary.as_ref(), &monitors) {
              // Compute the initial window size for the chosen display.
              w = w
                .with_inner_size(window_size_for_display(monitor))
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_position(monitor.position());
            }

            w = WindowAttributesExtX11::with_name(w, "orbolay", "orbolay")
              .with_x11_window_type(vec![WindowType::Utility])
              .with_override_redirect(true);
            WindowAttributesExtWayland::with_name(w, "orbolay", "orbolay")
          }),
      ),
  );
}

/// Logs Orbolay's own crates at `info` and dependencies at `warn` unless `RUST_LOG` overrides it.
fn init_logging() {
  const DEFAULT_FILTER: &str = "warn,orbolay=info,orbolay_control=info,orbolay_core=info,orbolay_keys=info,orbolay_transport=info,orbolay_ui=info";

  let mut builder =
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_FILTER));
  if std::env::var_os("JOURNAL_STREAM").is_some() {
    builder.format_timestamp(None);
  }
  builder.init();
}

/// Settings resolved before the window opens.
#[derive(Clone)]
struct Startup {
  port: u16,
  debug: bool,
  first_run: bool,
  config: Config,
}

fn app(startup: Startup) -> impl IntoElement {
  let Startup {
    port,
    debug,
    first_run,
    config: initial_config,
  } = startup;
  let mut app_state = use_state(AppState::new);
  let mut soundboard_open = use_state(|| false);

  let app_handle = use_hook(move || {
    let (transport_sender, transport_recv) = flume::unbounded::<BridgeMessage>();
    let (redraw_tx, redraw_rx) = flume::unbounded::<()>();
    let (action_tx, action_rx) = flume::unbounded::<KeyEvent>();
    let (open_config_tx, open_config_rx) = flume::unbounded::<()>();

    app_state.write().transport_sender = Some(transport_sender.clone());

    // Shared state for background threads
    let mut initial = AppState::new();
    initial.transport_sender = Some(transport_sender);

    initial.config = initial_config;

    let shared: SharedAppState = std::sync::Arc::new(std::sync::RwLock::new(initial));
    let app = AppHandle::new(shared.clone(), redraw_tx, open_config_tx);

    if !debug {
      window::set_clickable(false);
    }

    keys::watch_keybinds(shared.clone(), action_tx.clone());

    let ctrl_app = app.clone();

    orbolay_control::start(move |cmd| match cmd {
      Command::ToggleOverlay => {
        if !ctrl_app.read(|state| state.recording_keybind.load(Ordering::Relaxed)) {
          let _ = action_tx.send(KeyEvent::ToggleOverlay);
        }
      }
      Command::Mute => ctrl_app.send(BridgeMessage {
        cmd: "TOGGLE_MUTE".to_string(),
        data: serde_json::Value::Null,
      }),
      Command::Deafen => ctrl_app.send(BridgeMessage {
        cmd: "TOGGLE_DEAF".to_string(),
        data: serde_json::Value::Null,
      }),
    });

    create_transport_thread(app.clone(), port, transport_recv);
    create_notification_thread(app.clone());

    // sync SharedAppState -> AppState on every redraw signal
    let shared_sync = shared;
    spawn_in_window(async move {
      while redraw_rx.recv_async().await == Ok(()) {
        let synced = shared_sync.read().unwrap().clone();
        let transport_sender = app_state.read().transport_sender.clone();
        let is_open = app_state.read().is_open;
        *app_state.write() = AppState {
          transport_sender,
          is_open,
          ..synced
        };
      }
    });

    if first_run {
      let _ = app.open_config();
      app.redraw();

      // Write the config regardless so we don't trigger this in the future
      if let Err(err) = save_config(&app.read(|state| state.config.clone())) {
        warn!("Failed to save config.json: {err}");
      }
    }

    start_config_watcher(app.clone(), |old_config, new_config| {
      // Reposition the window when the target monitor changes
      if old_config.display_name != new_config.display_name
        || old_config.display_idx != new_config.display_idx
      {
        update_monitor(new_config);
      }
    });

    spawn_in_window(async move {
      while let Ok(event) = action_rx.recv_async().await {
        match event {
          KeyEvent::ToggleOverlay => OverlayManager::toggle(app_state),
        }
      }
    });

    let config_app = app.clone();
    spawn_in_window(async move {
      while open_config_rx.recv_async().await == Ok(()) {
        OverlayManager::close(app_state);
        open_configurator(&config_app);
      }
    });

    app
  });

  // Sync is_open -> cursor hit-test, and close soundboard when overlay closes
  use_side_effect(move || {
    let is_open = app_state.read().is_open;
    if !is_open {
      soundboard_open.set(false);
    }
    window::set_clickable(is_open);
  });

  let state = app_state.read();
  let voice_users = state.voice_users.clone();
  let messages = state.messages.clone();
  let messages_enabled = state.config.enable_message_notifications;
  let is_open = state.is_open;
  let is_censor = state.is_censor;
  let config = state.config.clone();
  let ui_scale = UiScale::new(config.ui_scale).factor();
  let theme = theme::Theme::from_values(
    theme::from_tuple(config.accent),
    theme::from_tuple(config.text_color),
    config.border_radius,
  );
  let current_user = state
    .voice_users
    .iter()
    .find(|u| u.id == state.user_id)
    .cloned();
  drop(state);

  let settings_alignment = Corner::unused(&[config.user_alignment, config.message_alignment])
    .unwrap_or(Corner::BottomLeft);
  let open_settings: EventHandler<()> = (move |()| {
    let _ = app_handle.open_config();
  })
  .into();

  rect()
    .width(Size::fill())
    .height(Size::fill())
    // Background overlay
    .child(
      rect()
        .position(Position::new_absolute().top(0.).left(0.))
        .background(if is_open {
          theme.transparent_gray
        } else {
          Color::TRANSPARENT
        })
        .width(Size::fill())
        .height(Size::fill())
        .on_press(move |_| {
          OverlayManager::close(app_state);
        }),
    )
    // Voice users
    .child(VoiceSection {
      voice_users,
      app_state,
      is_open,
      is_censor,
      user_alignment: config.user_alignment,
      user_offset_x: config.user_offset_x,
      user_offset_y: config.user_offset_y,
      display_voice_members: config.display_voice_members,
      speaking_grace_style: config.speaking_grace_style,
      theme,
      ui_scale,
      user_template_size: (320, 62),
      user_template: config.user_template.clone(),
    })
    // Messages
    .maybe(messages_enabled, |el| el.child(MessagesSection {
      messages,
      is_open,
      is_censor,
      message_alignment: config.message_alignment,
      message_offset_x: config.message_offset_x,
      message_offset_y: config.message_offset_y,
      messages_semitransparent: config.messages_semitransparent,
      notification_template: config.notification_template.clone(),
      app_state,
      theme,
      ui_scale,
    }))
    // Voice controls + soundboard
    .maybe(is_open, |el| {
      el
        // Transparent backdrop that catches clicks outside the popup to dismiss it
        // TODO maybe rework to be used for any sort of popup
        .maybe(*soundboard_open.read(), |el| {
          el.child(
            rect()
              .position(Position::new_absolute().top(0.).left(0.))
              .width(Size::fill())
              .height(Size::fill())
              .on_press(move |_| soundboard_open.set(false)),
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
            .maybe(*soundboard_open.read(), |el| {
              el.child(Soundboard {
                app_state,
                theme,
                ui_scale,
              })
            })
            .child(VoiceControls {
              user,
              app_state,
              soundboard_open,
              theme,
              ui_scale,
            })
        }))
    })
    // Settings button
    .maybe(is_open, |el| {
      el.child(SettingsButton {
        on_click: open_settings.clone(),
        alignment: settings_alignment,
        ui_scale,
      })
    })
}
