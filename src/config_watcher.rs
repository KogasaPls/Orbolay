use std::{sync::mpsc, time::Duration};

use log::{error, info, warn};
use notify::{Event, Result, Watcher};
use orbolay_core::{
  app_state::AppHandle,
  config::{CONFIG_FILE, config_dir, reload_config},
};

/// How long config.json must stay quiet before it is reloaded, so one save reloads once.
const RELOAD_DELAY: Duration = Duration::from_millis(100);

/// Applies edits made to config.json outside Orbolay. Orbolay's own saves are skipped because
/// `reload_config` recognizes what it last wrote.
pub fn start(app: AppHandle) {
  info!("Starting config file notification thread");
  std::thread::spawn(move || {
    let (tx, rx) = mpsc::channel::<Result<Event>>();

    let mut watcher = match notify::recommended_watcher(tx) {
      Ok(w) => w,
      Err(e) => {
        warn!(
          "Failed to create config watcher: {e}. Config changes will not reflect until Orbolay is restarted."
        );
        return;
      }
    };

    // Watch the directory: editors and save_config replace the file, which ends a watch on the
    // file's own inode.
    let Some(dir) = config_dir() else {
      warn!(
        "Failed to get config path. Config changes will not reflect until Orbolay is restarted."
      );
      return;
    };

    if let Err(e) = watcher.watch(&dir, notify::RecursiveMode::NonRecursive) {
      warn!(
        "Failed to watch {}: {e}. Config changes will not reflect until Orbolay is restarted.",
        dir.display()
      );
      return;
    }

    while let Ok(event) = rx.recv() {
      let event = match event {
        Ok(event) => event,
        Err(e) => {
          error!("Watcher error: {e}");
          continue;
        }
      };
      let touches_config = event
        .paths
        .iter()
        .any(|path| path.file_name().is_some_and(|name| name == CONFIG_FILE));
      if !touches_config || !(event.kind.is_modify() || event.kind.is_create()) {
        continue;
      }

      while rx.recv_timeout(RELOAD_DELAY).is_ok() {}

      match reload_config() {
        Ok(Some(new_config)) => {
          app.set_config(|config| *config = new_config);
        }
        Ok(None) => {}
        Err(e) => warn!("Failed to reload config: {e}"),
      }
    }
  });
}
