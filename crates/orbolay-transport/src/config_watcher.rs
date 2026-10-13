use std::sync::mpsc;

use notify::{Event, Result, Watcher};
use orbolay_logging::{error, log, warn};

use orbolay_core::{
  app_state::AppHandle,
  config::{Config, config_dir, load_config},
};

pub fn start_config_watcher(
  app: AppHandle,
  on_config_change: impl Fn(&Config, &Config) + Send + 'static,
) {
  log!("Starting config file notification thread");
  // The UI updates app state before saving, so compare against the last loaded config.
  let mut previous_config = app.read(|state| state.config.clone());
  std::thread::spawn(move || {
    let (tx, rx) = mpsc::channel::<Result<Event>>();

    let mut watcher = match notify::recommended_watcher(tx) {
      Ok(w) => w,
      Err(e) => {
        warn!(
          "Failed to create config watcher: {}. Config changes will not reflect until Orbolay is restarted.",
          e
        );
        return;
      }
    };

    let config_path = match config_dir() {
      Some(path) => path.join("config.json"),
      None => {
        warn!(
          "Failed to get config path. Config changes will not reflect until Orbolay is restarted."
        );
        return;
      }
    };

    if let Err(e) = watcher.watch(&config_path, notify::RecursiveMode::NonRecursive) {
      warn!(
        "Failed to watch config file at {:?}: {}. Config changes will not reflect until Orbolay is restarted.",
        config_path, e
      );
      return;
    }

    loop {
      match rx.recv() {
        Ok(Ok(event)) => {
          if !event.kind.is_modify() {
            continue;
          }

          if event.paths.iter().any(|p| p == &config_path) {
            log!("Config file changed, reloading...");
            if let Some(new_config) = load_config() {
              apply_config_change(&app, &mut previous_config, new_config, &on_config_change);

              log!("Config reloaded successfully");
            } else {
              warn!("Failed to reload config file");
            }
          }
        }
        Ok(Err(e)) => error!("Watcher error: {}", e),
        Err(e) => error!("Watcher channel error: {}", e),
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
  app.update(|state| state.config = new_config.clone());
  on_config_change(previous_config, &new_config);
  *previous_config = new_config;
}

#[cfg(test)]
mod tests {
  use super::apply_config_change;
  use orbolay_core::{
    app_state::{AppHandle, SharedAppState},
    config::Config,
  };
  use std::cell::RefCell;

  #[test]
  fn ui_changes_are_compared_with_the_previous_loaded_config() {
    let mut previous_config = Config {
      display_idx: Some(0),
      ..Config::default()
    };
    let new_config = Config {
      display_idx: Some(1),
      ..Config::default()
    };
    let (redraw_tx, _) = flume::unbounded();
    let app = AppHandle::new(SharedAppState::default(), redraw_tx);
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

    assert_eq!(
      *observed.borrow(),
      vec![(Some(0), Some(1)), (Some(1), Some(1))]
    );
  }
}
