use std::{
  ffi::OsString,
  process::{Child, Command},
};

use log::{error, info};

/// Runs `cmd` with `WAYLAND_DISPLAY` restored to its original value and exits Orbolay with the
/// child's exit code when it finishes.
pub fn launch_target(cmd: &[String], wayland_display: Option<OsString>) {
  let Some(child) = spawn(cmd, wayland_display) else {
    return;
  };
  std::thread::spawn(move || wait_and_exit(child));
}

/// Runs `cmd` like [`launch_target`] while another Orbolay instance shows the overlay, blocking
/// until it exits.
pub fn run_target(cmd: &[String], wayland_display: Option<OsString>) -> ! {
  match spawn(cmd, wayland_display) {
    Some(child) => wait_and_exit(child),
    None => std::process::exit(0),
  }
}

fn spawn(cmd: &[String], wayland_display: Option<OsString>) -> Option<Child> {
  let (program, args) = cmd.split_first()?;
  let mut child = Command::new(program);
  child.args(args);
  match wayland_display {
    Some(display) => child.env("WAYLAND_DISPLAY", display),
    None => child.env_remove("WAYLAND_DISPLAY"),
  };

  info!("Launching {cmd:?}");
  match child.spawn() {
    Ok(child) => Some(child),
    Err(e) => {
      error!("Could not launch {program}: {e}");
      std::process::exit(127);
    }
  }
}

fn wait_and_exit(mut child: Child) -> ! {
  let code = match child.wait() {
    Ok(status) => status.code().unwrap_or(1),
    Err(e) => {
      error!("Lost track of the launched program: {e}");
      1
    }
  };
  std::process::exit(code);
}
