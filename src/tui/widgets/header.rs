//! Header: application name on the left, current recording on the right.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::format::rate;
use crate::tui::app::App;

// const BANNER: [&str; 4] = [
//     " ▄▄▄ ▄▄▄  ▄ ▄▄  ▄▄▄ ▄▄▄▄   ▄▄█ ▄ ▄▄  ▄▄█ ",
//     "▀▀▄▄ █  █ █▀   █    █ █ █ █  █ █▀   █  █ ",
//     "▀▀▀  █▀▀  ▀     ▀▀▀ ▀ ▀ ▀  ▀▀▀ ▀     ▀▀▀ ",
//     "     ▀                                   ",
// ];

const BANNER: [&str; 3] = [
    "┌─┐┌─┐┬─┐┌─┐┌┬┐┌┬┐┬─┐┌┬┐",
    "└─┐├─┘├┬┘│  │││ │ ├┬┘ ││",
    "└─┘┴  ┴└─└─┘┴ ┴ ┴ ┴└──┴┘",
];
const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let big = area.height >= 3;

    let left: Vec<Line> = if big {
        BANNER
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut spans = vec![Span::styled(*row, theme.accent())];
                if i == 2 {
                    spans.push(Span::styled(format!(" {VERSION}"), theme.muted()));
                }
                Line::from(spans)
            })
            .collect()
    } else {
        vec![Line::from(vec![
            Span::styled("sprcmtrd", theme.accent()),
            Span::styled(format!(" {VERSION}"), theme.muted()),
        ])]
    };
    f.render_widget(Paragraph::new(left), area);

    let Some(rec) = &app.recording else {
        return;
    };
    let c = &rec.data.config;
    let rate = c
        .sample_rates
        .first()
        .filter(|r| r.rate > 0.0)
        .map_or_else(|| "timestamps".to_owned(), |r| rate(r.rate));
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let right: Vec<Line> = if big {
        vec![
            Line::styled(rec.name.clone(), bold),
            Line::from(format!("{} · {}", c.station_name, c.device_id)),
            Line::styled(
                format!(
                    "{} · {} · {} · {} samples",
                    c.revision,
                    c.data_format,
                    rate,
                    rec.data.len()
                ),
                theme.muted(),
            ),
        ]
    } else {
        vec![Line::from(vec![
            Span::styled(rec.name.clone(), bold),
            Span::styled(
                format!(" · {} · {} · {}", c.station_name, c.revision, rate),
                theme.muted(),
            ),
        ])]
    };
    // Keep clear of the banner on the left.
    let offset = if big { BANNER[0].chars().count() as u16 + 10 } else { 16 };
    let right_area = Rect {
        x: area.x + offset,
        width: area.width.saturating_sub(offset),
        ..area
    };
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), right_area);
}
