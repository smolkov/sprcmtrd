//! Key help overlay, generated from the binding table.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::{centered, overlay_block, scrolled_text};
use crate::tui::app::App;
use crate::tui::keys::{Action, BINDINGS};

pub fn render(f: &mut Frame, area: Rect, app: &App, scroll: u16) {
    let inner = overlay_block(f, centered(area, 70, 80), app, "Help");
    let theme = &app.theme;

    let keys_of = |action: Action| {
        app.keymap
            .keys(action)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    };
    // The nine harmonic orders share one line.
    let rows: Vec<(String, &str)> = BINDINGS
        .iter()
        .filter_map(|b| match b.action {
            Action::Harmonic(1) => {
                let all: Vec<String> = (1..=9).map(|n| keys_of(Action::Harmonic(n))).collect();
                let default = all.iter().enumerate().all(|(i, k)| *k == (i + 1).to_string());
                let keys = if default { "1 … 9".to_owned() } else { all.join(" ") };
                Some((keys, "Harmonics view: show / hide order 1 … 9"))
            }
            Action::Harmonic(_) => None,
            action => Some((keys_of(action), b.help)),
        })
        .collect();
    let width = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);

    let mut lines: Vec<Line<'static>> = rows
        .into_iter()
        .map(|(k, help)| {
            Line::from(vec![
                Span::styled(format!("  {k:<width$}  "), theme.accent()),
                Span::raw(help.to_owned()),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::styled("  File picker", theme.accent()));
    for text in [
        "Searches all subdirectories; Ctrl+r toggles this directory only",
        "Ctrl+b toggles showing only valid recordings (configuration and data load)",
        "Type to filter by path (case-insensitive substring)",
        "Start the filter with / to use a regular expression",
        "↑/↓ select, Ctrl+↑/↓ or PgUp/PgDn move 10, Home/End first and last",
        "Enter opens a file or directory",
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
