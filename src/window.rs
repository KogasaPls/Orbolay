use std::sync::{Mutex, OnceLock};

use freya::prelude::*;
use log::warn;
use winit::{
  raw_window_handle::{HasWindowHandle, RawWindowHandle},
  window::WindowId,
};
use x11rb::{
  connection::Connection,
  protocol::{
    shape::{self, ConnectionExt as _},
    xproto::{ClipOrdering, Rectangle},
  },
  rust_connection::RustConnection,
};

pub static WINDOW_ID: Mutex<Option<WindowId>> = Mutex::new(None);

/// The window and input region last applied, so a repeated call sends nothing.
static APPLIED: Mutex<Option<(u32, bool)>> = Mutex::new(None);

fn x11() -> Option<&'static RustConnection> {
  static CONNECTION: OnceLock<Option<RustConnection>> = OnceLock::new();
  CONNECTION
    .get_or_init(|| match x11rb::connect(None) {
      Ok((conn, _)) => Some(conn),
      Err(e) => {
        warn!("Failed to connect to the X server: {e}");
        None
      }
    })
    .as_ref()
}

pub fn set_clickable(clickable: bool) {
  let Some(window_id) = *WINDOW_ID.lock().unwrap() else {
    return;
  };

  Platform::get().with_window(window_id, move |w| {
    let _ = w.set_cursor_hittest(clickable);

    // On X11/KDE, set_cursor_hittest is not enough for click-through
    let Ok(handle) = w.window_handle() else {
      warn!("Failed to get window handle");
      return;
    };
    let xid = match handle.as_raw() {
      RawWindowHandle::Xcb(h) => h.window.get(),
      RawWindowHandle::Xlib(h) => match u32::try_from(h.window) {
        Ok(xid) => xid,
        Err(_) => return,
      },
      // Wayland does not need the input shape
      _ => return,
    };
    set_input_region(xid, clickable);
  });
}

fn set_input_region(xid: u32, clickable: bool) {
  let mut applied = APPLIED.lock().unwrap();
  if *applied == Some((xid, clickable)) {
    return;
  }
  let Some(conn) = x11() else {
    return;
  };

  let full = [Rectangle {
    x: 0,
    y: 0,
    width: u16::MAX,
    height: u16::MAX,
  }];
  let rects: &[Rectangle] = if clickable { &full } else { &[] };
  let sent = conn
    .shape_rectangles(
      shape::SO::SET,
      shape::SK::INPUT,
      ClipOrdering::UNSORTED,
      xid,
      0,
      0,
      rects,
    )
    .is_ok()
    && conn.flush().is_ok();

  if sent {
    *applied = Some((xid, clickable));
  } else {
    warn!("Failed to update the overlay's X11 input region");
  }
}
