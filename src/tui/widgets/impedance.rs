//! Impedance view: the fault loop impedances as trajectories in the R-X
//! plane over the visible time window, phase-to-ground loops (L-N) and
//! phase-to-phase loops (L-L) in two diagrams, with the values at the cursor.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::widgets::canvas::{Canvas, Line as CanvasLine};

use super::plot::plot_points;
use crate::analysis::Complex;
use crate::analysis::impedance::{self, Loop, LoopPoint};
use crate::analysis::phasor::Window;
use crate::format::si;
use crate::tui::app::App;
use crate::tui::recording::Recording;

/// Plot area width from which the two diagrams stand side by side.
const SIDE_BY_SIDE: u16 = 100;

/// A loop point is drawn only with at least this part of the loop's largest
/// current in the window: without current the impedance runs to infinity.
const MIN_CURRENT: f64 = 0.05;

/// Rows of a diagram's table: header and three loops.
const TABLE_ROWS: u16 = 4;

/// Draws both diagrams into `area`.
pub fn render(f: &mut Frame, area: Rect, app: &App, rec: &Recording) {
    let theme = &app.theme;
    let (Some(loops), Some(frequency)) = (rec.loops(), rec.line_frequency()) else {
        let msg = Paragraph::new(Span::styled(
            "No voltage and current A/B/C groups to measure the loops with.",
            theme.muted(),
        ));
        f.render_widget(msg, area);
        return;
    };
    let halves = if area.width >= SIDE_BY_SIDE {
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area)
    } else {
        Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area)
    };

    // One cycle at the line frequency, as `impedance --trend` measures.
    let data = &rec.data;
    let samples = data
        .len()
        .checked_sub(1)
        .and_then(|last| Window::ending_at(data, data.time()[last], 1.0, frequency).ok())
        .map_or(0, |w| w.samples);
    // Points for the wider of the two diagrams: two per column.
    let columns = halves[0].width.max(halves[1].width) as usize * 2;
    let range = data.index_range(rec.view.0, rec.view.1);
    let points = plot_points(range, columns);
    let k0 = Complex::default();
    let values = impedance::trajectory(&loops, data.time(), &points, samples, frequency, k0, app.side);
    let cursor = data
        .index_at(rec.cursor)
        .map(|k| impedance::trajectory(&loops, data.time(), &[k], samples, frequency, k0, app.side)[0]);

    let ground = [Loop::AN, Loop::BN, Loop::CN];
    let phases = [Loop::AB, Loop::BC, Loop::CA];
    for (half, (title, group)) in halves.iter().zip([("L-N (k0 = 0)", ground), ("L-L", phases)]) {
        diagram(f, *half, app, title, &group, &values, cursor);
    }
}

/// Position of `l` in [`Loop::ALL`].
fn index(l: Loop) -> usize {
    Loop::ALL.iter().position(|&x| x == l).expect("every loop is in ALL")
}

/// One R-X diagram of the three `group` loops, its title and its table.
fn diagram(
    f: &mut Frame,
    area: Rect,
    app: &App,
    title: &str,
    group: &[Loop; 3],
    values: &[[Option<LoopPoint>; 6]],
    cursor: Option<[Option<LoopPoint>; 6]>,
) {
    let theme = &app.theme;
    if area.height < TABLE_ROWS + 3 || area.width < 20 {
        return;
    }
    // Per loop the points with enough current, in time order; a gap ends a
    // line. The largest current is the loop's within the window.
    let largest = |l: Loop| {
        values
            .iter()
            .filter_map(|v| v[index(l)].map(|p| p.current.abs()))
            .fold(0.0, f64::max)
    };
    let limits: Vec<f64> = group.iter().map(|&l| MIN_CURRENT * largest(l)).collect();
    let kept = |limit: f64, p: Option<LoopPoint>| p.filter(|p| p.current.abs() >= limit && limit > 0.0).map(|p| p.z);
    let traces: Vec<Vec<Option<Complex>>> = group
        .iter()
        .zip(&limits)
        .map(|(&l, &limit)| values.iter().map(|v| kept(limit, v[index(l)])).collect())
        .collect();
    let at_cursor: Vec<Option<Complex>> = group
        .iter()
        .zip(&limits)
        .map(|(&l, &limit)| cursor.and_then(|c| kept(limit, c[index(l)])))
        .collect();

    // The range covers the points and the origin, the same Ω per dot on
    // both axes (a braille dot is about square), with 5 % margin.
    let canvas = Rect {
        y: area.y + 1,
        height: area.height - 1 - TABLE_ROWS,
        ..area
    };
    let all = traces.iter().flatten().flatten().chain(at_cursor.iter().flatten());
    let (mut r0, mut r1, mut x0, mut x1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for z in all {
        r0 = r0.min(z.re);
        r1 = r1.max(z.re);
        x0 = x0.min(z.im);
        x1 = x1.max(z.im);
    }
    let (dots_w, dots_h) = (canvas.width as f64 * 2.0, canvas.height as f64 * 4.0);
    let per_dot = ((r1 - r0) / dots_w).max((x1 - x0) / dots_h).max(1e-9) * 1.05;
    let (rc, xc) = ((r0 + r1) / 2.0, (x0 + x1) / 2.0);
    let (r0, r1) = (rc - per_dot * dots_w / 2.0, rc + per_dot * dots_w / 2.0);
    let (x0, x1) = (xc - per_dot * dots_h / 2.0, xc + per_dot * dots_h / 2.0);

    let head = Line::from(vec![
        Span::styled(title.to_owned(), theme.accent()),
        Span::styled(
            format!("  R {} … {} Ω · X {} … {} Ω", si(r0), si(r1), si(x0), si(x1)),
            theme.muted(),
        ),
    ]);
    f.render_widget(Paragraph::new(head), Rect { height: 1, ..area });

    let axes = theme.muted;
    let colors: Vec<Color> = group.iter().map(|l| theme.channel(&l.name()[..1], index(*l))).collect();
    let names: Vec<&'static str> = group.iter().map(|l| short(*l)).collect();
    let plot = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([r0, r1])
        .y_bounds([x0, x1])
        .paint(|ctx| {
            ctx.draw(&CanvasLine::new(r0, 0.0, r1, 0.0, axes));
            ctx.draw(&CanvasLine::new(0.0, x0, 0.0, x1, axes));
            ctx.layer();
            for (trace, &color) in traces.iter().zip(&colors) {
                for w in trace.windows(2) {
                    if let [Some(a), Some(b)] = w {
                        ctx.draw(&CanvasLine::new(a.re, a.im, b.re, b.im, color));
                    }
                }
            }
            // The names at the end of each trajectory and at the cursor, so
            // the loops read without color.
            for ((trace, &color), name) in traces.iter().zip(&colors).zip(&names) {
                if let Some(end) = trace.iter().rev().flatten().next() {
                    ctx.print(
                        end.re,
                        end.im,
                        Span::styled(name.to_lowercase(), Style::new().fg(color)),
                    );
                }
            }
            for ((z, &color), name) in at_cursor.iter().zip(&colors).zip(&names) {
                if let Some(z) = z {
                    let style = Style::new().fg(color).add_modifier(Modifier::BOLD | Modifier::REVERSED);
                    ctx.print(z.re, z.im, Span::styled(*name, style));
                }
            }
        });
    f.render_widget(plot, canvas);

    let table = Rect {
        y: area.y + area.height - TABLE_ROWS,
        height: TABLE_ROWS,
        ..area
    };
    let mut lines = vec![Line::styled(
        format!(
            "{:<5} {:>9} {:>9} {:>9} {:>8}",
            "loop", "R [Ω]", "X [Ω]", "|Z| [Ω]", "φ"
        ),
        theme.muted(),
    )];
    for ((l, z), &color) in group.iter().zip(&at_cursor).zip(&colors) {
        let cells = match z {
            Some(z) => format!(
                "{:>9} {:>9} {:>9} {:>7.1}°",
                si(z.re),
                si(z.im),
                si(z.abs()),
                z.arg().to_degrees()
            ),
            None => format!("{:>9} {:>9} {:>9} {:>8}", "-", "-", "-", "-"),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<5} ", l.name()), Style::new().fg(color)),
            Span::raw(cells),
        ]));
    }
    f.render_widget(Paragraph::new(lines), table);
}

/// The label of a loop in a diagram: `A` for A-N, `AB` for A-B.
fn short(l: Loop) -> &'static str {
    match l {
        Loop::AN => "A",
        Loop::BN => "B",
        Loop::CN => "C",
        Loop::AB => "AB",
        Loop::BC => "BC",
        Loop::CA => "CA",
    }
}
