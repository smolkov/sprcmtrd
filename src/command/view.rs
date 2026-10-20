//! `view` command: summary and plots printed to stdout.

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::bail;
use textplots::{Chart, LabelBuilder, LabelFormat, Plot, Shape};

use super::info::{key_values, summary};
use super::{Context, si, time};
use crate::comtrade::{Cell, Comtrade, Side};

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
    let (analog, binary) = select(&c, &opts.channels)?;

    let (first, last) = c.time_span();
    let t0 = opts.from_ms.map_or(first, |ms| ms / 1e3);
    let t1 = opts.to_ms.map_or(last, |ms| ms / 1e3);
    if t1 <= t0 {
        bail!("empty time window: --from must be before --to");
    }
    let width = opts.width.map(usize::from).unwrap_or_else(terminal_width);
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

/// Resolves the channel selection into analog and binary positions.
fn select(c: &Comtrade, tokens: &[String]) -> anyhow::Result<(Vec<usize>, Vec<usize>)> {
    let n_analog = c.analog_channels().len();
    let n_binary = c.binary_channels().len();
    if tokens.is_empty() {
        return Ok(((0..n_analog).collect(), (0..n_binary).collect()));
    }
    let (mut analog, mut binary) = (Vec::new(), Vec::new());
    for token in tokens.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let digital_index = token.strip_prefix(['d', 'D']).and_then(|n| n.parse::<usize>().ok());
        if let Ok(n) = token.parse::<usize>() {
            if n == 0 || n > n_analog {
                bail!("analog channel {n} does not exist (1..={n_analog})");
            }
            analog.push(n - 1);
        } else if let Some(n) = digital_index {
            if n == 0 || n > n_binary {
                bail!("binary channel d{n} does not exist (d1..=d{n_binary})");
            }
            binary.push(n - 1);
        } else if let Some(i) = c.analog_channels().position(|a| a.info.id.eq_ignore_ascii_case(token)) {
            analog.push(i);
        } else if let Some(i) = c.binary_channels().position(|b| b.info.id.eq_ignore_ascii_case(token)) {
            binary.push(i);
        } else {
            bail!("unknown channel '{token}'");
        }
    }
    Ok((analog, binary))
}

fn terminal_width() -> usize {
    if std::io::stdout().is_terminal()
        && let Ok((w, _)) = crossterm::terminal::size()
    {
        return w as usize;
    }
    100
}
