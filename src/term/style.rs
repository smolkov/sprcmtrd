//! Colors for command output.
//!
//! Colors are only an extra cue: every status also has a word and a symbol,
//! so output stays readable without color (and for color blind readers).
//! OK uses the theme's accent (cyan / blue) instead of green, so OK and
//! errors are not told apart by red versus green alone. Colors are used only
//! on a terminal, and never with `--no-color` or `NO_COLOR`.

use std::io::IsTerminal;

use crossterm::style::{Color, Stylize};

use crate::args::ThemeChoice;
use crate::tui::theme::ThemeKind;

/// What a piece of text stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Something that failed.
    Error,
    /// Something to look at.
    Warning,
    /// Something that passed.
    Ok,
    /// Extra information.
    Note,
    /// Column headers.
    Header,
}

/// Colors for one output stream, matching the TUI theme (dark or light).
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    enabled: bool,
    kind: ThemeKind,
}

impl Palette {
    /// Palette for stdout: colors when it is a terminal and not
    /// `colors_disabled` (`--no-color`, `NO_COLOR`), light or dark from `theme`.
    pub fn stdout(theme: ThemeChoice, colors_disabled: bool) -> Self {
        Self {
            enabled: std::io::stdout().is_terminal() && !colors_disabled,
            kind: ThemeKind::resolve(theme),
        }
    }

    /// Palette without colors.
    pub fn plain() -> Self {
        Self {
            enabled: false,
            kind: ThemeKind::Dark,
        }
    }

    /// `text` in the color of `role`, or unchanged without colors.
    pub fn paint(&self, text: &str, role: Role) -> String {
        if !self.enabled {
            return text.to_owned();
        }
        let light = self.kind == ThemeKind::Light;
        // The 16 ANSI colors, so the terminal's color scheme applies; the
        // same choices as the TUI theme (yellow is hard to read on white).
        let styled = match role {
            Role::Error => text.with(if light { Color::DarkRed } else { Color::Red }).bold(),
            Role::Warning => text.with(if light { Color::DarkMagenta } else { Color::DarkYellow }),
            Role::Ok => text.with(if light { Color::DarkBlue } else { Color::DarkCyan }),
            Role::Note => text.with(Color::DarkGrey),
            Role::Header => text.bold(),
        };
        styled.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_output_has_no_escape_codes() {
        assert_eq!(Palette::plain().paint("ERROR", Role::Error), "ERROR");
        let dark = Palette {
            enabled: true,
            kind: ThemeKind::Dark,
        };
        let s = dark.paint("ERROR", Role::Error);
        assert!(
            s.contains("ERROR") && s.starts_with('\x1b') && s.ends_with('m'),
            "{s:?}"
        );
        assert_ne!(
            dark.paint("x", Role::Warning),
            Palette {
                kind: ThemeKind::Light,
                ..dark
            }
            .paint("x", Role::Warning)
        );
    }
}
