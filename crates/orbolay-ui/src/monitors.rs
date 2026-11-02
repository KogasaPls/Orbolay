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
        native_name: name,
        is_primary: primary.map_or(idx == 0, |p| p == m),
        width: size.width,
        height: size.height,
      }
    })
    .collect();

  set_monitors(infos);
}
