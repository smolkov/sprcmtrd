//! Plot area: analog line plots, binary status strips and a shared time axis.
//!
//! When the shown channels don't fit with their minimum heights, the channel
//! rows scroll; the time axis on the bottom row and the block title stay.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine, Points};
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use super::time_axis::{self, column_of, cursor_label};
use crate::analysis::harmonics;
use crate::analysis::rms::{self, Dc};
use crate::comtrade::{Cell, decimated_segments};
use crate::format::{si, time};
use crate::tui::app::{App, Focus, PlotView};
use crate::tui::keys::Action;
use crate::tui::recording::Recording;

/// Width of the label column left of each plot.
const GUTTER: u16 = 16;
/// Minimum height of an analog plot; below that the plot area scrolls.
const MIN_ANALOG_HEIGHT: u16 = 5;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Plot && app.recording.is_some();
    let name = match (app.plot_view, app.rms_dc) {
        (PlotView::Waveform, _) => "Plot",
        (PlotView::Rms, Dc::Keep) => "RMS",
        (PlotView::Rms, Dc::Remove) => "RMS-DC",
        (PlotView::Harmonics, _) => "Harmonics",
    };
    // Built late: the title shows the scroll position once it is known.
    let block = |title: &str| {
        Block::bordered()
            .title(format!(" {name}{title} "))
            .title_style(if focused { theme.accent() } else { theme.base() })
            .border_style(theme.border(focused))
    };

    let Some(rec) = &app.recording else {
        let block = block("");
        let inner = block.inner(area);
        f.render_widget(block, area);
        empty_message(f, inner, app);
        return;
    };

    let (t0, t1) = rec.view;
    let span = t1 - t0;
    let block = |title: &str| {
        block(title).title(
            Line::from(format!(
                " cursor {} · window {} ",
                cursor_label(app, rec),
                time(span, span)
            ))
            .alignment(Alignment::Right),
        )
    };
    let inner = Block::bordered().inner(area);
    if inner.width <= GUTTER + 4 || inner.height < 2 {
        f.render_widget(block(""), area);
        return;
    }

    // Analog rows: a channel, and in the harmonics view one row per shown order.
    let orders = match app.plot_view {
        PlotView::Harmonics => app.shown_orders().into_iter().map(Some).collect(),
        _ => vec![None],
    };
    let analog: Vec<(usize, Option<usize>)> = (0..rec.visible_analog.len())
        .filter(|&i| rec.visible_analog[i])
        .flat_map(|i| orders.iter().map(move |&h| (i, h)))
        .collect();
    let binary: Vec<usize> = (0..rec.visible_binary.len())
        .filter(|&i| rec.visible_binary[i])
        .collect();
    if analog.is_empty() && binary.is_empty() {
        f.render_widget(block(""), area);
        let msg = if orders.is_empty() && rec.visible_analog.iter().any(|&v| v) {
            "No harmonic orders shown. Press 1 … 9 to show an order.".to_owned()
        } else {
            let key = app.keymap.label(Action::ToggleChannel);
            format!("No channels shown. Select channels in the sidebar and press {key}.")
        };
        centered_text(f, inner, &msg, theme.muted());
        return;
    }

    // The bottom row is the time axis; the channel rows above it scroll.
    let rows = inner.height - 1;
    let keep = app
        .plot_follow
        .get()
        .then(|| shown_position(rec, &analog, app.selected))
        .flatten();
    let layout = layout(rows, analog.len(), binary.len(), app.plot_scroll.get(), keep);
    app.plot_scroll.set(layout.offset);
    app.plot_page.set(layout.rows.len());
    let n = analog.len() + binary.len();
    let position = if layout.scrolls {
        let last = layout.offset + layout.rows.len();
        format!(" · channels {}–{last} of {n}", layout.offset + 1)
    } else {
        String::new()
    };
    f.render_widget(block(&position), area);

    let cursor_idx = rec.data.index_at(rec.cursor);
    for (k, &(y, height)) in layout.rows.iter().enumerate() {
        let row = Rect {
            x: inner.x,
            y: inner.y + y,
            width: inner.width,
            height,
        };
        let item = layout.offset + k;
        if let Some(&(i, order)) = analog.get(item) {
            // Shade by row position, so shades don't swap while scrolling.
            analog_plot(f, row, app, rec, (i, order), cursor_idx, theme.plot_background(item));
        } else {
            binary_strip(f, row, app, rec, binary[item - analog.len()], cursor_idx);
        }
    }

    if layout.scrolls {
        // On the right border, next to the channel rows.
        let track = Rect {
            x: area.x + area.width - 1,
            y: inner.y,
            width: 1,
            height: rows,
        };
        let mut state = ScrollbarState::new(n - layout.rows.len() + 1)
            .position(layout.offset)
            .viewport_content_length(layout.rows.len());
        let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some(ratatui::symbols::line::VERTICAL))
            .track_style(theme.border(focused))
            .thumb_style(theme.accent());
        f.render_stateful_widget(bar, track, &mut state);
    }

    let axis = Rect {
        x: inner.x + GUTTER,
        y: inner.y + rows,
        width: inner.width - GUTTER,
        height: 1,
    };
    time_axis::render(f, axis, app, rec);
}

/// Row of channel `selected` (analog first, then binary) among the shown
/// rows (`analog` rows, then the shown binary channels), if it is shown: its
/// first row in the harmonics view.
fn shown_position(rec: &Recording, analog: &[(usize, Option<usize>)], selected: usize) -> Option<usize> {
    let na = rec.visible_analog.len();
    if selected < na {
        return analog.iter().position(|&(i, _)| i == selected);
    }
    let b = selected - na;
    rec.visible_binary
        .get(b)
        .copied()
        .unwrap_or(false)
        .then(|| analog.len() + rec.visible_binary[..b].iter().filter(|&&v| v).count())
}

/// Rows of the shown channels in the scrolled part of the plot area.
#[derive(Debug, PartialEq)]
struct PlotLayout {
    /// First drawn channel (index into the shown channels, analog first).
    offset: usize,
    /// Top row (relative to the area) and height of each drawn channel from `offset` on.
    rows: Vec<(u16, u16)>,
    /// Not all channels fit, so the area scrolls.
    scrolls: bool,
}

/// Lays out `n_analog` analog plots and `n_binary` binary strips in `height`
/// rows. If all fit with their minimum height, analog plots share the rows the
/// strips leave. Otherwise they get the minimum height and only the channels
/// from `offset` on are drawn; `offset` is clamped so the last page is full,
/// and moved so that channel `keep` is drawn.
fn layout(height: u16, n_analog: usize, n_binary: usize, offset: usize, keep: Option<usize>) -> PlotLayout {
    let n = n_analog + n_binary;
    if n_analog * MIN_ANALOG_HEIGHT as usize + n_binary <= height as usize {
        let analog_rows = height - n_binary as u16;
        let mut rows = Vec::with_capacity(n);
        let mut y = 0;
        for k in 0..n_analog {
            let base = analog_rows / n_analog as u16;
            let h = if k + 1 == n_analog {
                analog_rows - base * (n_analog as u16 - 1)
            } else {
                base
            };
            rows.push((y, h));
            y += h;
        }
        rows.extend((0..n_binary).map(|k| (y + k as u16, 1)));
        return PlotLayout {
            offset: 0,
            rows,
            scrolls: false,
        };
    }

    let analog_height = MIN_ANALOG_HEIGHT.min(height);
    let h = |i: usize| if i < n_analog { analog_height } else { 1 };
    // Largest offset: the channels from there to the end still fill the area.
    let mut max_offset = n;
    let mut used = 0;
    while max_offset > 0 && used + h(max_offset - 1) <= height {
        max_offset -= 1;
        used += h(max_offset);
    }
    let mut offset = offset.min(max_offset);
    if let Some(k) = keep.filter(|&k| k < n) {
        if k < offset {
            offset = k;
        } else {
            // Smallest offset from the current one on that still draws `k`.
            let (mut first, mut used) = (k, h(k));
            while first > offset && used + h(first - 1) <= height {
                first -= 1;
                used += h(first);
            }
            offset = first;
        }
    }

    let mut heights = Vec::new();
    let mut used = 0;
    for i in offset..n {
        if used + h(i) > height {
            break;
        }
        heights.push(h(i));
        used += h(i);
    }
    // Rows too few for the next channel go to the drawn analog plots.
    let drawn_analog = n_analog.saturating_sub(offset).min(heights.len()) as u16;
    let spare = height - used;
    for (k, h) in (0..drawn_analog).zip(&mut heights) {
        *h += spare / drawn_analog + u16::from(k < spare % drawn_analog);
    }
    let mut y = 0;
    let rows = heights
        .into_iter()
        .map(|h| {
            y += h;
            (y - h, h)
        })
        .collect();
    PlotLayout {
        offset,
        rows,
        scrolls: true,
    }
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

/// Draws analog channel `i` into `area` (gutter and plot) on `background`;
/// with `order`, that harmonic order over time.
fn analog_plot(
    f: &mut Frame,
    area: Rect,
    app: &App,
    rec: &Recording,
    (i, order): (usize, Option<usize>),
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
    let columns = canvas.width as usize * 2;
    let time = rec.data.time();
    let is_rms = app.plot_view == PlotView::Rms;
    let rms_of = |range| rms::sliding(ch, time, range, |k| rec.rms_seconds(k), app.rms_dc, app.side);
    // Order 1 as RMS, higher orders in % of the fundamental at that moment.
    let harmonic_at = |points: &[usize], h: usize| -> Vec<Option<f64>> {
        let ords: &[usize] = if h == 1 { &[1] } else { &[1, h] };
        harmonics::sliding(ch, time, points, |k| rec.rms_seconds(k), ords, app.side)
            .into_iter()
            .map(|v| match (h, v[0], v.get(1).copied().flatten()) {
                (1, x, _) => x,
                (_, Some(f), Some(x)) if f > 0.0 => Some(x / f * 100.0),
                _ => None,
            })
            .collect()
    };
    let segments = match order {
        Some(h) => {
            // A DFT per point: only the points drawn, one per dot column.
            let points = plot_points(range, columns);
            let values = harmonic_at(&points, h);
            point_segments(time, &points, &values)
        }
        None if is_rms => {
            let series = rms_of(range.clone());
            decimated_segments(time, range.clone(), (t0, t1), columns, |k| series[k - range.start])
        }
        None => ch.plot_segments(time, range, (t0, t1), columns, app.side),
    };
    let from_zero = is_rms || order.is_some();
    let (lo, hi) = segments
        .iter()
        .flatten()
        .filter(|p| p.0 >= t0 && p.0 <= t1)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
    // From 0 in the RMS view, so a steady value doesn't look like a large swing.
    let (lo, hi) = match (lo.is_finite(), hi - lo) {
        _ if from_zero => (0.0, if hi > 1e-12 { hi * 1.05 } else { 1.0 }),
        (false, _) => (-1.0, 1.0),
        (true, d) if d < 1e-12 => (lo - 1.0, hi + 1.0),
        (true, d) => (lo - d * 0.05, hi + d * 0.05),
    };

    // Gutter: name, value at cursor, value range.
    let value = cursor_idx.and_then(|k| match order {
        Some(h) => harmonic_at(&[k], h).first().copied().flatten(),
        None if is_rms => rms_of(k..k + 1).first().copied().flatten(),
        None => ch.value_on(k, app.side),
    });
    let name = match order {
        Some(1) => format!("{} h1 [{}]", ch.info.id, ch.info.unit),
        Some(h) => format!("{} h{h} [%]", ch.info.id),
        None => format!("{} [{}]", ch.info.id, ch.info.unit),
    };
    let mut lines = vec![
        Line::styled(name, Style::new().fg(color).add_modifier(Modifier::BOLD)),
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

/// Samples of `range` to compute and draw: all of them when they fit in one
/// per braille dot column (`columns`, two per character), else one per dot
/// column, evenly spread.
fn plot_points(range: std::ops::Range<usize>, columns: usize) -> Vec<usize> {
    let n = range.len();
    let wanted = columns.max(2);
    if n <= wanted {
        return range.collect();
    }
    (0..wanted).map(|k| range.start + k * (n - 1) / (wanted - 1)).collect()
}

/// Line segments of `values` at the samples `points`; a missing value ends a
/// segment.
fn point_segments(time: &[f64], points: &[usize], values: &[Option<f64>]) -> Vec<Vec<(f64, f64)>> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    for (&k, v) in points.iter().zip(values) {
        match v {
            Some(v) => current.push((time[k], *v)),
            None if !current.is_empty() => segments.push(std::mem::take(&mut current)),
            None => {}
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
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

#[cfg(test)]
mod tests {
    use super::*;

    const M: u16 = MIN_ANALOG_HEIGHT;

    fn heights(l: &PlotLayout) -> Vec<u16> {
        l.rows.iter().map(|r| r.1).collect()
    }

    #[test]
    fn everything_fits_and_analog_plots_share_the_rows() {
        let l = layout(20, 3, 2, 7, Some(4));
        assert!(!l.scrolls);
        assert_eq!(l.offset, 0);
        // 18 rows for 3 analog plots, the last one takes the remainder.
        assert_eq!(heights(&l), [6, 6, 6, 1, 1]);
        assert_eq!(l.rows[3], (18, 1));

        let l = layout(10, 0, 3, 0, None);
        assert_eq!(l.rows, [(0, 1), (1, 1), (2, 1)]);
        assert_eq!(heights(&layout(M * 2, 2, 0, 0, None)), [M, M]);
    }

    #[test]
    fn overflow_keeps_minimum_height_and_scrolls() {
        // 4 analog (4 × M) and 4 binary need more than 19 rows.
        let l = layout(19, 4, 4, 0, None);
        assert!(l.scrolls);
        assert_eq!(l.offset, 0);
        // The 4 rows left over go to the drawn plots.
        assert_eq!(heights(&l), [M + 2, M + 1, M + 1]);
        assert_eq!(l.rows[2], (2 * M + 3, M + 1));

        // Offset 1: analog 1..4, then binary strips while rows are left.
        let l = layout(19, 4, 4, 1, None);
        assert_eq!(heights(&l), [M, M, M, 1, 1, 1, 1]);
    }

    #[test]
    fn offset_is_clamped_to_a_full_last_page() {
        let l = layout(19, 4, 4, 100, None);
        assert_eq!(l.offset, 1);
        assert_eq!(l.rows.len(), 7);
        // Only binary strips: last page shows the last `height` strips.
        let l = layout(5, 0, 12, 100, None);
        assert_eq!((l.offset, l.rows.len()), (7, 5));
    }

    #[test]
    fn keep_scrolls_a_channel_into_view() {
        // Below the page: the kept channel becomes the last drawn one.
        let l = layout(12, 6, 0, 0, Some(4));
        assert_eq!(l.offset, 3);
        assert_eq!(l.offset + l.rows.len() - 1, 4);
        // Above the page: it becomes the first.
        assert_eq!(layout(12, 6, 0, 4, Some(1)).offset, 1);
        // Already drawn: the offset stays.
        assert_eq!(layout(12, 6, 0, 2, Some(3)).offset, 2);
    }

    #[test]
    fn analog_plots_shrink_to_tiny_areas() {
        let l = layout(2, 3, 0, 0, None);
        assert!(l.scrolls);
        assert_eq!(heights(&l), [2]);
    }
}
