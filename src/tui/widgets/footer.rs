//! Footer: key hints on the left, status on the right.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::command::bytes;
use crate::tui::app::{App, BINDINGS, Overlay, StatusKind};

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let (status, style) = if let Some(path) = &app.loading {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        (
            format!("{} loading {name}", SPINNER[app.frame % SPINNER.len()]),
            theme.accent(),
        )
    } else if let Some(s) = &app.status {
        let style = match s.kind {
            StatusKind::Info => theme.base(),
            StatusKind::Error => theme.error(),
        };
        (s.text.clone(), style)
    } else if let Some(rec) = &app.recording {
        let size = rec.size.map(|s| format!(" · {}", bytes(s))).unwrap_or_default();
        (format!("{} samples{size}", rec.data.len()), theme.muted())
    } else {
        (String::new(), theme.base())
    };
    let status_width = (status.chars().count() as u16 + 1).min(area.width / 2);

    let hints: Vec<Span> = match &app.overlay {
        Some(Overlay::Picker(_)) => picker_hints(app),
        Some(Overlay::Config(state)) if state.capturing => key_hint(app, "Esc", "cancel").to_vec(),
        Some(Overlay::Config(_)) => [
            ("↑↓", "select"),
            ("Enter", "change"),
            ("Backspace", "default"),
            ("Esc", "close"),
        ]
        .iter()
        .flat_map(|(k, l)| key_hint(app, k, l))
        .collect(),
        Some(_) => key_hint(app, "Esc", "close")
            .into_iter()
            .chain(key_hint(app, "↑↓", "scroll"))
            .collect(),
        None => BINDINGS
            .iter()
            .filter_map(|b| b.hint.map(|h| (b.action, h)))
            .flat_map(|(action, hint)| key_hint(app, &app.keymap.label(action), hint))
            .collect(),
    };
    let hints_area = Rect {
        width: area.width.saturating_sub(status_width),
        ..area
    };
    f.render_widget(Paragraph::new(Line::from(hints)), hints_area);

    let status_area = Rect {
        x: area.x + area.width - status_width,
        width: status_width,
        ..area
    };
    f.render_widget(
        Paragraph::new(Span::styled(status, style)).alignment(Alignment::Right),
        status_area,
    );
}

fn key_hint(app: &App, key: &str, label: &str) -> [Span<'static>; 2] {
    [
        Span::styled(key.to_owned(), app.theme.accent()),
        Span::styled(format!(" {label}  "), app.theme.muted()),
    ]
}

fn picker_hints(app: &App) -> Vec<Span<'static>> {
    let Some(Overlay::Picker(p)) = &app.overlay else {
        return Vec::new();
    };
    // Arrow keys and the /regex filter are left to the help overlay, so the
    // hints fit in 80 columns.
    let toggle = if p.recursive { "this dir" } else { "subdirs" };
    let valid = if p.valid_only { "show all" } else { "valid only" };
    [
        ("Enter", "open"),
        ("Backspace", "parent"),
        ("Ctrl+r", toggle),
        ("Ctrl+b", valid),
        ("Esc", "close"),
    ]
    .iter()
    .flat_map(|(k, l)| key_hint(app, k, l))
    .collect()
}
