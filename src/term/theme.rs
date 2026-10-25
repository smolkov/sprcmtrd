//! Light or dark terminal background, shared by the command output colors
//! and the TUI themes.

use crate::args::ThemeChoice;

/// Which theme is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// Dark background.
    Dark,
    /// Light background.
    Light,
}

impl ThemeKind {
    /// Resolves `auto` using the `COLORFGBG` variable many terminals set
    /// (`"15;0"` = light text on dark background).
    pub fn resolve(choice: ThemeChoice) -> Self {
        match choice {
            ThemeChoice::Dark => Self::Dark,
            ThemeChoice::Light => Self::Light,
            ThemeChoice::Auto => {
                let bg = std::env::var("COLORFGBG")
                    .ok()
                    .and_then(|v| v.rsplit(';').next().and_then(|b| b.parse::<u8>().ok()));
                match bg {
                    Some(7 | 15) => Self::Light,
                    _ => Self::Dark,
                }
            }
        }
    }

    /// The other theme.
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}
