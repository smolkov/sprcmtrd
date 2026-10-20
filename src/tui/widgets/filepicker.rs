//! File picker overlay.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState};

use super::{centered, overlay_block};
use crate::tui::app::{App, Picker, PickerEntry};

const SPINNER: [char; 4] = ['◐', '◓', '◑', '◒'];

pub fn render(f: &mut Frame, area: Rect, app: &App, p: &Picker) {
    let theme = &app.theme;
    let title = format!("Open recording · {}", p.dir.display());
    let inner = overlay_block(f, centered(area, 85, 75), app, &title);
    let [input, list, bottom] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(inner);

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
            PickerEntry::Dir { name, .. } => {
                Row::new(vec![Span::styled(format!("▸ {name}"), theme.accent())])
            }
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
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(30),
            Constraint::Length(19),
            Constraint::Length(9),
        ],
    )
    .row_highlight_style(theme.selection());
    let mut state = TableState::default().with_selected((!empty).then_some(p.selected));
    f.render_stateful_widget(table, list, &mut state);

    let message = if let Some(e) = &p.error {
        Line::styled(e.clone(), theme.error())
    } else if empty && !p.scanning {
        Line::styled("no COMTRADE recordings here", theme.muted())
    } else {
        Line::styled(
            format!("{} of {} entries", p.filtered.len(), p.entries.len()),
            Style::new().patch(theme.muted()),
        )
    };
    f.render_widget(Paragraph::new(message), bottom);
}
