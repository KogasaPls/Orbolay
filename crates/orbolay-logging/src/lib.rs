use std::io::Write;
use std::sync::OnceLock;

use chrono::Local;
use colored::Colorize;

#[derive(Debug, Clone, Copy)]
pub enum LogKind {
  Debug,
  Info,
  Success,
  Warn,
  Error,
}

pub fn log_impl(s: &str, kind: Option<LogKind>) {
  let status = match kind {
    Some(LogKind::Debug) => "DBUG".dimmed(),
    Some(LogKind::Info) | None => "INFO".blue(),
    Some(LogKind::Success) => "DONE".green(),
    Some(LogKind::Warn) => "WARN".yellow(),
    Some(LogKind::Error) => "FAIL".red(),
  };

  let _ = writeln!(
    std::io::stdout().lock(),
    "[{}] [{}] {}",
    Local::now().format("%Y-%m-%d %H:%M:%S"),
    status,
    s
  );
}

pub fn debug_enabled() -> bool {
  static ENABLED: OnceLock<bool> = OnceLock::new();
  *ENABLED
    .get_or_init(|| std::env::var_os("ORBOLAY_DEBUG").is_some_and(|v| !v.is_empty() && v != "0"))
}

#[macro_export]
macro_rules! debug {
  ($($arg:tt)*) => {
    if $crate::debug_enabled() {
      $crate::log_impl(&format!($($arg)*), Some($crate::LogKind::Debug))
    }
  };
}

#[macro_export]
macro_rules! log {
  ($($arg:tt)*) => {
    $crate::log_impl(&format!($($arg)*), Some($crate::LogKind::Info))
  };
}

#[macro_export]
macro_rules! success {
  ($($arg:tt)*) => {
    $crate::log_impl(&format!($($arg)*), Some($crate::LogKind::Success))
  };
}

#[macro_export]
macro_rules! warn {
  ($($arg:tt)*) => {
    $crate::log_impl(&format!($($arg)*), Some($crate::LogKind::Warn))
  };
}

#[macro_export]
macro_rules! error {
  ($($arg:tt)*) => {
    $crate::log_impl(&format!($($arg)*), Some($crate::LogKind::Error))
  };
}
