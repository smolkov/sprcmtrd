//! Plot area: analog line plots, binary status strips and a shared time axis.

use chrono::TimeDelta;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};
use ratatui::widgets::{Block, Paragraph};

use crate::command::{si, time};
use crate::comtrade::Cell;
use crate::tui::app::{Action, App, Focus, Recording};

/// Width of the label column left of each plot.
const GUTTER: u16 = 16;
/// Minimum height of an analog plot.
const MIN_ANALOG_HEIGHT: u16 = 3;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Plot && app.recording.is_some();
    let mut block = Block::bordered()
        .title(" Plot ")
        .title_style(if focused { theme.accent() } else { theme.base() })
        .border_style(theme.border(focused));

    let Some(rec) = &app.recording else {
        let inner = block.inner(area);
        f.render_widget(block, area);
        empty_message(f, inner, app);
        return;
    };

    let (t0, t1) = rec.view;
    let span = t1 - t0;
    block = block.title(
        Line::from(format!(
            " cursor {} · window {} ",
            cursor_label(app, rec),
            time(span, span)
        ))
        .alignment(Alignment::Right),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width <= GUTTER + 4 || inner.height < 2 {
        return;
    }

    let analog: Vec<usize> = (0..rec.visible_analog.len())
        .filter(|&i| rec.visible_analog[i])
        .collect();
    let binary: Vec<usize> = (0..rec.visible_binary.len())
        .filter(|&i| rec.visible_binary[i])
        .collect();
    if analog.is_empty() && binary.is_empty() {
        let key = app.keymap.label(Action::ToggleChannel);
        let msg = format!("No channels shown. Select channels in the sidebar and press {key}.");
        centered_text(f, inner, &msg, theme.muted());
        return;
    }

    // Row budget: bottom row is the time axis, binary strips take one row
    // each, analog plots share the rest (at least MIN_ANALOG_HEIGHT each).
    let rows = inner.height - 1;
    let reserve = if analog.is_empty() { 0 } else { MIN_ANALOG_HEIGHT };
    let n_binary = binary.len().min(rows.saturating_sub(reserve) as usize);
    let analog_rows = rows - n_binary as u16;
    let n_analog = analog.len().min((analog_rows / MIN_ANALOG_HEIGHT) as usize);
    let hidden = analog.len() - n_analog + binary.len() - n_binary;

    let canvas_x = inner.x + GUTTER;
    let canvas_width = inner.width - GUTTER;
    let cursor_idx = rec.data.index_at(rec.cursor);

    let mut y = inner.y;
    for (k, &i) in analog.iter().take(n_analog).enumerate() {
        let base = analog_rows / n_analog as u16;
        let height = if k + 1 == n_analog {
            analog_rows - base * (n_analog as u16 - 1)
        } else {
            base
        };
        let row = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height,
        };
        analog_plot(f, row, app, rec, i, cursor_idx, app.theme.plot_background(k));
        y += height;
    }
    for &i in binary.iter().take(n_binary) {
        let row = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: 1,
        };
        binary_strip(f, row, app, rec, i, cursor_idx);
        y += 1;
    }

    let axis = Rect {
        x: canvas_x,
        y: inner.y + inner.height - 1,
        width: canvas_width,
        height: 1,
    };
    time_axis(f, axis, app, rec);
    let note = if hidden > 0 {
        format!("+{hidden} hidden")
    } else {
        String::new()
    };
    f.render_widget(
        Paragraph::new(Span::styled(note, theme.warning())),
        Rect {
            x: inner.x,
            y: axis.y,
            width: GUTTER,
            height: 1,
        },
    );
}

fn empty_message(f: &mut Frame, area: Rect, app: &App) {
    let msg = match &app.loading {
        Some(p) => format!("Loading {} …", p.display()),
        None => format!(
            "No recording loaded. Press {} to open a COMTRADE file.",
            app.keymap.label(Action::Open)
        ),
    };
    centered_text(f, area, &msg, app.theme.muted());
}

fn centered_text(f: &mut Frame, area: Rect, text: &str, style: Style) {
    let y = area.y + area.height / 2;
    f.render_widget(
        Paragraph::new(Span::styled(text.to_owned(), style)).alignment(Alignment::Center),
        Rect { y, height: 1, ..area },
    );
}

/// Draws analog channel `i` into `area` (gutter and plot) on `background`.
fn analog_plot(
    f: &mut Frame,
    area: Rect,
    app: &App,
    rec: &Recording,
    i: usize,
    cursor_idx: Option<usize>,
    background: Color,
) {
    let theme = &app.theme;
    // Shade the whole row, so gutter and plot read as one band.
    f.render_widget(Block::new().style(Style::new().bg(background)), area);
    let ch = rec.data.analog(i).expect("visible channel exists");
    let color = theme.channel(&ch.info.phase, i);
    let (t0, t1) = rec.view;
    let canvas = Rect {
        x: area.x + GUTTER,
        width: area.width - GUTTER,
        ..area
    };

    // One extra sample on each side so lines reach the plot edges.
    let range = rec.data.index_range(t0, t1);
    let range = range.start.saturating_sub(1)..(range.end + 1).min(rec.data.len());
    let segments = ch.plot_segments(rec.data.time(), range, (t0, t1), canvas.width as usize * 2, app.side);
    let (lo, hi) = segments
        .iter()
        .flatten()
        .filter(|p| p.0 >= t0 && p.0 <= t1)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
    let (lo, hi) = match (lo.is_finite(), hi - lo) {
        (false, _) => (-1.0, 1.0),
        (true, d) if d < 1e-12 => (lo - 1.0, hi + 1.0),
        (true, d) => (lo - d * 0.05, hi + d * 0.05),
    };

    // Gutter: name, value at cursor, value range.
    let value = cursor_idx.and_then(|k| ch.value_on(k, app.side));
    let mut lines = vec![
        Line::styled(
            format!("{} [{}]", ch.info.id, ch.info.unit),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ),
        Line::from(format!("▸ {}", value.map_or_else(|| "-".into(), si))),
    ];
    if area.height >= 4 {
        lines.resize(area.height as usize - 2, Line::raw(""));
        lines.push(Line::styled(format!("↑ {}", si(hi)), theme.muted()));
        lines.push(Line::styled(format!("↓ {}", si(lo)), theme.muted()));
    } else {
        lines.push(Line::styled(format!("{}…{}", si(lo), si(hi)), theme.muted()));
    }
    let gutter = Rect {
        width: GUTTER - 1,
        ..area
    };
    f.render_widget(Paragraph::new(lines), gutter);

    let trigger = theme.trigger;
    let cursor = theme.cursor;
    let zero = theme.muted;
    let cursor_t = rec.cursor;
    let plot = Canvas::default()
        .marker(Marker::Braille)
        .background_color(background)
        .x_bounds([t0, t1])
        .y_bounds([lo, hi])
        .paint(move |ctx| {
            if (t0..=t1).contains(&0.0) {
                ctx.draw(&CanvasLine::new(0.0, lo, 0.0, hi, trigger));
            }
            ctx.draw(&CanvasLine::new(cursor_t, lo, cursor_t, hi, cursor));
            if lo < 0.0 && hi > 0.0 {
                ctx.draw(&CanvasLine::new(t0, 0.0, t1, 0.0, zero));
            }
            ctx.layer();
            for seg in &segments {
                if seg.len() == 1 {
                    ctx.draw(&Points { coords: seg, color });
                }
                for w in seg.windows(2) {
                    ctx.draw(&CanvasLine::new(w[0].0, w[0].1, w[1].0, w[1].1, color));
                }
            }
        });
    f.render_widget(plot, canvas);
}

fn binary_strip(f: &mut Frame, area: Rect, app: &App, rec: &Recording, i: usize, cursor_idx: Option<usize>) {
    let theme = &app.theme;
    let ch = rec.data.binary(i).expect("visible channel exists");
    let color = Style::new().fg(theme.channel("", i + 4));
    let state = cursor_idx.map(|k| ch.get(k));
    let label = format!("{:<w$.w$}", ch.info.id, w = (GUTTER - 3) as usize);
    let gutter = Line::from(vec![
        Span::styled(label, color),
        Span::styled(
            state.map_or(" -".into(), |s| format!(" {}", u8::from(s))),
            if state.is_some_and(|s| s != ch.info.normal_state) {
                theme.warning()
            } else {
                theme.base()
            },
        ),
    ]);
    f.render_widget(Paragraph::new(gutter), Rect { width: GUTTER, ..area });

    let width = area.width - GUTTER;
    let cells = ch.strip(rec.data.time(), rec.view, width as usize);
    let cursor_col = column_of(rec, rec.cursor, width);
    let normal = ch.info.normal_state;
    let buf = f.buffer_mut();
    for (c, cell) in cells.into_iter().enumerate() {
        let (symbol, abnormal) = match cell {
            Cell::Empty => (" ", false),
            Cell::Low => ("▁", normal),
            Cell::High => ("█", !normal),
            Cell::Mixed => ("▌", true),
        };
        let mut style = if abnormal { theme.warning() } else { theme.muted() };
        if Some(c as u16) == cursor_col {
            style = style.add_modifier(Modifier::REVERSED);
        }
        let x = area.x + GUTTER + c as u16;
        buf[(x, area.y)].set_symbol(symbol).set_style(style);
    }
}

/// Column of time `t` within a plot `width` cells wide, if visible.
fn column_of(rec: &Recording, t: f64, width: u16) -> Option<u16> {
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
fn cursor_label(app: &App, rec: &Recording) -> String {
    if app.absolute_time {
        time_label(app, rec, rec.cursor, 0.0)
    } else {
        format!("{:.d$} ms", rec.cursor * 1e3, d = ms_decimals(rec))
    }
}

/// Bottom row: tick labels, the trigger marker `T` and the cursor marker
/// with its time. The cursor label is always drawn; tick labels that would
/// overlap it are skipped.
fn time_axis(f: &mut Frame, area: Rect, app: &App, rec: &Recording) {
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
