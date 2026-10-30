//! Time axis below the plots: tick labels, trigger and cursor markers.

use chrono::TimeDelta;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::format::time;
use crate::tui::app::App;
use crate::tui::recording::Recording;

/// Column of time `t` within a plot `width` cells wide, if visible.
pub(super) fn column_of(rec: &Recording, t: f64, width: u16) -> Option<u16> {
    let (t0, t1) = rec.view;
    if !(t0..=t1).contains(&t) || t1 <= t0 {
        return None;
    }
    Some(
        (((t - t0) / (t1 - t0)) * f64::from(width))
            .floor()
            .min(f64::from(width - 1)) as u16,
    )
}

fn time_label(app: &App, rec: &Recording, t: f64, span: f64) -> String {
    if app.absolute_time {
        let trigger = rec.data.config.trigger_time;
        let abs = trigger + TimeDelta::nanoseconds((t * 1e9).round() as i64);
        abs.format("%H:%M:%S%.6f").to_string()
    } else {
        time(t, span)
    }
}

/// Number of decimals that show every sample time in ms exactly
/// (1 kHz: 0, 2 kHz: 1, 4 kHz: 2), at most 3 (µs).
fn ms_decimals(rec: &Recording) -> usize {
    let (first, last) = rec.data.time_span();
    let n = rec.data.len();
    if n < 2 || last <= first {
        return 3;
    }
    let period_ms = (last - first) / (n - 1) as f64 * 1e3;
    (0..3)
        .find(|&d| {
            let scaled = period_ms * 10f64.powi(d as i32);
            (scaled - scaled.round()).abs() < 1e-6 * scaled.max(1.0)
        })
        .unwrap_or(3)
}

/// Cursor time: milliseconds relative to the trigger with sample
/// resolution, or the absolute time of day.
pub(super) fn cursor_label(app: &App, rec: &Recording) -> String {
    if app.absolute_time {
        time_label(app, rec, rec.cursor, 0.0)
    } else {
        format!("{:.d$} ms", rec.cursor * 1e3, d = ms_decimals(rec))
    }
}

/// Bottom row: tick labels, the trigger marker `T` and the cursor marker
/// with its time. The cursor label is always drawn; tick labels that would
/// overlap it are skipped.
pub(super) fn render(f: &mut Frame, area: Rect, app: &App, rec: &Recording) {
    let theme = &app.theme;
    let (t0, t1) = rec.view;
    let span = t1 - t0;
    let width = area.width as usize;
    let mut chars = vec![' '; width];

    // Cursor label next to its marker, flipped left near the right edge.
    let label = cursor_label(app, rec);
    let cursor_text: Vec<char> = match column_of(rec, rec.cursor, area.width) {
        Some(c) if c as usize + label.chars().count() + 2 <= width => format!("▲ {label}"),
        Some(_) => format!("{label} ▲"),
        None if rec.cursor < t0 => format!("◀ {label}"),
        None => format!("{label} ▶"),
    }
    .chars()
    .collect();
    let len = cursor_text.len().min(width);
    let cursor_start = match column_of(rec, rec.cursor, area.width) {
        Some(c) if cursor_text[0] == '▲' => c as usize,
        Some(c) => (c as usize + 1).saturating_sub(len),
        None if rec.cursor < t0 => 0,
        None => width - len,
    };
    let cursor_range = cursor_start..(cursor_start + len).min(width);
    for (slot, &ch) in chars[cursor_range.clone()].iter_mut().zip(&cursor_text) {
        *slot = ch;
    }

    let trigger_col = column_of(rec, 0.0, area.width).filter(|&c| chars[c as usize] == ' ');
    if let Some(c) = trigger_col {
        chars[c as usize] = 'T';
    }

    let ticks = if app.absolute_time { 3 } else { 5 };
    for k in 0..ticks {
        let frac = k as f64 / (ticks - 1) as f64;
        let label: Vec<char> = time_label(app, rec, t0 + span * frac, span).chars().collect();
        let pos = (frac * (width - 1) as f64).round() as usize;
        let start = if k == 0 {
            0
        } else if k == ticks - 1 {
            width.saturating_sub(label.len())
        } else {
            pos.saturating_sub(label.len() / 2)
        };
        // Skip labels closer than two spaces to the cursor label, a marker or
        // another label, so neighbouring times are not read as one.
        let around = start.saturating_sub(2)..(start + label.len() + 2).min(width);
        if chars[around].iter().any(|&c| c != ' ') {
            continue;
        }
        for (j, &ch) in label.iter().enumerate() {
            if let Some(slot) = chars.get_mut(start + j) {
                *slot = ch;
            }
        }
    }
    let text: String = chars.into_iter().collect();
    f.render_widget(Paragraph::new(Span::styled(text, theme.muted())), area);

    let buf = f.buffer_mut();
    if let Some(c) = trigger_col {
        buf[(area.x + c, area.y)].set_style(Style::new().fg(theme.trigger).add_modifier(Modifier::BOLD));
    }
    let cursor_style = Style::new().fg(theme.cursor).add_modifier(Modifier::BOLD);
    for x in cursor_range {
        buf[(area.x + x as u16, area.y)].set_style(cursor_style);
    }
}
