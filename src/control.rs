use std::io::{BufRead, BufReader, Write};

use interprocess::local_socket::{GenericFilePath, ListenerOptions, ToFsName, prelude::*};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Command {
  ToggleOverlay,
  Mute,
  Deafen,
}

pub fn socket_path() -> Option<std::path::PathBuf> {
  Some(dirs::runtime_dir()?.join("orbolay").join("orbolay.sock"))
}

pub fn start(handler: impl Fn(Command) + Send + Sync + 'static) {
  let handler = std::sync::Arc::new(handler);

  std::thread::spawn(move || {
    let listener = match make_listener() {
      Ok(listener) => listener,
      Err(e) => {
        error!("Failed to create the control socket: {e}");
        return;
      }
    };

    info!("Control socket listening");

    loop {
      match listener.accept() {
        Ok(stream) => {
          let handler = handler.clone();
          std::thread::spawn(move || {
            serve(BufReader::new(stream), handler.as_ref());
            debug!("Control socket client disconnected");
          });
        }
        Err(e) => {
          error!("Control listener error: {e}");
          break;
        }
      }
    }
  });
}

fn make_listener() -> Result<LocalSocketListener, Box<dyn std::error::Error>> {
  let path = socket_path().ok_or("Could not determine socket path")?;
  if let Some(parent) = path.parent() {
    std::fs::create_dir_all(parent)?;
  }
  let _ = std::fs::remove_file(&path);
  let name = path.to_fs_name::<GenericFilePath>()?;
  Ok(ListenerOptions::new().name(name).create_sync()?)
}

/// Runs `handler` for each JSON command line until the client disconnects.
fn serve(reader: impl BufRead, handler: &impl Fn(Command)) {
  for line in reader.lines() {
    let line = match line {
      Ok(line) => line,
      Err(e) => {
        warn!("Control socket read failed: {e}");
        return;
      }
    };
    let line = line.trim();
    if line.is_empty() {
      continue;
    }
    match serde_json::from_str::<Command>(line) {
      Ok(cmd) => {
        debug!("Received control command: {cmd:?}");
        handler(cmd);
      }
      Err(e) => warn!("Invalid control command: {e}"),
    }
  }
}

pub struct Client {
  pub stream: LocalSocketStream,
}

impl Client {
  pub fn connect() -> Result<Self, Box<dyn std::error::Error>> {
    let path = socket_path().ok_or("Could not determine socket path")?;
    let stream = LocalSocketStream::connect(path.to_fs_name::<GenericFilePath>()?)?;
    Ok(Self { stream })
  }

  pub fn send(&mut self, cmd: Command) -> Result<(), Box<dyn std::error::Error>> {
    let mut payload = serde_json::to_string(&cmd)?;
    payload.push('\n');
    self.stream.write_all(payload.as_bytes())?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, io::Cursor};

  use super::{Command, serve};

  #[test]
  fn each_valid_line_runs_one_command() {
    let received = RefCell::new(Vec::new());
    let input = "\"toggle-overlay\"\n\n  \nnot json\n\"mute\"\n\"deafen\"";
    serve(Cursor::new(input), &|cmd| received.borrow_mut().push(cmd));
    assert_eq!(
      *received.borrow(),
      [Command::ToggleOverlay, Command::Mute, Command::Deafen]
    );
  }
}
