//! `sprcmtrd`: terminal viewer for COMTRADE (IEEE C37.111) recordings.

pub mod args;
pub mod command;
pub mod complete;
pub mod comtrade;
pub mod config;
pub mod error;
pub mod logging;
pub mod tui;

pub use comtrade::Comtrade;
pub use error::Error;
