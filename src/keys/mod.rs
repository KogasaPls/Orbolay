use std::cell::RefCell;
use std::sync::{
  Arc, RwLock,
  atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use rdev::{Event, grab, listen};

use log::{info, warn};
use orbolay_core::app_state::AppHandle;

use orbolay_keys::{KeyEvent, KeyState, Keybind, strings_to_keys};

pub fn watch_keybinds(app: AppHandle, keybind_tx: flume::Sender<KeyEvent>) {
  let enabled = Arc::new(AtomicBool::new(true));
  let keybinds: Arc<RwLock<Vec<Keybind>>> = Arc::new(RwLock::new(vec![Keybind::new(
    Vec::new(),
    KeyEvent::ToggleOverlay,
  )]));
  let recording = app.recording_keybind().clone();

  if !app.read(|state| state.config.keybind_enabled) {
    info!("Global keybinds are disabled; not watching input devices until enabled");
  }

  // Poll config every second to sync keybind/enabled state, and start the
  // listener the first time keybinds are enabled
  {
    thread::spawn(move || {
      let mut listening = false;

      loop {
        let (is_enabled, overlay_keybind) = app.read(|state| {
          (
            state.config.keybind_enabled,
            state.config.overlay_keybind.clone(),
          )
        });

        enabled.store(is_enabled, Ordering::Relaxed);

        if is_enabled && !listening {
          listening = true;
          start_listener(
            enabled.clone(),
            keybinds.clone(),
            recording.clone(),
            keybind_tx.clone(),
          );
        }

        let overlay_keys = strings_to_keys(&overlay_keybind);

        {
          let mut kbs = keybinds.write().unwrap();
          if let Some(kb) = kbs
            .iter_mut()
            .find(|kb| matches!(kb.event, KeyEvent::ToggleOverlay))
            && kb.keys != overlay_keys
          {
            kb.keys.clone_from(&overlay_keys);
            kb.reset();
          }
        }

        thread::sleep(Duration::from_secs(1));
      }
    });
  }
}

fn start_listener(
  enabled: Arc<AtomicBool>,
  keybinds: Arc<RwLock<Vec<Keybind>>>,
  recording: Arc<AtomicBool>,
  keybind_tx: flume::Sender<KeyEvent>,
) {
  thread::spawn(move || {
    let keybind_tx_grab = keybind_tx.clone();
    let enabled_grab = enabled.clone();
    let keybinds_grab = keybinds.clone();
    let recording_grab = recording.clone();
    let key_state_grab = RefCell::new(KeyState::new());

    let grab_result = grab(move |event: Event| {
      if !recording_grab.load(Ordering::Relaxed) && enabled_grab.load(Ordering::Relaxed) {
        let kbs = keybinds_grab.read().unwrap();
        for event in
          orbolay_keys::process(&event.event_type, &mut key_state_grab.borrow_mut(), &kbs)
        {
          let _ = keybind_tx_grab.send(event);
        }
      }
      Some(event)
    });

    if let Err(e) = grab_result {
      warn!("Failed to grab global hotkeys: {e:?}");

      let key_state_listen = RefCell::new(KeyState::new());
      if let Err(e) = listen(move |event: Event| {
        if !recording.load(Ordering::Relaxed) && enabled.load(Ordering::Relaxed) {
          let kbs = keybinds.read().unwrap();
          for event in
            orbolay_keys::process(&event.event_type, &mut key_state_listen.borrow_mut(), &kbs)
          {
            let _ = keybind_tx.send(event);
          }
        }
      }) {
        warn!("Failed to listen for global hotkeys: {e:?}");
      }
    }
  });
}
