//! Dark and light themes built from the 16 ANSI colors, so they follow the
//! terminal's color scheme.

use ratatui::style::{Color, Modifier, Style};

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

/// Named colors for all TUI elements.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Background.
    pub background: Color,
    /// Normal text.
    pub text: Color,
    /// Less important text.
    pub muted: Color,
    /// Borders of unfocused blocks.
    pub border: Color,
    /// Focused borders, titles, highlights.
    pub accent: Color,
    /// Background of the selected item.
    pub selection: Color,
    /// Alternating backgrounds of the analog plots: lighter (1st, 3rd, ...)
    /// and darker (2nd, 4th, ...). One of them is the theme background, the
    /// other a subtle gray from the 256-color palette.
    pub plot_shades: [Color; 2],
    /// Warnings and abnormal binary states.
    pub warning: Color,
    /// Errors.
    pub error: Color,
    /// Trigger marker.
    pub trigger: Color,
    /// Plot cursor.
    pub cursor: Color,
    /// Colors for phases A, B, C and N.
    pub phase: [Color; 4],
    /// Colors for channels without a known phase.
    pub palette: [Color; 8],
    /// No colors at all, only modifiers.
    pub plain: bool,
}

impl Theme {
    /// Theme for `kind`, or the colorless theme when `plain`.
    pub fn new(kind: ThemeKind, plain: bool) -> Self {
        if plain {
            return Self::plain();
        }
        match kind {
            ThemeKind::Dark => Self::dark(),
            ThemeKind::Light => Self::light(),
        }
    }

    /// Theme for dark backgrounds.
    pub fn dark() -> Self {
        Self {
            background: Color::Black,
            text: Color::White,
            muted: Color::DarkGray,
            border: Color::Gray,
            accent: Color::Cyan,
            selection: Color::DarkGray,
            plot_shades: [Color::Indexed(234), Color::Black],
            warning: Color::Yellow,
            error: Color::LightRed,
            trigger: Color::Magenta,
            cursor: Color::LightYellow,
            phase: [Color::LightRed, Color::LightGreen, Color::LightBlue, Color::Gray],
            palette: [
                Color::LightCyan,
                Color::LightMagenta,
                Color::Yellow,
                Color::Green,
                Color::LightBlue,
                Color::LightRed,
                Color::Cyan,
                Color::White,
            ],
            plain: false,
        }
    }

    /// Theme for light backgrounds.
    pub fn light() -> Self {
        Self {
            background: Color::White,
            text: Color::Black,
            muted: Color::DarkGray,
            border: Color::DarkGray,
            accent: Color::Blue,
            selection: Color::Gray,
            plot_shades: [Color::White, Color::Indexed(254)],
            warning: Color::Magenta,
            error: Color::Red,
            trigger: Color::Magenta,
            cursor: Color::Red,
            phase: [Color::Red, Color::Green, Color::Blue, Color::DarkGray],
            palette: [
                Color::Blue,
                Color::Magenta,
                Color::Cyan,
                Color::Green,
                Color::Red,
                Color::DarkGray,
                Color::LightBlue,
                Color::Black,
            ],
            plain: false,
        }
    }

    /// Theme without colors (`--no-color` / `NO_COLOR`).
    pub fn plain() -> Self {
        let r = Color::Reset;
        Self {
            background: r,
            text: r,
            muted: r,
            border: r,
            accent: r,
            selection: r,
            plot_shades: [r; 2],
            warning: r,
            error: r,
            trigger: r,
            cursor: r,
            phase: [r; 4],
            palette: [r; 8],
            plain: true,
        }
    }

    /// Base style for all areas.
    pub fn base(&self) -> Style {
        Style::new().fg(self.text).bg(self.background)
    }

    /// Style for secondary text.
    pub fn muted(&self) -> Style {
        if self.plain {
            Style::new().add_modifier(Modifier::DIM)
        } else {
            Style::new().fg(self.muted)
        }
    }

    /// Style for a border, accented when focused.
    pub fn border(&self, focused: bool) -> Style {
        match (focused, self.plain) {
            (true, true) => Style::new().add_modifier(Modifier::BOLD),
            (true, false) => Style::new().fg(self.accent),
            (false, _) => Style::new().fg(self.border),
        }
    }

    /// Style for the selected item.
    pub fn selection(&self) -> Style {
        if self.plain {
            Style::new().add_modifier(Modifier::REVERSED)
        } else {
            Style::new().bg(self.selection).add_modifier(Modifier::BOLD)
        }
    }

    /// Style for titles and highlights.
    pub fn accent(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(Modifier::BOLD)
    }

    /// Style for warnings.
    pub fn warning(&self) -> Style {
        Style::new().fg(self.warning).add_modifier(Modifier::BOLD)
    }

    /// Style for errors.
    pub fn error(&self) -> Style {
        Style::new().fg(self.error).add_modifier(Modifier::BOLD)
    }

    /// Background of the analog plot at visible position `k` (0-based):
    /// lighter for even positions, darker for odd ones.
    pub fn plot_background(&self, k: usize) -> Color {
        self.plot_shades[k % 2]
    }

    /// Color of a channel: by phase when known, else from the palette.
    pub fn channel(&self, phase: &str, position: usize) -> Color {
        let p = phase.trim().to_ascii_uppercase();
        match p.as_str() {
            "A" | "L1" | "R" => self.phase[0],
            "B" | "L2" | "S" => self.phase[1],
            "C" | "L3" | "T" => self.phase[2],
            "N" | "E" | "G" => self.phase[3],
            _ => self.palette[position % self.palette.len()],
        }
    }
}
