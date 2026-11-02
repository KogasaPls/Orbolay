use std::{ffi::OsString, process::Command};

use log::{error, info};

/// Runs `cmd` with `WAYLAND_DISPLAY` restored to its original value and exits Orbolay with the
/// child's exit code when it finishes.
pub fn launch_target(cmd: &[String], wayland_display: Option<OsString>) {
  let Some((program, args)) = cmd.split_first() else {
    return;
  };
  let mut child = Command::new(program);
  child.args(args);
  match wayland_display {
    Some(display) => child.env("WAYLAND_DISPLAY", display),
    None => child.env_remove("WAYLAND_DISPLAY"),
  };

  info!("Launching {cmd:?}");
  let spawned = child.spawn();
  let mut child = match spawned {
    Ok(child) => child,
    Err(e) => {
      error!("Could not launch {program}: {e}");
      std::process::exit(127);
    }
  };
  std::thread::spawn(move || {
    let code = match child.wait() {
      Ok(status) => status.code().unwrap_or(1),
      Err(e) => {
        error!("Lost track of the launched program: {e}");
        1
      }
    };
    std::process::exit(code);
  });
}
