//! Configuration overlay: theme and key bindings, saved to the config file.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::{centered, overlay_block};
use crate::term::ThemeKind;
use crate::tui::app::{App, BINDINGS, ConfigState, StatusKind};

pub fn render(f: &mut Frame, area: Rect, app: &App, state: &ConfigState) {
    let theme = &app.theme;
    let path = app
        .config_path
        .as_ref()
        .map_or_else(|| "not saved".to_owned(), |p| p.display().to_string());
    let inner = overlay_block(f, centered(area, 75, 85), app, &format!("Configuration · {path}"));
    let [list_area, _, message_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1), Constraint::Length(1)]).areas(inner);

    let name_width = BINDINGS
        .iter()
        .map(|b| b.name.len())
        .max()
        .unwrap_or(0)
        .max("Theme".len());
    let keys_width = 20;
    let header = |text: &'static str| ListItem::new(Line::styled(text, theme.accent()));

    let theme_value = match app.theme_kind {
        ThemeKind::Dark => "◀ dark ▶",
        ThemeKind::Light => "◀ light ▶",
    };
    let mut items = vec![
        header("Appearance"),
        ListItem::new(Line::from(vec![
            Span::raw(format!("  {:<name_width$}  ", "Theme")),
            Span::styled(format!("{theme_value:<keys_width$}"), theme.base()),
            Span::styled("dark or light background", theme.muted()),
        ])),
        ListItem::new(""),
        header("Key bindings"),
    ];
    for (i, b) in BINDINGS.iter().enumerate() {
        let capturing = state.capturing && state.selected == i + 1;
        let keys = if capturing {
            Span::styled(format!("{:<keys_width$}", "press a key…"), theme.warning())
        } else {
            let keys: Vec<String> = app.keymap.keys(b.action).iter().map(ToString::to_string).collect();
            Span::styled(format!("{:<keys_width$}", keys.join(", ")), theme.base())
        };
        items.push(ListItem::new(Line::from(vec![
            Span::raw(format!("  {:<name_width$}  ", b.name)),
            keys,
            Span::styled(b.help, theme.muted()),
        ])));
    }

    // Rows: theme is list item 1, binding i is list item i + 4 (after the headers).
    let row = if state.selected == 0 { 1 } else { state.selected + 3 };
    let mut list_state = ListState::default().with_selected(Some(row));
    f.render_stateful_widget(
        List::new(items).highlight_style(theme.selection()),
        list_area,
        &mut list_state,
    );

    let message = match &state.message {
        Some((text, StatusKind::Info)) => Line::styled(text.clone(), theme.muted()),
        Some((text, StatusKind::Error)) => Line::styled(text.clone(), theme.error()),
        None if state.capturing => Line::styled("Press the new key, Esc cancels", theme.warning()),
        None if state.selected == 0 => {
            Line::styled("Enter / ←→ switch theme · changes are saved immediately", theme.muted())
        }
        None => Line::styled(
            "Enter change key · Backspace reset to default · changes are saved immediately",
            theme.muted(),
        ),
    };
    f.render_widget(Paragraph::new(message), message_area);
}
