//! Key help overlay, generated from the binding table.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::{centered, overlay_block, scrolled_text};
use crate::tui::app::{App, BINDINGS};

pub fn render(f: &mut Frame, area: Rect, app: &App, scroll: u16) {
    let inner = overlay_block(f, centered(area, 70, 80), app, "Help");
    let theme = &app.theme;

    let keys: Vec<String> = BINDINGS
        .iter()
        .map(|b| {
            app.keymap
                .keys(b.action)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect();
    let width = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);

    let mut lines: Vec<Line<'static>> = BINDINGS
        .iter()
        .zip(keys)
        .map(|(b, k)| {
            Line::from(vec![
                Span::styled(format!("  {k:<width$}  "), theme.accent()),
                Span::raw(b.help),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled("  File picker", theme.accent()));
    for text in [
        "Searches all subdirectories; Ctrl+r toggles this directory only",
        "Type to filter by path (case-insensitive substring)",
        "Start the filter with / to use a regular expression",
        "↑/↓ select, Enter open file or directory",
        "Backspace on an empty filter goes to the parent directory",
        "Esc closes the picker",
    ] {
        lines.push(Line::from(format!("  {text}")));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "  Keys can be changed in ~/.config/sprcmtrd/config.toml ([keys] section)",
        theme.muted(),
    ));
    scrolled_text(f, inner, app, lines, scroll);
}
