//! Reads keyboards through evdev without grabbing them, so other programs still receive every
//! key press.
//!
//! While global keybinds are disabled the keybind thread only waits for config changes: no
//! async runtime, open device, or `/dev/input` watcher exists until they are enabled again.

use std::{
  path::PathBuf,
  sync::atomic::{AtomicBool, Ordering},
  time::{Duration, Instant},
};

use evdev::{Device, EventSummary};
use log::{debug, info, warn};
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::task::JoinSet;

use orbolay_core::{app_state::AppHandle, config::Config};
use orbolay_keys::{Key, KeyEvent, KeyState, Keybind, strings_to_keys};

const INPUT_DIR: &str = "/dev/input";
/// udev applies device permissions shortly after the node appears.
const HOTPLUG_DELAY: Duration = Duration::from_millis(500);

pub fn watch_keybinds(app: AppHandle, keybind_tx: flume::Sender<KeyEvent>) {
  let config_rx = app.subscribe_config();
  let spawned = std::thread::Builder::new()
    .name("orbolay-keys".into())
    .spawn(move || run(&app, &config_rx, &keybind_tx));
  if let Err(e) = spawned {
    warn!("Could not start the keybind thread: {e}");
  }
}

/// The overlay keybind, or `None` while keybinds are disabled or unset.
fn overlay_keys(config: &Config) -> Option<Vec<Key>> {
  config
    .keybind_enabled
    .then(|| strings_to_keys(&config.overlay_keybind))
    .filter(|keys| !keys.is_empty())
}

fn run(app: &AppHandle, config_rx: &flume::Receiver<Config>, keybind_tx: &flume::Sender<KeyEvent>) {
  let mut keys = app.read(|state| overlay_keys(&state.config));
  loop {
    let Some(active) = keys.take() else {
      info!("Global keybinds are disabled; not reading input devices");
      // Block until the config enables keybinds; nothing else runs meanwhile.
      keys = loop {
        let Ok(config) = config_rx.recv() else { return };
        if let Some(enabled) = overlay_keys(&config) {
          break Some(enabled);
        }
      };
      continue;
    };

    match listen_until_changed(&active, app, config_rx, keybind_tx) {
      Ok(next) => keys = next,
      Err(flume::RecvError::Disconnected) => return,
    }
  }
}

/// Reads devices for `keys` until the overlay keybind changes, then drops the runtime, which
/// closes every device. Returns the new keybind, or an error once the app stops sending config.
fn listen_until_changed(
  keys: &[Key],
  app: &AppHandle,
  config_rx: &flume::Receiver<Config>,
  keybind_tx: &flume::Sender<KeyEvent>,
) -> Result<Option<Vec<Key>>, flume::RecvError> {
  let runtime = match tokio::runtime::Builder::new_current_thread()
    .enable_io()
    .enable_time()
    .build()
  {
    Ok(runtime) => runtime,
    Err(e) => {
      warn!("Could not start the keybind runtime: {e}");
      return config_rx.recv().map(|config| overlay_keys(&config));
    }
  };

  runtime.block_on(async {
    let mut listening = std::pin::pin!(listen(keys.to_vec(), app.recording_keybind(), keybind_tx));
    let mut finished = false;
    loop {
      tokio::select! {
        () = &mut listening, if !finished => finished = true,
        config = config_rx.recv_async() => {
          let new_keys = overlay_keys(&config?);
          if new_keys.as_deref() != Some(keys) {
            debug!("Overlay keybind changed to {new_keys:?}");
            return Ok(new_keys);
          }
        }
      }
    }
  })
}

/// Reads every device that can send one of `keys` until the future is dropped, which closes
/// the devices.
async fn listen(keys: Vec<Key>, recording: &AtomicBool, keybind_tx: &flume::Sender<KeyEvent>) {
  let (key_tx, key_rx) = flume::unbounded::<(Key, bool)>();
  let mut readers = JoinSet::new();
  let mut opened = 0;
  for (path, device) in evdev::enumerate() {
    if relevant(&path, &device, &keys) && open(&mut readers, &path, device, &key_tx) {
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

  let (added_tx, added_rx) = flume::unbounded::<PathBuf>();
  let _watcher = watch_new_devices(added_tx);

  let mut binds = vec![Keybind::new(keys.clone(), KeyEvent::ToggleOverlay)];
  let mut state = KeyState::default();
  loop {
    tokio::select! {
      change = key_rx.recv_async() => {
        let Ok((key, pressed)) = change else { return };
        if recording.load(Ordering::Relaxed) {
          continue;
        }
        for event in state.process(key, pressed, &mut binds, Instant::now()) {
          info!("Keybind pressed: {event:?}");
          let _ = keybind_tx.send(event);
        }
      }
      Ok(path) = added_rx.recv_async() => {
        tokio::time::sleep(HOTPLUG_DELAY).await;
        if let Ok(device) = Device::open(&path)
          && relevant(&path, &device, &keys)
        {
          open(&mut readers, &path, device, &key_tx);
        }
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
  let name = device_name(&device);
  let mut events = match device.into_event_stream() {
    Ok(events) => events,
    Err(e) => {
      debug!("Cannot read {name} ({}): {e}", path.display());
      return false;
    }
  };
  debug!("Reading keys from {name} ({})", path.display());

  let key_tx = key_tx.clone();
  readers.spawn(async move {
    loop {
      match events.next_event().await.map(EventSummary::from) {
        Ok(EventSummary::Key(_, code, value @ (0 | 1))) => {
          if let Some(key) = Key::from_code(code)
            && key_tx.send((key, value == 1)).is_err()
          {
            return;
          }
        }
        Ok(_) => {}
        Err(e) => {
          debug!("Stopped reading {name}: {e}");
          return;
        }
      }
    }
  });
  true
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
