//! File picker overlay.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState};

use super::{centered, overlay_block};
use crate::tui::app::App;
use crate::tui::picker::{Picker, PickerEntry};

const SPINNER: [char; 4] = ['◐', '◓', '◑', '◒'];

pub fn render(f: &mut Frame, area: Rect, app: &App, p: &Picker) {
    let theme = &app.theme;
    let mode = if p.recursive {
        "with subdirectories"
    } else {
        "this directory only"
    };
    let shown = if p.valid_only { " · valid only" } else { "" };
    // Modes before the directory, so a long path can't push them out.
    let title = format!("Open recording · {mode}{shown} · {}", p.dir.display());
    let inner = overlay_block(f, centered(area, 85, 75), app, &title);
    let [input, list, bottom] =
        Layout::vertical([Constraint::Length(2), Constraint::Min(0), Constraint::Length(1)]).areas(inner);

    let mut input_line = vec![
        Span::styled("› ", theme.accent()),
        Span::raw(p.query.clone()),
        Span::styled("▏", theme.accent()),
    ];
    if p.scanning {
        input_line.push(Span::styled(
            format!("  {} scanning", SPINNER[app.frame % SPINNER.len()]),
            theme.muted(),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(input_line)), input);

    let rows: Vec<Row> = p
        .filtered
        .iter()
        .map(|&i| match &p.entries[i] {
            PickerEntry::Dir { name, .. } => Row::new(vec![Span::styled(format!("▸ {name}"), theme.accent())]),
            PickerEntry::Recording { name, details, .. } => match details {
                Ok((station, start, a, d)) => Row::new(vec![
                    Span::raw(format!("  {name}")),
                    Span::raw(station.clone()),
                    Span::styled(start.clone(), theme.muted()),
                    Span::styled(format!("{a}A {d}D"), theme.muted()),
                ]),
                Err(e) => Row::new(vec![
                    Span::raw(format!("  {name}")),
                    Span::styled(e.clone(), theme.error()),
                ]),
            },
        })
        .collect();
    let empty = rows.is_empty();
    let total = rows.len();
    // A column for the scrollbar when the rows don't fit.
    let scrolls = total > list.height as usize && list.height > 0;
    let (list, track) = if scrolls {
        let [list, track] = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(list);
        (list, Some(track))
    } else {
        (list, None)
    };
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Length(19),
            Constraint::Length(9),
        ],
    )
    .row_highlight_style(theme.selection());
    let mut state = TableState::default().with_selected((!empty).then_some(p.selected));
    f.render_stateful_widget(table, list, &mut state);
    if let Some(track) = track {
        super::scrollbar(f, track, (total, list.height as usize, state.offset()), theme, true);
    }

    let message = if let Some(e) = &p.error {
        Line::styled(e.clone(), theme.error())
    } else if empty && !p.scanning {
        let what = if p.valid_only {
            "valid COMTRADE recordings"
        } else {
            "COMTRADE recordings"
        };
        Line::styled(
            if p.recursive {
                format!("no {what} here or in subdirectories")
            } else {
                format!("no {what} here (Ctrl+r searches subdirectories)")
            },
            theme.muted(),
        )
    } else {
        // The selected entry's position; the number found when a filter
        // hides some.
        let position = format!("{} / {}", p.selected + 1, p.filtered.len());
        let text = if p.entries.is_empty() {
            "0 entries".to_owned()
        } else if p.filtered.is_empty() {
            format!("none of {} entries match", p.entries.len())
        } else if p.filtered.len() == p.entries.len() {
            format!("{position} entries")
        } else {
            format!("{position} · {} entries", p.entries.len())
        };
        Line::styled(text, Style::new().patch(theme.muted()))
    };
    f.render_widget(Paragraph::new(message), bottom);
}
