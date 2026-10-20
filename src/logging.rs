//! Logging setup.
//!
//! CLI commands log to stderr. The TUI logs to a file (never to the
//! screen), and only when `-v` or `RUST_LOG` is given.

use std::path::PathBuf;

use tracing_subscriber::EnvFilter;

/// Where log output goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Standard error (CLI commands).
    Stderr,
    /// Log file (TUI).
    File,
}

fn filter(verbose: u8) -> EnvFilter {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(format!("sprcmtrd={level}")))
}

/// Default log file location.
pub fn log_file() -> Option<PathBuf> {
    dirs::state_dir()
        .or_else(dirs::cache_dir)
        .map(|d| d.join("sprcmtrd").join("sprcmtrd.log"))
}

/// Installs the global logger. Returns the log file path when logging to a file.
pub fn init(verbose: u8, target: Target) -> Option<PathBuf> {
    match target {
        Target::Stderr => {
            tracing_subscriber::fmt()
                .with_env_filter(filter(verbose))
                .with_writer(std::io::stderr)
                .without_time()
                .with_target(false)
                .init();
            None
        }
        Target::File => {
            if verbose == 0 && std::env::var_os("RUST_LOG").is_none() {
                return None;
            }
            let path = log_file()?;
            std::fs::create_dir_all(path.parent()?).ok()?;
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()?;
            tracing_subscriber::fmt()
                .with_env_filter(filter(verbose))
                .with_writer(std::sync::Mutex::new(file))
                .with_ansi(false)
                .init();
            Some(path)
        }
    }
}
