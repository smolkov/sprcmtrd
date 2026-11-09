//! Braille line charts of sample segments (`view`, `compare --plot`).

use textplots::{Chart, LabelBuilder, LabelFormat, Plot, Shape};

use crate::format::{si, time};

/// Width reserved right of the plot for textplots' y labels.
pub const LABEL_WIDTH: usize = 12;

/// Smallest and largest value of the segments (infinite without points).
pub fn y_range<'a>(segments: impl IntoIterator<Item = &'a Vec<(f64, f64)>>) -> (f64, f64) {
    segments
        .into_iter()
        .flatten()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p.1), hi.max(p.1))
        })
}

/// Renders line segments with textplots, on the y range `y` (default: that
/// of the segments).
pub fn plot(
    segments: &[Vec<(f64, f64)>],
    (t0, t1): (f64, f64),
    y: Option<(f64, f64)>,
    cols: usize,
    rows: usize,
) -> String {
    if segments.iter().all(Vec::is_empty) {
        return "  (no samples in window)\n".into();
    }
    let (lo, hi) = y.unwrap_or_else(|| y_range(segments));
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
