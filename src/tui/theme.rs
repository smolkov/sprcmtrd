//! The TUI's colors and text styles by role, from a theme file
//! ([`crate::term::themes`]): the built-in `dark` and `light` use the 16 ANSI
//! colors, so they follow the terminal's color scheme; the others use RGB.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType};

use crate::term::ThemeKind;
use crate::term::themes::{Border, Catalog, Styles, ThemeSpec};

/// Named colors for all TUI elements.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Dark or light background.
    pub kind: ThemeKind,
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
    /// other a subtle shade of it.
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
    /// Text modifiers of the roles.
    pub styles: Styles,
    /// Border lines of the blocks.
    pub border_type: BorderType,
    /// No colors at all, only modifiers.
    pub plain: bool,
}

impl Theme {
    /// Theme from a theme file. Without `truecolor` the RGB colors are
    /// mapped to the 256-color palette.
    pub fn from_spec(spec: ThemeSpec, truecolor: bool) -> Self {
        let spec = if truecolor { spec } else { spec.to_256_colors() };
        let c = spec.colors;
        Self {
            kind: spec.kind,
            background: c.background,
            text: c.text,
            muted: c.muted,
            border: c.border,
            accent: c.accent,
            selection: c.selection,
            plot_shades: c.plot_shades,
            warning: c.warning,
            error: c.error,
            trigger: c.trigger,
            cursor: c.cursor,
            phase: c.phase,
            palette: c.palette,
            styles: spec.styles,
            border_type: match spec.border {
                Border::Plain => BorderType::Plain,
                Border::Rounded => BorderType::Rounded,
                Border::Thick => BorderType::Thick,
                Border::Double => BorderType::Double,
            },
            plain: false,
        }
    }

    /// Theme for dark backgrounds (built-in `dark`).
    pub fn dark() -> Self {
        Self::from_spec(Catalog::builtin("dark"), true)
    }

    /// Theme for light backgrounds (built-in `light`).
    pub fn light() -> Self {
        Self::from_spec(Catalog::builtin("light"), true)
    }

    /// Theme without colors (`--no-color` / `NO_COLOR`): the modifiers of
    /// `dark`, plain borders.
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
            border_type: BorderType::Plain,
            plain: true,
            ..Self::dark()
        }
    }

    /// A bordered block with the theme's border lines.
    pub fn block(&self) -> Block<'static> {
        Block::bordered().border_type(self.border_type)
    }

    /// Style of a block title: accented when focused.
    pub fn title(&self, focused: bool) -> Style {
        if focused {
            Style::new().fg(self.accent).add_modifier(self.styles.title)
        } else {
            self.base()
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
            Style::new().fg(self.muted).add_modifier(self.styles.muted)
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
            Style::new().bg(self.selection).add_modifier(self.styles.selection)
        }
    }

    /// Style for titles and highlights.
    pub fn accent(&self) -> Style {
        Style::new().fg(self.accent).add_modifier(self.styles.accent)
    }

    /// Style for warnings.
    pub fn warning(&self) -> Style {
        Style::new().fg(self.warning).add_modifier(self.styles.warning)
    }

    /// Style for errors.
    pub fn error(&self) -> Style {
        Style::new().fg(self.error).add_modifier(self.styles.error)
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
