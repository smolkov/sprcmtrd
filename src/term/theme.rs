//! Light or dark terminal background, shared by the command output colors
//! and the TUI themes.

use serde::{Deserialize, Serialize};

/// Whether a theme is made for a dark or a light background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeKind {
    /// Dark background.
    Dark,
    /// Light background.
    Light,
}

impl ThemeKind {
    /// The background reported by the `COLORFGBG` variable many terminals
    /// set (`"15;0"` = light text on dark background), dark when unknown.
    pub fn detect() -> Self {
        let bg = std::env::var("COLORFGBG")
            .ok()
            .and_then(|v| v.rsplit(';').next().and_then(|b| b.parse::<u8>().ok()));
        match bg {
            Some(7 | 15) => Self::Light,
            _ => Self::Dark,
        }
    }

    /// Name of the built-in ANSI theme for this background.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}
