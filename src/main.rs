use std::sync::atomic::Ordering;

use freya::prelude::*;
use gumdrop::Options;
use log::warn;
use orbolay_control::Command;
use orbolay_core::{
  app_state::{AppHandle, AppState},
  config::{Config, Corner, LoadError, load_config, save_config},
  dirs::ensure_config_dirs,
  protocol::BridgeCommand,
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

use crate::display::{select_monitor, update_monitor, window_size_for_display};

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

  if orbolay_control::Client::connect().is_ok() {
    eprintln!("Orbolay is already running; stop it before starting another instance.");
    std::process::exit(1);
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
  let debug = startup.debug;
  let mut model = use_state(|| AppState::new(startup.config.clone()));
  let mut is_open = use_state(|| false);
  let mut soundboard_open = use_state(|| false);

  let app = use_hook(move || {
    let (redraw_tx, redraw_rx) = flume::bounded(1);
    let (open_config_tx, open_config_rx) = flume::unbounded();
    let (toggle_tx, toggle_rx) = flume::unbounded::<KeyEvent>();
    let (app, commands) = AppHandle::new(
      AppState::new(startup.config.clone()),
      redraw_tx,
      open_config_tx,
    );

    start_background(&app, &startup, commands, toggle_tx);

    let snapshot_app = app.clone();
    spawn_in_window(async move {
      while redraw_rx.recv_async().await.is_ok() {
        model.set(snapshot_app.snapshot());
      }
    });

    spawn_in_window(async move {
      while matches!(toggle_rx.recv_async().await, Ok(KeyEvent::ToggleOverlay)) {
        let open = !is_open();
        is_open.set(open);
      }
    });

    let config_app = app.clone();
    spawn_in_window(async move {
      while open_config_rx.recv_async().await.is_ok() {
        is_open.set(false);
        open_configurator(&config_app);
      }
    });

    if startup.first_run {
      let _ = app.open_config();
    }

    app
  });

  // Make the window click-through while closed, and close the soundboard with it
  use_side_effect(move || {
    let open = is_open();
    if !open {
      soundboard_open.set_if_modified(false);
    }
    window::set_clickable(open || debug);
  });

  let state = model.read();
  let config = &state.config;
  let open = is_open();
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

  let settings_alignment = Corner::unused(&[config.user_alignment, config.message_alignment])
    .unwrap_or(Corner::BottomLeft);
  let settings_app = app.clone();
  let open_settings: EventHandler<()> = (move |()| {
    let _ = settings_app.open_config();
  })
  .into();

  let voice = VoiceSection {
    app: app.clone(),
    voice_users: state.voice_users.clone(),
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
    user_template_size: (320, 62),
    user_template: config.user_template.clone(),
  };
  let messages = config
    .enable_message_notifications
    .then(|| MessagesSection {
      app: app.clone(),
      messages: state.messages.clone(),
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
    current_guild_id: state.current_guild_id.clone(),
    has_nitro: state.has_nitro,
    theme,
    ui_scale,
  };
  drop(state);

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
        .on_press(move |_| is_open.set(false)),
    )
    .child(voice)
    .maybe_child(messages)
    // Voice controls + soundboard
    .maybe(open, |el| {
      el
        // Transparent backdrop that catches clicks outside the popup to dismiss it
        .maybe(soundboard_open(), |el| {
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
            .maybe(soundboard_open(), |el| el.child(soundboard))
            .child(VoiceControls {
              user,
              app: app.clone(),
              soundboard_open,
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

/// Starts the threads that feed the shared state: keybinds, the control socket, the bridge
/// transport, and the config file watcher.
fn start_background(
  app: &AppHandle,
  startup: &Startup,
  commands: flume::Receiver<BridgeCommand>,
  toggle_tx: flume::Sender<KeyEvent>,
) {
  keys::watch_keybinds(app.clone(), toggle_tx.clone());

  let ctrl_app = app.clone();
  orbolay_control::start(move |cmd| match cmd {
    Command::ToggleOverlay => {
      if !ctrl_app.recording_keybind().load(Ordering::Relaxed) {
        let _ = toggle_tx.send(KeyEvent::ToggleOverlay);
      }
    }
    Command::Mute => ctrl_app.send(BridgeCommand::ToggleMute),
    Command::Deafen => ctrl_app.send(BridgeCommand::ToggleDeaf),
  });

  create_transport_thread(app.clone(), startup.port, commands);

  if startup.first_run
    && let Err(err) = save_config(&startup.config)
  {
    warn!("Failed to save config.json: {err}");
  }

  start_config_watcher(app.clone(), |old_config, new_config| {
    // Reposition the window when the target monitor changes
    if old_config.display_name != new_config.display_name
      || old_config.display_idx != new_config.display_idx
    {
      update_monitor(new_config);
    }
  });
}
