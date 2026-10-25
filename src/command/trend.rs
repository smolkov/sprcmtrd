//! `trend` command: values over time, cycle by cycle.

use anyhow::{anyhow, bail};
use serde::Serialize;

use super::{select_channels, time_window};
use crate::analysis::phasor::{Polar, Window, WindowError};
use crate::analysis::sequence::Group;
use crate::analysis::trend::{self, Step};
use crate::args::{TableFormat, TrendArgs, TrendStep};
use crate::comtrade::{AnalogChannel, Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, rate, si, table, time};

impl TrendArgs {
    /// Runs the `trend` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        };
        let (analog, binary) = select_channels(&c, &self.channels)?;
        if let Some(&i) = binary.first()
            && !self.channels.is_empty()
        {
            let ch = c.binary(i).expect("selected index is valid");
            bail!("{} is a binary channel; trend needs analog channels", ch.info.id);
        }
        if analog.is_empty() {
            bail!("the recording has no analog channels");
        }
        let channels: Vec<&AnalogChannel> = analog
            .iter()
            .map(|&i| c.analog(i).expect("selected index is valid"))
            .collect();
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }

        // Window length from the sample spacing at the end of the recording.
        let (first, last) = c.time_span();
        let window = Window::ending_at(&c, last, self.cycles, frequency).map_err(|e| match e {
            WindowError::TooShort(n) => anyhow!(
                "{} cycles at {frequency} Hz are only {n} samples; use more --cycles",
                self.cycles
            ),
            WindowError::BeforeStart { samples, .. } => {
                anyhow!("the recording is shorter than one window of {samples} samples")
            }
            e => anyhow!(e),
        })?;
        let samples = window.samples;
        let dt = self.cycles / frequency / samples as f64;
        let step = match self.step {
            TrendStep::Cycles(n) => n / frequency,
            TrendStep::Seconds(s) => s,
        };
        let step_samples = ((step / dt).round() as usize).max(1);

        let reference = match &self.reference {
            None => 0,
            Some(r) => {
                let (a, _) = select_channels(&c, std::slice::from_ref(r))?;
                let Some(pos) = a.first().and_then(|i| analog.iter().position(|j| j == i)) else {
                    bail!("the reference {r} must be one of the shown channels");
                };
                pos
            }
        };
        let (t0, t1) = time_window(&c, self.from, self.to)?;
        let range = c.index_range(t0, t1);
        let result = trend::trend(
            &channels,
            c.time(),
            range,
            samples,
            step_samples,
            frequency,
            reference,
            side,
        );
        if result.steps.is_empty() {
            bail!(
                "no window of {samples} samples fits in the time range; the recording runs from {} to {}",
                time(first, last - first),
                time(last, last - first)
            );
        }

        let table = Table {
            channels: &channels,
            groups: &result.groups,
            labels: result
                .groups
                .iter()
                .enumerate()
                .map(|(n, g)| label(&channels, g, n))
                .collect(),
            angles: self.angles,
            rms: self.rms,
        };
        match self.format {
            TableFormat::Csv => print!("{}", table.csv(&result.steps, reference)),
            TableFormat::Json => {
                let json = table.json(&c, &result.steps, reference, frequency, samples, step_samples, side);
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            TableFormat::Text => {
                let name = c
                    .path
                    .as_ref()
                    .map(|p| p.main_file().display().to_string())
                    .unwrap_or_default();
                let ch = channels[reference];
                let cycles = |n: f64| format!("{n} {}", if n == 1.0 { "cycle" } else { "cycles" });
                let step_text = match self.step {
                    TrendStep::Cycles(n) => cycles(n),
                    TrendStep::Seconds(s) => time(s, s),
                };
                print!(
                    "{}",
                    key_values(&[
                        ("File", name),
                        (
                            "Window",
                            format!(
                                "{} ({samples} samples) at {}, every {step_text} ({} rows)",
                                cycles(self.cycles),
                                rate(frequency),
                                result.steps.len()
                            ),
                        ),
                        ("Reference", format!("A{} {}", ch.info.index, ch.info.id)),
                        (
                            "Values",
                            format!(
                                "{}, RMS of the fundamental",
                                if side == Side::Secondary {
                                    "secondary"
                                } else {
                                    "primary"
                                }
                            ),
                        ),
                    ])
                );
                println!("\n{}", table.text(&result.steps, reference).trim_end());
            }
        }
        Ok(())
    }
}

/// Name of a group's sequence columns: the common start of its channel ids
/// (`I` for IA, IB, IC), else `G1`, `G2`, …
fn label(channels: &[&AnalogChannel], g: &Group, n: usize) -> String {
    let ids = g.phases.map(|i| channels[i].info.id.as_str());
    let common: String = ids[0]
        .chars()
        .zip(ids[1].chars())
        .zip(ids[2].chars())
        .take_while(|((a, b), c)| a == b && b == c)
        .map(|((a, _), _)| a)
        .collect();
    let common = common.trim_end_matches([' ', '_', '-']).to_owned();
    if common.is_empty() {
        format!("G{}", n + 1)
    } else {
        common
    }
}

/// What the rows show.
struct Table<'a> {
    channels: &'a [&'a AnalogChannel],
    groups: &'a [Group],
    labels: Vec<String>,
    angles: bool,
    rms: bool,
}

/// One value of a row: the number, or why it is missing.
enum Cell {
    Value(f64),
    Angle(f64),
    Percent(f64),
    Frequency(f64),
    Missing,
}

impl Table<'_> {
    /// Column names as `(text header, CSV header)`.
    fn headers(&self) -> Vec<(String, String)> {
        let mut h = vec![
            ("time".to_owned(), "time_ms".to_owned()),
            ("f [Hz]".to_owned(), "frequency_hz".to_owned()),
        ];
        for ch in self.channels {
            let (id, unit) = (&ch.info.id, &ch.info.unit);
            h.push((format!("{id} [{unit}]"), csv_name(&format!("{id}_{unit}"))));
            if self.angles {
                h.push((format!("{id} ∠"), csv_name(&format!("{id}_deg"))));
            }
            if self.rms {
                h.push((format!("{id} rms"), csv_name(&format!("{id}_rms_{unit}"))));
            }
        }
        for (g, label) in self.groups.iter().zip(&self.labels) {
            for n in ["1", "2", "0"] {
                h.push((
                    format!("{label}{n} [{}]", g.unit),
                    csv_name(&format!("{label}{n}_{}", g.unit)),
                ));
            }
            h.push((format!("{label}2/1"), csv_name(&format!("{label}_unbalance_pct"))));
        }
        h
    }

    /// The cells of `step`, angles relative to the reference channel.
    fn cells(&self, step: &Step, reference: usize) -> Vec<Cell> {
        let angle0 = step.phasors[reference].map_or(0.0, |p| p.arg());
        let mut cells = vec![step.frequency.map_or(Cell::Missing, Cell::Frequency)];
        for (p, rms) in step.phasors.iter().zip(&step.rms) {
            let polar = p.map(|p| Polar::new(p, angle0));
            cells.push(polar.map_or(Cell::Missing, |p| Cell::Value(p.magnitude)));
            if self.angles {
                cells.push(polar.map_or(Cell::Missing, |p| Cell::Angle(p.angle)));
            }
            if self.rms {
                cells.push(rms.map_or(Cell::Missing, Cell::Value));
            }
        }
        for s in &step.sequences {
            match s {
                Some(s) => {
                    let largest = [s.positive, s.negative, s.zero]
                        .map(|p| p.abs())
                        .into_iter()
                        .fold(0.0, f64::max);
                    for p in [s.positive, s.negative, s.zero] {
                        // Residue of the integer samples (1e-8 of the rest) is no component.
                        let m = Polar::new(p, 0.0).magnitude;
                        cells.push(Cell::Value(if p.abs() <= 1e-6 * largest { 0.0 } else { m }));
                    }
                    cells.push(s.unbalance().map_or(Cell::Missing, Cell::Percent));
                }
                None => cells.extend((0..4).map(|_| Cell::Missing)),
            }
        }
        cells
    }

    fn text(&self, steps: &[Step], reference: usize) -> String {
        let headers = self.headers();
        let names: Vec<&str> = headers.iter().map(|(t, _)| t.as_str()).collect();
        let span = steps.last().map_or(0.0, |s| s.time) - steps.first().map_or(0.0, |s| s.time);
        let rows: Vec<Vec<String>> = steps
            .iter()
            .map(|step| {
                let mut row = vec![time(step.time, span.max(1e-3))];
                row.extend(self.cells(step, reference).iter().map(|c| match c {
                    Cell::Value(v) => si(*v),
                    Cell::Angle(a) => format!("{a:.1}°"),
                    Cell::Percent(p) => format!("{p:.1} %"),
                    Cell::Frequency(f) => format!("{f:.3}"),
                    Cell::Missing => "-".into(),
                }));
                row
            })
            .collect();
        table(&names, &rows)
    }

    fn csv(&self, steps: &[Step], reference: usize) -> String {
        let headers = self.headers();
        let mut out = headers.iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>().join(",") + "\n";
        for step in steps {
            let mut row = vec![round12(step.time * 1e3).to_string()];
            row.extend(self.cells(step, reference).iter().map(|c| match c {
                Cell::Value(v) | Cell::Angle(v) | Cell::Percent(v) | Cell::Frequency(v) => round12(*v).to_string(),
                Cell::Missing => String::new(),
            }));
            out += &(row.join(",") + "\n");
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn json(
        &self,
        c: &Comtrade,
        steps: &[Step],
        reference: usize,
        frequency: f64,
        samples: usize,
        step_samples: usize,
        side: Side,
    ) -> Json {
        let rounded = |p: Polar| Polar {
            magnitude: round12(p.magnitude),
            angle: round12(p.angle),
        };
        Json {
            file: c
                .path
                .as_ref()
                .map(|p| p.main_file().display().to_string())
                .unwrap_or_default(),
            side: if side == Side::Secondary {
                "secondary"
            } else {
                "primary"
            },
            frequency,
            samples,
            step_samples,
            reference: self.channels[reference].info.id.clone(),
            channels: self
                .channels
                .iter()
                .map(|ch| JsonChannel {
                    index: ch.info.index,
                    id: ch.info.id.clone(),
                    unit: ch.info.unit.clone(),
                })
                .collect(),
            groups: self
                .groups
                .iter()
                .zip(&self.labels)
                .map(|(g, label)| JsonGroup {
                    label: label.clone(),
                    circuit: g.circuit.clone(),
                    unit: g.unit.clone(),
                    channels: g.phases.map(|i| self.channels[i].info.id.clone()),
                })
                .collect(),
            steps: steps
                .iter()
                .map(|s| {
                    let angle0 = s.phasors[reference].map_or(0.0, |p| p.arg());
                    JsonStep {
                        time_s: round12(s.time),
                        frequency: s.frequency.map(round12),
                        phasors: s
                            .phasors
                            .iter()
                            .map(|p| p.map(|p| rounded(Polar::new(p, angle0))))
                            .collect(),
                        rms: s.rms.iter().map(|r| r.map(round12)).collect(),
                        sequences: s
                            .sequences
                            .iter()
                            .map(|q| {
                                q.map(|q| JsonSequence {
                                    zero: rounded(Polar::new(q.zero, angle0)),
                                    positive: rounded(Polar::new(q.positive, angle0)),
                                    negative: rounded(Polar::new(q.negative, angle0)),
                                    unbalance: q.unbalance().map(round12),
                                })
                            })
                            .collect(),
                    }
                })
                .collect(),
        }
    }
}

/// A CSV header name without separators or quotes.
fn csv_name(s: &str) -> String {
    s.replace([',', '"', ' '], "_")
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    samples: usize,
    step_samples: usize,
    reference: String,
    channels: Vec<JsonChannel>,
    groups: Vec<JsonGroup>,
    steps: Vec<JsonStep>,
}

#[derive(Serialize)]
struct JsonChannel {
    index: usize,
    id: String,
    unit: String,
}

#[derive(Serialize)]
struct JsonGroup {
    label: String,
    circuit: String,
    unit: String,
    channels: [String; 3],
}

#[derive(Serialize)]
struct JsonStep {
    time_s: f64,
    frequency: Option<f64>,
    phasors: Vec<Option<Polar>>,
    rms: Vec<Option<f64>>,
    sequences: Vec<Option<JsonSequence>>,
}

#[derive(Serialize)]
struct JsonSequence {
    zero: Polar,
    positive: Polar,
    negative: Polar,
    unbalance: Option<f64>,
}
