//! `frequency` command: system frequency over time and its rate of change.

use anyhow::{anyhow, bail};
use serde::Serialize;

use super::{select_channels, time_window};
use crate::analysis::frequency::{self, AtTime, Method, Point, Summary};
use crate::analysis::impedance::{Quantity, unit_factor};
use crate::analysis::phasor::{Window, WindowError};
use crate::args::{FrequencyArgs, FrequencyMethod, TableFormat, TrendStep};
use crate::comtrade::{AnalogChannel, Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, rate, table, time};

impl FrequencyArgs {
    /// Runs the `frequency` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        };
        let ch = channel(&c, self.channel.as_deref())?;
        let nominal = self.frequency.unwrap_or(c.config.line_frequency);
        if nominal <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        let method = match self.method {
            FrequencyMethod::Crossings => Method::Crossings,
            FrequencyMethod::Phasor => Method::Phasor,
        };
        if method == Method::Crossings && self.cycles.fract() != 0.0 {
            bail!("--method crossings measures whole periods; give --cycles as a whole number");
        }

        // Measurement length from the sample spacing at the end of the recording.
        let (first, last) = c.time_span();
        let window = Window::ending_at(&c, last, self.cycles, nominal).map_err(|e| match e {
            WindowError::TooShort(n) => anyhow!(
                "{} cycles at {nominal} Hz are only {n} samples; use more --cycles",
                self.cycles
            ),
            WindowError::BeforeStart { samples, .. } => {
                anyhow!("the recording is shorter than one measurement of {samples} samples")
            }
            e => anyhow!(e),
        })?;
        let samples = window.samples;
        let dt = self.cycles / nominal / samples as f64;
        let step = match self.step {
            TrendStep::Cycles(n) => n / nominal,
            TrendStep::Seconds(s) => s,
        };
        let step_samples = ((step / dt).round() as usize).max(1);

        let (t0, t1) = time_window(&c, self.from, self.to)?;
        let range = c.index_range(t0, t1);
        let points = frequency::series(
            ch,
            c.time(),
            range,
            self.cycles,
            samples,
            step_samples,
            nominal,
            method,
            side,
        );
        if points.is_empty() {
            bail!(
                "no measurement of {samples} samples fits in the time range; the recording runs from {} to {}",
                time(first, last - first),
                time(last, last - first)
            );
        }
        let summary = frequency::summary(&points);

        match self.format {
            TableFormat::Csv => print!("{}", csv(&points, nominal)),
            TableFormat::Json => {
                let json = Json {
                    file: file_name(&c),
                    channel: JsonChannel {
                        index: ch.info.index,
                        id: ch.info.id.clone(),
                        unit: ch.info.unit.clone(),
                    },
                    method: method_name(method),
                    nominal,
                    cycles: self.cycles,
                    samples,
                    step_samples,
                    summary: summary.map(|s| Summary {
                        min: rounded(s.min),
                        max: rounded(s.max),
                        mean: round12(s.mean),
                        rocof: s.rocof.map(|r| AtTime {
                            value: snap(r.value),
                            ..rounded(r)
                        }),
                    }),
                    points: points
                        .iter()
                        .map(|p| JsonPoint {
                            time_s: round12(p.time),
                            frequency: p.frequency.map(round12),
                            deviation: p.frequency.map(|f| deviation(f, nominal)),
                            rocof: p.rocof.map(snap),
                        })
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            TableFormat::Text => {
                let cycles = |n: f64| format!("{n} {}", if n == 1.0 { "cycle" } else { "cycles" });
                let step_text = match self.step {
                    TrendStep::Cycles(n) => cycles(n),
                    TrendStep::Seconds(s) => time(s, s),
                };
                let span = points.last().map_or(0.0, |p| p.time) - points[0].time;
                let at = |t: f64| time(t, span.max(1e-3));
                let mut pairs = vec![
                    ("File", file_name(&c)),
                    (
                        "Channel",
                        format!("A{} {} [{}]", ch.info.index, ch.info.id, ch.info.unit),
                    ),
                    (
                        "Method",
                        format!(
                            "{} over {} ({samples} samples), every {step_text} ({} rows)",
                            method_name(method).replace('_', " "),
                            cycles(self.cycles),
                            points.len()
                        ),
                    ),
                    ("Nominal", rate(nominal)),
                ];
                match summary {
                    Some(s) => {
                        pairs.push((
                            "Frequency",
                            format!(
                                "min {:.3} Hz at {}, max {:.3} Hz at {}, mean {:.3} Hz",
                                s.min.value,
                                at(s.min.time_s),
                                s.max.value,
                                at(s.max.time_s),
                                s.mean
                            ),
                        ));
                        if let Some(r) = s.rocof {
                            pairs.push(("ROCOF", format!("largest {} Hz/s at {}", signed(r.value), at(r.time_s))));
                        }
                    }
                    None => pairs.push(("Frequency", "no signal to measure".into())),
                }
                print!("{}", key_values(&pairs));
                println!("\n{}", text(&points, nominal, span).trim_end());
            }
        }
        Ok(())
    }
}

/// The channel to measure: `--channel`, else the first voltage channel, else
/// the first analog channel.
fn channel<'a>(c: &'a Comtrade, selection: Option<&str>) -> anyhow::Result<&'a AnalogChannel> {
    let Some(token) = selection else {
        return c
            .analog_channels()
            .find(|a| unit_factor(&a.info.unit).is_some_and(|(q, _)| q == Quantity::Voltage))
            .or_else(|| c.analog(0))
            .ok_or_else(|| anyhow!("the recording has no analog channels"));
    };
    let (analog, binary) = select_channels(c, &[token.to_owned()])?;
    if let Some(&i) = binary.first() {
        let ch = c.binary(i).expect("selected index is valid");
        bail!("{} is a binary channel; frequency needs an analog channel", ch.info.id);
    }
    let i = analog.first().copied().expect("one channel was selected");
    Ok(c.analog(i).expect("selected index is valid"))
}

fn file_name(c: &Comtrade) -> String {
    c.path
        .as_ref()
        .map(|p| p.main_file().display().to_string())
        .unwrap_or_default()
}

fn method_name(method: Method) -> &'static str {
    match method {
        Method::Crossings => "zero_crossings",
        Method::Phasor => "phasor",
    }
}

fn rounded(a: AtTime) -> AtTime {
    AtTime {
        value: round12(a.value),
        time_s: round12(a.time_s),
    }
}

/// Deviation of `f` from `nominal`, without the float noise of `f`.
fn deviation(f: f64, nominal: f64) -> f64 {
    round12(round12(f) - nominal)
}

/// `v` rounded to 12 significant digits, and below 1e-9 to 0: the noise of
/// a difference of equal frequencies.
fn snap(v: f64) -> f64 {
    if v.abs() < 1e-9 { 0.0 } else { round12(v) }
}

/// `v` with 3 decimals and a sign; a value that rounds to zero is `0.000`.
fn signed(v: f64) -> String {
    if (v * 1e3).round() == 0.0 {
        "0.000".into()
    } else {
        format!("{v:+.3}")
    }
}

fn text(points: &[Point], nominal: f64, span: f64) -> String {
    let rows: Vec<Vec<String>> = points
        .iter()
        .map(|p| {
            let missing = || "-".to_owned();
            vec![
                time(p.time, span.max(1e-3)),
                p.frequency.map_or_else(missing, |f| format!("{f:.3}")),
                p.frequency.map_or_else(missing, |f| signed(f - nominal)),
                p.rocof.map_or_else(missing, signed),
            ]
        })
        .collect();
    table(&["time", "f [Hz]", "Δf [Hz]", "df/dt [Hz/s]"], &rows)
}

fn csv(points: &[Point], nominal: f64) -> String {
    let value = |v: Option<f64>| v.map(|v| round12(v).to_string()).unwrap_or_default();
    let mut out = String::from("time_ms,frequency_hz,deviation_hz,rocof_hz_per_s\n");
    for p in points {
        out += &format!(
            "{},{},{},{}\n",
            round12(p.time * 1e3),
            value(p.frequency),
            value(p.frequency.map(|f| deviation(f, nominal))),
            value(p.rocof.map(snap))
        );
    }
    out
}

#[derive(Serialize)]
struct Json {
    file: String,
    channel: JsonChannel,
    method: &'static str,
    nominal: f64,
    cycles: f64,
    samples: usize,
    step_samples: usize,
    summary: Option<Summary>,
    points: Vec<JsonPoint>,
}

#[derive(Serialize)]
struct JsonChannel {
    index: usize,
    id: String,
    unit: String,
}

#[derive(Serialize)]
struct JsonPoint {
    time_s: f64,
    frequency: Option<f64>,
    deviation: Option<f64>,
    rocof: Option<f64>,
}
