//! `sprcmtrd`: terminal viewer for COMTRADE (IEEE C37.111) recordings.

pub mod analysis;
pub mod args;
pub mod command;
pub mod complete;
pub mod comtrade;
pub mod config;
pub mod context;
pub mod error;
pub mod format;
pub mod logging;
pub mod term;
pub mod tui;

pub use comtrade::Comtrade;
pub use error::Error;
