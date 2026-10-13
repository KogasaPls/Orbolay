use freya::prelude::{Platform, WinitPlatformExt};
use winit::{dpi::PhysicalSize, monitor::MonitorHandle};

use orbolay_core::config::load_config;
use orbolay_logging::warn;
use orbolay_ui::populate_monitor_cache;

use crate::window::WINDOW_ID;

pub fn select_monitor<'a>(
  primary: Option<&MonitorHandle>,
  monitors: &'a [MonitorHandle],
) -> Option<&'a MonitorHandle> {
  let config = load_config().unwrap_or_default();
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
  let size = monitor.size();

  // https://discourse.glfw.org/t/black-screen-when-setting-window-to-transparent-and-size-to-1920x1080/2585/5
  // We do -1 specifically on the height to fix the Windows hidden taskbar thing
  PhysicalSize::new(size.width + 1, size.height.saturating_sub(1))
}

pub fn update_monitor() {
  let Some(window_id) = *WINDOW_ID.lock().unwrap() else {
    return;
  };
  Platform::get().with_window(window_id, move |w| {
    let monitors: Vec<_> = w.available_monitors().collect();
    let primary = w.primary_monitor();

    populate_monitor_cache(primary.as_ref(), &monitors);

    let Some(monitor) = select_monitor(primary.as_ref(), &monitors) else {
      return;
    };

    w.set_outer_position(monitor.position());
    let _ = w.request_inner_size(window_size_for_display(monitor));
  });
}
