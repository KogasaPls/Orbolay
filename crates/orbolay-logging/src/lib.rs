use std::fmt::Display;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Local;
use colored::Colorize;

static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn set_debug(enabled: bool) {
  DEBUG.store(enabled, Ordering::Relaxed);
}

pub fn debug_enabled() -> bool {
  DEBUG.load(Ordering::Relaxed)
}

pub enum LogKind {
  Debug,
  Info,
  Success,
  Warn,
  Error,
}

pub fn log_impl(s: impl AsRef<str> + Display, kind: Option<LogKind>) {
  let status = match kind {
    Some(LogKind::Debug) => "DBUG".dimmed(),
    Some(LogKind::Info) => "INFO".blue(),
    Some(LogKind::Success) => "DONE".green(),
    Some(LogKind::Warn) => "WARN".yellow(),
    Some(LogKind::Error) => "FAIL".red(),
    None => "INFO".blue(),
  };

  let _ = writeln!(
    std::io::stdout().lock(),
    "[{}] [{}] {}",
    Local::now().format("%Y-%m-%d %H:%M:%S"),
    status,
    s
  );
}

#[macro_export]
macro_rules! debug {
  ($($arg:tt)*) => {
    if $crate::debug_enabled() {
      $crate::log_impl(format!($($arg)*), Some($crate::LogKind::Debug))
    }
  };
}

#[macro_export]
macro_rules! log {
  ($($arg:tt)*) => {
    $crate::log_impl(format!($($arg)*), Some($crate::LogKind::Info))
  };
}

#[macro_export]
macro_rules! success {
  ($($arg:tt)*) => {
    $crate::log_impl(format!($($arg)*), Some($crate::LogKind::Success))
  };
}

#[macro_export]
macro_rules! warn {
  ($($arg:tt)*) => {
    $crate::log_impl(format!($($arg)*), Some($crate::LogKind::Warn))
  };
}

#[macro_export]
macro_rules! error {
  ($($arg:tt)*) => {
    $crate::log_impl(format!($($arg)*), Some($crate::LogKind::Error))
  };
}
