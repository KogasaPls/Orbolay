use freya::prelude::{Platform, WinitPlatformExt};
use log::warn;
use orbolay_core::config::Config;
use winit::{dpi::PhysicalSize, monitor::MonitorHandle};

use crate::window::WINDOW_ID;

pub fn select_monitor<'a>(
  config: &Config,
  primary: Option<&MonitorHandle>,
  monitors: &'a [MonitorHandle],
) -> Option<&'a MonitorHandle> {
  let names: Vec<_> = monitors.iter().map(MonitorHandle::name).collect();
  let saved_missing = config.saved_display(&names).is_none()
    && (config.display_name.is_some() || config.display_idx.is_some());
  if saved_missing {
    warn!("The saved display is not connected; using the primary display");
  }
  let primary = primary.and_then(|p| monitors.iter().position(|m| m == p));
  monitors.get(config.select_display(&names, primary)?)
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

    let Some(monitor) = select_monitor(&config, primary.as_ref(), &monitors) else {
      return;
    };

    w.set_outer_position(monitor.position());
    let _ = w.request_inner_size(window_size_for_display(monitor));
  });
}
