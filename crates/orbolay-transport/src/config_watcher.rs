use std::sync::mpsc;

use log::{error, info, warn};
use notify::{Event, Result, Watcher};

use orbolay_core::{
  app_state::AppHandle,
  config::{CONFIG_FILE, Config, config_dir, load_config},
};

pub fn start_config_watcher(
  app: AppHandle,
  on_config_change: impl Fn(&Config, &Config) + Send + 'static,
) {
  info!("Starting config file notification thread");
  // The UI updates app state before saving, so compare against the last loaded config.
  let mut previous_config = app.read(|state| state.config.clone());
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

    for event in rx {
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

      match load_config() {
        Ok(new_config) => {
          apply_config_change(&app, &mut previous_config, new_config, &on_config_change);
        }
        Err(e) => warn!("Failed to reload config: {e}"),
      }
    }
  });
}

fn apply_config_change(
  app: &AppHandle,
  previous_config: &mut Config,
  new_config: Config,
  on_config_change: &impl Fn(&Config, &Config),
) {
  if *previous_config == new_config {
    return;
  }
  info!("Config file changed, reloading");
  app.update_if_changed(|state| {
    let changed = state.config != new_config;
    if changed {
      state.config = new_config.clone();
    }
    changed
  });
  on_config_change(previous_config, &new_config);
  *previous_config = new_config;
}

#[cfg(test)]
mod tests {
  use super::apply_config_change;
  use orbolay_core::{
    app_state::{AppHandle, AppState},
    config::Config,
  };
  use std::cell::RefCell;

  #[test]
  fn ui_changes_are_compared_with_the_previous_loaded_config_once() {
    let mut previous_config = Config {
      display_idx: Some(0),
      ..Config::default()
    };
    let new_config = Config {
      display_idx: Some(1),
      ..Config::default()
    };
    let (redraw_tx, _) = flume::bounded(1);
    let (open_config_tx, _) = flume::unbounded();
    let (app, _commands) = AppHandle::new(AppState::default(), redraw_tx, open_config_tx);
    // The settings UI updates app state before the file watcher reloads it.
    app.update(|state| state.config = new_config.clone());

    let observed = RefCell::new(Vec::new());
    let on_change = |old: &Config, new: &Config| {
      observed
        .borrow_mut()
        .push((old.display_idx, new.display_idx));
    };
    apply_config_change(&app, &mut previous_config, new_config.clone(), &on_change);
    apply_config_change(&app, &mut previous_config, new_config, &on_change);

    assert_eq!(*observed.borrow(), vec![(Some(0), Some(1))]);
  }
}
