use freya::prelude::{Platform, WinitPlatformExt};
use winit::{dpi::PhysicalSize, monitor::MonitorHandle};

use log::warn;
use orbolay_core::config::Config;
use orbolay_ui::populate_monitor_cache;

use crate::window::WINDOW_ID;

pub fn select_monitor<'a>(
  config: &Config,
  primary: Option<&MonitorHandle>,
  monitors: &'a [MonitorHandle],
) -> Option<&'a MonitorHandle> {
  let display_idx = config.display_index(monitors.iter().map(MonitorHandle::name));

  if let Some(idx) = display_idx {
    if let Some(monitor) = monitors.get(idx) {
      return Some(monitor);
    }

    warn!(
      "Saved display index {} is out of bounds ({} displays detected)",
      idx,
      monitors.len()
    );
  }

  if let Some(name) = config.display_name.as_deref() {
    warn!("Saved display name {name:?} was not found; using the primary display");
  }

  let primary_idx = primary
    .and_then(|p| monitors.iter().position(|m| m == p))
    .unwrap_or(0);
  monitors.get(primary_idx)
}

pub fn window_size_for_display(monitor: &MonitorHandle) -> PhysicalSize<u32> {
  monitor.size()
}

pub fn update_monitor(config: &Config) {
  let config = config.clone();
  let Some(window_id) = *WINDOW_ID.lock().unwrap() else {
    return;
  };
  Platform::get().with_window(window_id, move |w| {
    let monitors: Vec<_> = w.available_monitors().collect();
    let primary = w.primary_monitor();

    populate_monitor_cache(primary.as_ref(), &monitors);

    let Some(monitor) = select_monitor(&config, primary.as_ref(), &monitors) else {
      return;
    };

    w.set_outer_position(monitor.position());
    let _ = w.request_inner_size(window_size_for_display(monitor));
  });
}
