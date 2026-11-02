//! Reads keyboards through evdev without grabbing them, so other programs still receive every
//! key press.
//!
//! While global keybinds are disabled the keybind thread only waits for config changes: no
//! async runtime, open device, or `/dev/input` watcher exists until they are enabled again.

use std::{
  convert::Infallible,
  path::PathBuf,
  time::{Duration, Instant},
};

use evdev::{Device, EventStream, EventSummary};
use log::{debug, info, warn};
use notify::{EventKind, RecursiveMode, Watcher};
use orbolay_core::{app_state::AppHandle, config::Config};
use orbolay_keys::{Key, KeyState, Keybind, strings_to_keys};
use tokio::task::JoinSet;

const INPUT_DIR: &str = "/dev/input";
/// udev applies device permissions shortly after the node appears.
const HOTPLUG_DELAY: Duration = Duration::from_millis(500);

/// Sends `()` on `pressed_tx` each time the overlay keybind is pressed.
pub fn watch_keybinds(app: AppHandle, pressed_tx: flume::Sender<()>) {
  let config_rx = app.subscribe_config();
  let spawned = std::thread::Builder::new()
    .name("orbolay-keys".into())
    .spawn(move || run(&app, &config_rx, &pressed_tx));
  if let Err(e) = spawned {
    warn!("Could not start the keybind thread: {e}");
  }
}

/// The overlay keybind, or `None` while keybinds are disabled or unset.
fn overlay_bind(config: &Config) -> Option<Keybind> {
  config
    .keybind_enabled
    .then(|| Keybind::new(strings_to_keys(&config.overlay_keybind)))
    .flatten()
}

fn run(app: &AppHandle, config_rx: &flume::Receiver<Config>, pressed_tx: &flume::Sender<()>) {
  let mut bind = app.read(|state| overlay_bind(&state.config));
  loop {
    let Some(active) = bind.take() else {
      info!("Global keybinds are disabled; not reading input devices");
      // Block until the config enables keybinds; nothing else runs meanwhile.
      bind = loop {
        let Ok(config) = config_rx.recv() else { return };
        if let Some(enabled) = overlay_bind(&config) {
          break Some(enabled);
        }
      };
      continue;
    };

    match listen_until_changed(&active, config_rx, pressed_tx) {
      Ok(next) => bind = next,
      Err(flume::RecvError::Disconnected) => return,
    }
  }
}

/// Reads devices for `bind` until the overlay keybind changes, then drops the runtime, which
/// closes every device. Returns the new keybind, or an error once the app stops sending config.
fn listen_until_changed(
  bind: &Keybind,
  config_rx: &flume::Receiver<Config>,
  pressed_tx: &flume::Sender<()>,
) -> Result<Option<Keybind>, flume::RecvError> {
  let runtime = match tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .build()
  {
    Ok(runtime) => runtime,
    Err(e) => {
      warn!("Could not start the keybind runtime: {e}");
      return config_rx.recv().map(|config| overlay_bind(&config));
    }
  };

  runtime.block_on(async {
    let mut listening = std::pin::pin!(listen(bind, pressed_tx));
    loop {
      tokio::select! {
        never = &mut listening => match never {},
        config = config_rx.recv_async() => {
          let new_bind = overlay_bind(&config?);
          if new_bind.as_ref() != Some(bind) {
            debug!("Overlay keybind changed to {new_bind:?}");
            return Ok(new_bind);
          }
        }
      }
    }
  })
}

/// Reads every device that can send one of the bind's keys until the future is dropped, which
/// closes the devices. It never finishes on its own: it holds a sender for its own key channel.
async fn listen(bind: &Keybind, pressed_tx: &flume::Sender<()>) -> Infallible {
  let keys = bind.keys();
  let (key_tx, key_rx) = flume::unbounded::<(Key, bool)>();
  // Watch before enumerating so a device plugged in meanwhile is not missed; one reported by both
  // is read twice, which KeyState tolerates.
  let (added_tx, added_rx) = flume::unbounded::<PathBuf>();
  let _watcher = watch_new_devices(added_tx);

  let mut readers = JoinSet::new();
  let mut opened = 0;
  for (path, device) in evdev::enumerate() {
    if relevant(&path, &device, keys) && open(&mut readers, &path, device, &key_tx) {
      opened += 1;
    }
  }
  if opened == 0 {
    warn!(
      "No readable keyboard in {INPUT_DIR}; add your user to the input group or bind `orbolay --toggle-overlay` in your desktop's shortcut settings"
    );
  } else {
    info!("Listening for the overlay keybind on {opened} device(s)");
  }

  let mut state = KeyState::default();
  loop {
    tokio::select! {
      Ok((key, pressed)) = key_rx.recv_async() => {
        if state.process(bind, key, pressed, Instant::now()) {
          info!("Overlay keybind pressed");
          let _ = pressed_tx.send(());
        }
      }
      Some(_) = readers.join_next() => {}
      Ok(path) = added_rx.recv_async() => {
        let (keys, key_tx) = (keys.to_vec(), key_tx.clone());
        readers.spawn(async move {
          tokio::time::sleep(HOTPLUG_DELAY).await;
          if let Ok(device) = Device::open(&path)
            && relevant(&path, &device, &keys)
            && let Some(events) = stream(&path, device)
          {
            forward(events, &key_tx).await;
          }
        });
      }
    }
  }
}

/// Whether `device` can send any key in `keys`.
fn relevant(path: &std::path::Path, device: &Device, keys: &[Key]) -> bool {
  let relevant = device
    .supported_keys()
    .is_some_and(|supported| keys.iter().any(|key| supported.contains(key.code())));
  if !relevant {
    debug!("Skipping {} ({})", device_name(device), path.display());
  }
  relevant
}

fn device_name(device: &Device) -> String {
  device.name().unwrap_or("unnamed device").to_owned()
}

/// Starts forwarding key presses and releases from `device`. Returns whether it could be read.
fn open(
  readers: &mut JoinSet<()>,
  path: &std::path::Path,
  device: Device,
  key_tx: &flume::Sender<(Key, bool)>,
) -> bool {
  let Some(events) = stream(path, device) else {
    return false;
  };
  let key_tx = key_tx.clone();
  readers.spawn(async move { forward(events, &key_tx).await });
  true
}

fn stream(path: &std::path::Path, device: Device) -> Option<EventStream> {
  let name = device_name(&device);
  match device.into_event_stream() {
    Ok(events) => {
      debug!("Reading keys from {name} ({})", path.display());
      Some(events)
    }
    Err(e) => {
      debug!("Cannot read {name} ({}): {e}", path.display());
      None
    }
  }
}

/// Sends each key press, repeat, and release until the device or the listener goes away. Repeats
/// count as presses, so a held key does not go stale.
async fn forward(mut events: EventStream, key_tx: &flume::Sender<(Key, bool)>) {
  loop {
    match events.next_event().await.map(EventSummary::from) {
      Ok(EventSummary::Key(_, code, value @ 0..=2)) => {
        if let Some(key) = Key::from_code(code)
          && key_tx.send((key, value != 0)).is_err()
        {
          return;
        }
      }
      Ok(_) => {}
      Err(e) => {
        debug!("Stopped reading {}: {e}", device_name(events.device()));
        return;
      }
    }
  }
}

/// Reports input devices created after startup.
fn watch_new_devices(added_tx: flume::Sender<PathBuf>) -> Option<notify::RecommendedWatcher> {
  let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
    let Ok(event) = event else { return };
    if !matches!(event.kind, EventKind::Create(_)) {
      return;
    }
    for path in event.paths {
      let is_event_node = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("event"));
      if is_event_node {
        let _ = added_tx.send(path);
      }
    }
  })
  .ok()?;
  watcher
    .watch(INPUT_DIR.as_ref(), RecursiveMode::NonRecursive)
    .map_err(|e| debug!("Cannot watch {INPUT_DIR} for new devices: {e}"))
    .ok()?;
  Some(watcher)
}
