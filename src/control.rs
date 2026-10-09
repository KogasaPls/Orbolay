use std::{
  ffi::OsString,
  fs::DirBuilder,
  io::{self, BufRead, BufReader, ErrorKind, Read, Write},
  os::unix::fs::{DirBuilderExt, MetadataExt},
  path::{Path, PathBuf},
  time::Duration,
};

use interprocess::local_socket::{
  GenericFilePath, Listener, ListenerOptions, ToFsName, prelude::*,
};
use orbolay_logging::{debug, error, log, warn};

use crate::keys::KeyEvent;

const TIMEOUT: Duration = Duration::from_secs(1);
const MAX_LINE: u64 = 64;

// Per the XDG Base Directory spec, a relative XDG_RUNTIME_DIR is invalid, and
// when it is unset applications fall back to a similar directory with a warning
fn runtime_dir(xdg_runtime_dir: Option<OsString>, uid: u32) -> (PathBuf, bool) {
  match xdg_runtime_dir.map(PathBuf::from) {
    Some(dir) if dir.is_absolute() => (dir, false),
    _ => (std::env::temp_dir().join(format!("orbolay-{}", uid)), true),
  }
}

// The fallback must match XDG_RUNTIME_DIR: owned by the user, mode 0700
fn ensure_private_dir(dir: &Path, uid: u32) -> io::Result<()> {
  match DirBuilder::new().mode(0o700).create(dir) {
    Err(e) if e.kind() != ErrorKind::AlreadyExists => return Err(e),
    _ => {}
  }

  let meta = std::fs::symlink_metadata(dir)?;
  if !meta.is_dir() || meta.uid() != uid || meta.mode() & 0o777 != 0o700 {
    return Err(io::Error::new(
      ErrorKind::PermissionDenied,
      format!(
        "{} is not a mode 0700 directory owned by uid {}",
        dir.display(),
        uid
      ),
    ));
  }

  Ok(())
}

fn socket_path(create: bool) -> io::Result<PathBuf> {
  let uid = std::fs::metadata("/proc/self")?.uid();
  let (dir, fallback) = runtime_dir(std::env::var_os("XDG_RUNTIME_DIR"), uid);

  if fallback {
    warn!(
      "XDG_RUNTIME_DIR is unset or not absolute; using {}",
      dir.display()
    );
    if create {
      ensure_private_dir(&dir, uid)?;
    }
  }

  Ok(dir.join("orbolay.sock"))
}

fn connect(path: &Path) -> io::Result<LocalSocketStream> {
  let conn = LocalSocketStream::connect(path.to_fs_name::<GenericFilePath>()?)?;
  conn.set_recv_timeout(Some(TIMEOUT))?;
  conn.set_send_timeout(Some(TIMEOUT))?;
  Ok(conn)
}

fn bind(path: &Path) -> io::Result<Listener> {
  // Only remove a socket file that no live instance is listening on
  if path.exists() {
    match connect(path) {
      Ok(_) => {
        return Err(io::Error::new(
          ErrorKind::AddrInUse,
          "another instance is listening",
        ));
      }
      Err(e) if e.kind() == ErrorKind::ConnectionRefused => std::fs::remove_file(path)?,
      Err(e) => return Err(e),
    }
  }

  ListenerOptions::new()
    .name(path.to_fs_name::<GenericFilePath>()?)
    .create_sync()
}

fn handle(conn: LocalSocketStream, keybind_tx: &flume::Sender<KeyEvent>) -> io::Result<()> {
  conn.set_recv_timeout(Some(TIMEOUT))?;
  conn.set_send_timeout(Some(TIMEOUT))?;

  let mut conn = BufReader::new(conn);
  let mut line = String::new();
  if conn.by_ref().take(MAX_LINE).read_line(&mut line)? == 0 {
    return Ok(());
  }

  let reply = match line.trim() {
    "toggle" => {
      debug!("Control command: toggle");
      match keybind_tx.send(KeyEvent::ToggleOverlay) {
        Ok(()) => "ok\n".to_string(),
        Err(_) => "error: overlay is not accepting events\n".to_string(),
      }
    }
    other => format!("error: unknown command {:?}\n", other),
  };

  conn.get_mut().write_all(reply.as_bytes())
}

pub fn listen(keybind_tx: flume::Sender<KeyEvent>) {
  match socket_path(true) {
    Ok(path) => listen_at(path, keybind_tx),
    Err(e) => error!("Control socket disabled: {}", e),
  }
}

fn listen_at(path: PathBuf, keybind_tx: flume::Sender<KeyEvent>) {
  let listener = match bind(&path) {
    Ok(listener) => listener,
    Err(e) => {
      error!("Failed to create control socket {}: {}", path.display(), e);
      return;
    }
  };

  log!("Listening for control commands on {}", path.display());

  std::thread::spawn(move || {
    for conn in listener.incoming() {
      match conn {
        Ok(conn) => {
          let keybind_tx = keybind_tx.clone();
          std::thread::spawn(move || {
            if let Err(e) = handle(conn, &keybind_tx) {
              error!("Control connection failed: {}", e);
            }
          });
        }
        Err(e) => {
          error!("Control socket accept failed: {}", e);
          std::thread::sleep(Duration::from_millis(100));
        }
      }
    }
  });
}

pub fn send(command: &str) -> Result<(), String> {
  let path = socket_path(false).map_err(|e| e.to_string())?;
  send_to(&path, command)
}

fn send_to(path: &Path, command: &str) -> Result<(), String> {
  let conn = connect(path).map_err(|e| format!("cannot connect to {}: {}", path.display(), e))?;

  let mut conn = BufReader::new(conn);
  conn
    .get_mut()
    .write_all(format!("{}\n", command).as_bytes())
    .map_err(|e| e.to_string())?;

  let mut reply = String::new();
  conn
    .take(MAX_LINE)
    .read_line(&mut reply)
    .map_err(|e| match e.kind() {
      ErrorKind::WouldBlock | ErrorKind::TimedOut => {
        "timed out waiting for the running instance".to_string()
      }
      _ => e.to_string(),
    })?;

  match reply.trim() {
    "ok" => Ok(()),
    "" => Err("no reply from the running instance".to_string()),
    other => Err(other.to_string()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("orbolay-{}-{}.sock", name, std::process::id()))
  }

  #[test]
  fn runtime_dir_follows_xdg_spec() {
    let fallback = std::env::temp_dir().join("orbolay-1000");

    assert_eq!(
      runtime_dir(Some("/run/user/1000".into()), 1000),
      (PathBuf::from("/run/user/1000"), false)
    );
    assert_eq!(runtime_dir(None, 1000), (fallback.clone(), true));
    assert_eq!(runtime_dir(Some("".into()), 1000), (fallback.clone(), true));
    assert_eq!(
      runtime_dir(Some("relative/dir".into()), 1000),
      (fallback, true)
    );
  }

  #[test]
  fn fallback_dir_must_be_private() {
    let uid = std::fs::metadata("/proc/self").unwrap().uid();
    let dir = std::env::temp_dir().join(format!("orbolay-private-{}", std::process::id()));

    ensure_private_dir(&dir, uid).unwrap();
    assert_eq!(std::fs::metadata(&dir).unwrap().mode() & 0o777, 0o700);
    ensure_private_dir(&dir, uid).unwrap();
    assert_eq!(
      ensure_private_dir(&dir, uid + 1).map_err(|e| e.kind()),
      Err(ErrorKind::PermissionDenied)
    );

    std::fs::set_permissions(&dir, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    assert_eq!(
      ensure_private_dir(&dir, uid).map_err(|e| e.kind()),
      Err(ErrorKind::PermissionDenied)
    );

    std::fs::remove_dir(&dir).unwrap();
  }

  #[test]
  fn toggle_round_trip() {
    let path = test_path("toggle");
    let (tx, rx) = flume::unbounded();
    listen_at(path.clone(), tx);

    assert_eq!(send_to(&path, "toggle"), Ok(()));
    assert!(matches!(
      rx.recv_timeout(TIMEOUT),
      Ok(KeyEvent::ToggleOverlay)
    ));

    assert_eq!(
      send_to(&path, "bogus"),
      Err("error: unknown command \"bogus\"".to_string())
    );
    assert!(rx.try_recv().is_err());

    let _ = std::fs::remove_file(&path);
  }

  #[test]
  fn silent_client_does_not_block_others() {
    let path = test_path("silent");
    let (tx, rx) = flume::unbounded();
    listen_at(path.clone(), tx);

    let _silent = connect(&path).unwrap();
    assert_eq!(send_to(&path, "toggle"), Ok(()));
    assert!(rx.recv_timeout(TIMEOUT).is_ok());

    let _ = std::fs::remove_file(&path);
  }

  #[test]
  fn live_listener_is_not_displaced() {
    let path = test_path("live");
    let (tx, _rx) = flume::unbounded();
    listen_at(path.clone(), tx);

    assert_eq!(
      bind(&path).err().map(|e| e.kind()),
      Some(ErrorKind::AddrInUse)
    );
    assert_eq!(send_to(&path, "toggle"), Ok(()));

    let _ = std::fs::remove_file(&path);
  }

  #[test]
  fn stale_socket_is_replaced() {
    let path = test_path("stale");
    let stale = ListenerOptions::new()
      .name(path.as_path().to_fs_name::<GenericFilePath>().unwrap())
      .reclaim_name(false)
      .create_sync()
      .unwrap();
    drop(stale);
    assert!(path.exists());

    let (tx, rx) = flume::unbounded();
    listen_at(path.clone(), tx);
    assert_eq!(send_to(&path, "toggle"), Ok(()));
    assert!(rx.recv_timeout(TIMEOUT).is_ok());

    let _ = std::fs::remove_file(&path);
  }
}
