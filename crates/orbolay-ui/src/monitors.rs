use std::sync::Mutex;

use winit::monitor::MonitorHandle;

/// The monitors the overlay and settings window saw last, in Winit order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorInfo {
  pub name: String,
  /// The name used for monitor matching, separate from the settings label.
  pub native_name: Option<String>,
  pub is_primary: bool,
  pub width: u32,
  pub height: u32,
}

static CACHE: Mutex<Vec<MonitorInfo>> = Mutex::new(Vec::new());

pub fn set_monitors(infos: Vec<MonitorInfo>) {
  *CACHE.lock().unwrap() = infos;
}

pub fn monitors_list() -> Vec<MonitorInfo> {
  CACHE.lock().unwrap().clone()
}

/// Keep overlay and configurator choices in the same Winit order and name namespace.
pub fn populate_monitor_cache(primary: Option<&MonitorHandle>, monitors: &[MonitorHandle]) {
  let infos = monitors
    .iter()
    .enumerate()
    .map(|(idx, m)| {
      let size = m.size();
      let name = m.name();
      MonitorInfo {
        name: name.clone().unwrap_or_else(|| format!("Display {idx}")),
        native_name: name,
        is_primary: primary.map_or(idx == 0, |p| p == m),
        width: size.width,
        height: size.height,
      }
    })
    .collect();

  set_monitors(infos);
}
