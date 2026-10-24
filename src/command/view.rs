//! `view` command: summary and plots printed to stdout.

use std::path::PathBuf;

use textplots::{Chart, LabelBuilder, LabelFormat, Plot, Shape};

use super::info::{key_values, summary};
use super::{Context, select_channels, si, time, time_window};
use crate::comtrade::{Cell, Comtrade, Side};
use crate::term;

/// Options of the `view` command.
#[derive(Debug, Clone)]
pub struct Options {
    /// Any file of the recording.
    pub file: PathBuf,
    /// Channel selection (indices or ids).
    pub channels: Vec<String>,
    /// Window start in ms relative to the trigger.
    pub from_ms: Option<f64>,
    /// Window end in ms relative to the trigger.
    pub to_ms: Option<f64>,
    /// Output width in columns.
    pub width: Option<u16>,
    /// Height of each analog plot in rows.
    pub height: u16,
}

/// Width reserved right of the plot for textplots' y labels.
const LABEL_WIDTH: usize = 12;

/// Runs the `view` command.
pub fn run(ctx: &Context, opts: &Options) -> anyhow::Result<()> {
    let c = Comtrade::open(&opts.file)?;
    let side = if ctx.global.secondary {
        Side::Secondary
    } else {
        Side::Primary
    };
    let (analog, binary) = select_channels(&c, &opts.channels)?;
    let (t0, t1) = time_window(&c, opts.from_ms, opts.to_ms)?;
    let width = opts.width.map(usize::from).or_else(term::terminal_width).unwrap_or(100);
    let plot_cols = width.saturating_sub(LABEL_WIDTH).max(16);
    let range = c.index_range(t0, t1);

    let name = c
        .path
        .as_ref()
        .map(|p| p.main_file().display().to_string())
        .unwrap_or_default();
    let mut out = key_values(&summary(&name, &c.config));
    out += &key_values(&[
        ("Window", format!("{} .. {}", time(t0, t1 - t0), time(t1, t1 - t0))),
        ("Values", format!("{side:?}").to_lowercase()),
    ]);

    for i in analog {
        let ch = c.analog(i).expect("selected index is valid");
        out += &format!("\nA{} {} [{}]\n", ch.info.index, ch.info.id, ch.info.unit);
        let segments = ch.plot_segments(c.time(), range.clone(), (t0, t1), plot_cols * 2, side);
        out += &plot(&segments, (t0, t1), plot_cols, opts.height.max(1) as usize);
    }

    if !binary.is_empty() {
        out += "\n";
    }
    for i in binary {
        let ch = c.binary(i).expect("selected index is valid");
        let strip: String = ch
            .strip(c.time(), (t0, t1), plot_cols)
            .into_iter()
            .map(|cell| match cell {
                Cell::Empty => ' ',
                Cell::Low => '▁',
                Cell::High => '█',
                Cell::Mixed => '▌',
            })
            .collect();
        out += &format!("{strip} D{} {}\n", ch.info.index, ch.info.id);
    }
    print!("{out}");
    Ok(())
}

/// Renders line segments with textplots.
fn plot(segments: &[Vec<(f64, f64)>], (t0, t1): (f64, f64), cols: usize, rows: usize) -> String {
    let (lo, hi) = segments
        .iter()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        });
    if !lo.is_finite() {
        return "  (no samples in window)\n".into();
    }
    let (lo, hi) = if hi - lo < 1e-12 {
        (lo - 1.0, hi + 1.0)
    } else {
        (lo, hi)
    };

    let points: Vec<Vec<(f32, f32)>> = segments
        .iter()
        .map(|s| s.iter().map(|&(t, v)| ((t * 1e3) as f32, v as f32)).collect())
        .collect();
    let shapes: Vec<Shape> = points
        .iter()
        .map(|p| {
            if p.len() == 1 {
                Shape::Points(p)
            } else {
                Shape::Lines(p)
            }
        })
        .collect();

    let span = t1 - t0;
    let mut chart = Chart::new_with_y_range(
        (cols * 2) as u32,
        (rows * 4).max(3) as u32,
        (t0 * 1e3) as f32,
        (t1 * 1e3) as f32,
        lo as f32,
        hi as f32,
    );
    let mut c = chart
        .x_label_format(LabelFormat::Custom(Box::new(move |ms| time(ms as f64 / 1e3, span))))
        .y_label_format(LabelFormat::Custom(Box::new(|v| si(v as f64))));
    for s in &shapes {
        c = c.lineplot(s);
    }
    c.axis();
    c.figures();
    // textplots pads with U+2800 (blank braille); plain spaces copy better.
    c.to_string().replace('\u{2800}', " ")
}
