use orbolay_core::monitors::{MonitorInfo, set_monitors};
use winit::monitor::MonitorHandle;

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
        // Winit's macOS names identify a model, not an individual monitor.
        #[cfg(target_os = "macos")]
        native_name: None,
        #[cfg(not(target_os = "macos"))]
        native_name: name,
        is_primary: primary.map(|p| p == m).unwrap_or(idx == 0),
        width: size.width,
        height: size.height,
      }
    })
    .collect();

  set_monitors(infos);
}
