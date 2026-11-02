use std::sync::{Arc, atomic::Ordering};

use freya::prelude::*;
use gumdrop::Options;
use log::{info, warn};
use orbolay_core::{
  app_state::{AppHandle, AppState},
  config::{Config, LoadError, ensure_config_dirs, load_config, save_config},
  protocol::BridgeCommand,
};
use orbolay_ui::{
  components::Overlay, open_configurator, open_configurator_standalone, view::OverlayView,
};
use winit::{
  platform::{
    wayland::WindowAttributesExtWayland,
    x11::{WindowAttributesExtX11, WindowType},
  },
  window::WindowLevel,
};

use crate::{
  control::Command,
  display::{select_monitor, update_monitor, window_size_for_display},
};

mod bridge;
mod config_watcher;
mod control;
mod display;
mod keys;
mod target;
mod window;

static TWEMOJI_FONT: &[u8] = include_bytes!("../assets/fonts/Twemoji.ttf");

/// Set by the build environment, for example `GIT_HASH=$(git rev-parse --short HEAD)`.
const GIT_HASH: Option<&str> = option_env!("GIT_HASH");

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
      let rev = GIT_HASH
        .map(|hash| format!(" (rev {hash})"))
        .unwrap_or_default();
      println!("orbolay {}{rev}", env!("CARGO_PKG_VERSION"));
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

  let wayland_display = std::env::var_os("WAYLAND_DISPLAY");

  if config.xwayland {
    unsafe { std::env::set_var("WAYLAND_DISPLAY", "") };
  }

  if args.config {
    open_configurator_standalone();
    std::process::exit(0);
  }

  if args.toggle_overlay || args.mute || args.deafen {
    let Ok(mut client) = control::Client::connect() else {
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
      if let Err(e) = client.send(cmd) {
        eprintln!("Failed to send command: {e}");
        std::process::exit(1);
      }
    }

    std::process::exit(0);
  }

  if control::Client::connect().is_ok() {
    if !args.target.is_empty() {
      info!("Orbolay is already running; launching the program without another overlay");
      target::run_target(&args.target, wayland_display);
    }
    eprintln!("Orbolay is already running; stop it before starting another instance.");
    std::process::exit(1);
  }

  // Write the defaults so later starts are not first runs
  if first_run && let Err(err) = save_config(&config) {
    warn!("Failed to save config.json: {err}");
  }

  if !config.xwayland && wayland_display.as_ref().is_some_and(|d| !d.is_empty()) {
    warn!(
      "XWayland is disabled; the overlay may not stay on top or pass clicks through under Wayland"
    );
  }

  target::launch_target(&args.target, wayland_display);

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

/// Directive names match module paths by prefix, so `orbolay` also covers every `orbolay_*` crate.
const DEFAULT_LOG_FILTER: &str = "warn,orbolay=info";

/// Logs Orbolay's own crates at `info` and dependencies at `warn` unless `RUST_LOG` overrides it.
fn init_logging() {
  let mut builder =
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_LOG_FILTER));
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
  let debug = startup.debug;
  let mut model = use_state(|| Arc::new(AppState::new(startup.config.clone())));
  let mut view = use_state(OverlayView::default);

  let app = use_hook(move || {
    let (redraw_tx, redraw_rx) = flume::bounded(1);
    let (toggle_tx, toggle_rx) = flume::unbounded::<()>();
    let (app, commands) = AppHandle::new(AppState::new(startup.config.clone()), redraw_tx);

    start_background(&app, &startup, commands, toggle_tx);

    let snapshot_app = app.clone();
    spawn_in_window(async move {
      while redraw_rx.recv_async().await.is_ok() {
        model.set(Arc::new(snapshot_app.snapshot()));
      }
    });

    // Toggles from the keybind and the control socket; ignored while the settings window
    // records a new keybind, whose key presses would otherwise toggle the overlay
    let recording = app.recording_keybind().clone();
    spawn_in_window(async move {
      while toggle_rx.recv_async().await.is_ok() {
        if !recording.load(Ordering::Relaxed) {
          let next = view().toggled();
          view.set(next);
        }
      }
    });

    // Move the window when the selected display changes
    let config_rx = app.subscribe_config();
    let mut display = (
      startup.config.display_name.clone(),
      startup.config.display_idx,
    );
    spawn_in_window(async move {
      while let Ok(config) = config_rx.recv_async().await {
        let selected = (config.display_name.clone(), config.display_idx);
        if selected != display {
          display = selected;
          update_monitor(&config);
        }
      }
    });

    if startup.first_run {
      let config_app = app.clone();
      spawn_in_window(async move { open_configurator(&config_app) });
    }

    app
  });

  // Make the window click-through while closed
  use_side_effect(move || window::set_clickable(view().is_open() || debug));

  Overlay {
    app,
    state: model.read().clone(),
    view,
  }
}

/// Starts the threads that feed the shared state: keybinds, the control socket, the bridge
/// transport, and the config file watcher.
fn start_background(
  app: &AppHandle,
  startup: &Startup,
  commands: flume::Receiver<BridgeCommand>,
  toggle_tx: flume::Sender<()>,
) {
  keys::watch_keybinds(app.clone(), toggle_tx.clone());

  let ctrl_app = app.clone();
  control::start(move |cmd| match cmd {
    Command::ToggleOverlay => {
      let _ = toggle_tx.send(());
    }
    Command::Mute => ctrl_app.send(BridgeCommand::ToggleMute),
    Command::Deafen => ctrl_app.send(BridgeCommand::ToggleDeaf),
  });

  bridge::start(app.clone(), startup.port, commands);

  orbolay_ui::configurator::save_config_changes(app);
  config_watcher::start(app.clone());
}

#[cfg(test)]
mod tests {
  use log::{Level, Metadata};

  use super::DEFAULT_LOG_FILTER;

  fn enabled(target: &str, level: Level) -> bool {
    let filter = env_filter::Builder::new().parse(DEFAULT_LOG_FILTER).build();
    filter.enabled(&Metadata::builder().target(target).level(level).build())
  }

  #[test]
  fn default_log_filter_covers_every_orbolay_crate() {
    for target in [
      module_path!(),
      "orbolay::bridge",
      "orbolay_core::config",
      "orbolay_keys",
      "orbolay_ui::util::html_template",
    ] {
      assert!(enabled(target, Level::Info), "{target}");
      assert!(!enabled(target, Level::Debug), "{target}");
    }
    assert!(!enabled("tokio_tungstenite::compat", Level::Info));
    assert!(enabled("tokio_tungstenite::compat", Level::Warn));
  }
}
