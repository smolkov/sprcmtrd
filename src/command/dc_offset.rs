//! `dc-offset` command: the decaying DC offset of the fault current per
//! phase (DC part at inception, time constant, X/R).

use anyhow::bail;
use serde::Serialize;

use super::fault::currents;
use super::{describe_window, time_window};
use crate::analysis::Window;
use crate::analysis::dc_offset::{self, Decay, SIGNIFICANT_PART, TimeConstant};
use crate::analysis::fault::{self, PHASES};
use crate::args::DcOffsetArgs;
use crate::comtrade::{Comtrade, round12};
use crate::context::Context;
use crate::format::{key_values, si, table, time as format_time};

#[derive(Serialize)]
struct JsonWindow {
    from_s: f64,
    to_s: f64,
    samples: usize,
}

#[derive(Serialize)]
struct JsonPhase {
    phase: &'static str,
    channel: String,
    faulted: bool,
    /// DC part at inception in A.
    dc: Option<f64>,
    dc_percent: Option<f64>,
    /// Time constant in s; `null` without a significant DC part.
    tau_s: Option<f64>,
    /// The DC part hardly decays within the window (τ above 1 s).
    tau_above_limit: bool,
    x_over_r: Option<f64>,
    /// RMS of the AC part in A.
    ac_rms: Option<f64>,
    /// Instantaneous peak in A from inception to the interruption.
    peak: Option<f64>,
    peak_s: Option<f64>,
    fit_error_percent: Option<f64>,
}

#[derive(Serialize)]
struct JsonFound {
    fault_type: String,
    inception_s: f64,
    interruption_s: Option<f64>,
    window: JsonWindow,
    phases: Vec<JsonPhase>,
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    current: [String; 3],
    found: bool,
    #[serde(flatten)]
    fault: Option<JsonFound>,
}

/// A percentage with one decimal, without a `-0.0`.
fn percent(part: f64) -> String {
    let v = part * 100.0;
    format!("{:.1} %", if v.abs() < 0.05 { 0.0 } else { v })
}

/// τ as text: `40.0 ms`, `> 1 s`, `-`.
fn tau_text(tau: TimeConstant) -> String {
    match tau {
        TimeConstant::Fitted(t) => format!("{:.1} ms", t * 1e3),
        TimeConstant::Longer => format!("> {} s", dc_offset::TAU_MAX),
        TimeConstant::Insignificant => "-".to_owned(),
    }
}

impl DcOffsetArgs {
    /// Runs the `dc-offset` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = ctx.global.side();
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        let current = currents(&c, &self.current)?;
        let (t0, t1) = time_window(&c, self.from, self.to)?;
        let range = c.index_range(t0, t1);
        let time = c.time();
        let found = fault::analyse(current.channels, time, range.clone(), frequency, side);
        let result = found
            .as_ref()
            .map(|f| dc_offset::analyse(current.channels, time, f, range.end, self.cycles, frequency, side));

        let file = c.name();
        let side_name = side.name();
        let k = current.factor;
        // Below 1e-6 of the largest AC peak is the residue of integer samples.
        let largest = result
            .iter()
            .flat_map(|r| r.phases.iter().flatten())
            .map(|d| d.ac_peak)
            .fold(0.0, f64::max);
        let dc_of = |d: &Decay| if d.dc.abs() < 1e-6 * largest { 0.0 } else { d.dc };

        if self.json {
            let fault = found.as_ref().zip(result.as_ref()).map(|(f, r)| JsonFound {
                fault_type: f.kind.name(),
                inception_s: round12(time[f.inception]),
                interruption_s: f.interruption.map(|i| round12(time[i])),
                window: JsonWindow {
                    from_s: round12(time[r.window.start]),
                    to_s: round12(time[r.window.end.saturating_sub(1).max(r.window.start)]),
                    samples: r.window.len(),
                },
                phases: (0..3)
                    .map(|p| {
                        let d = r.phases[p].as_ref();
                        JsonPhase {
                            phase: PHASES[p],
                            channel: current.channels[p].info.id.clone(),
                            faulted: f.kind.phases[p],
                            dc: d.map(|d| round12(dc_of(d) * k)),
                            dc_percent: d.map(|d| round12(dc_of(d) / d.ac_peak * 100.0)),
                            tau_s: d.and_then(|d| d.tau.seconds()).map(round12),
                            tau_above_limit: d.is_some_and(|d| d.tau == TimeConstant::Longer),
                            x_over_r: d.and_then(|d| d.x_over_r(frequency)).map(round12),
                            ac_rms: d.map(|d| round12(d.ac_peak / 2f64.sqrt() * k)),
                            peak: f.peaks[p].map(|x| round12(x.value * k)),
                            peak_s: f.peaks[p].map(|x| round12(time[x.sample])),
                            fit_error_percent: d.map(|d| round12(d.fit_error * 100.0)),
                        }
                    })
                    .collect(),
            });
            let out = Json {
                file,
                side: side_name,
                frequency,
                current: current.ids(),
                found: fault.is_some(),
                fault,
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        let (first, last) = c.time_span();
        let span = last - first;
        let signed = |t: f64| {
            let s = format_time(t, span);
            if t > 0.0 { format!("+{s}") } else { s }
        };
        let mut head = vec![
            ("File", file),
            ("Currents", format!("{} [{}]", current.ids().join(" "), current.unit)),
            ("Values", side_name.to_owned()),
        ];
        let (Some(f), Some(r)) = (found, result) else {
            head.push(("Searched", format!("{} .. {}", signed(t0), signed(t1))));
            print!("{}\nNo fault found.\n", key_values(&head));
            return Ok(());
        };
        head.push((
            "Fault",
            format!(
                "{} at {}{}",
                f.kind.name(),
                signed(time[f.inception]),
                f.interruption
                    .map_or_else(String::new, |i| format!(", interrupted at {}", signed(time[i])))
            ),
        ));
        let last_sample = r.window.end.saturating_sub(1).max(r.window.start);
        let cycles = (r.window.len() as f64 / f.per_cycle as f64 * 100.0).round() / 100.0;
        let window = Window {
            end: last_sample,
            samples: r.window.len(),
            frequency,
        };
        head.push((
            "Fit",
            describe_window(&window, time[r.window.start], time[last_sample], cycles, frequency),
        ));
        let mut out = key_values(&head);
        if r.window.len() < f.per_cycle {
            out += "\nThe fault is shorter than one cycle; no fit.\n";
            print!("{out}");
            return Ok(());
        }
        let dash = || "-".to_owned();
        let rows: Vec<Vec<String>> = (0..3)
            .map(|p| {
                let d = r.phases[p].as_ref();
                let peak = f.peaks[p];
                vec![
                    PHASES[p].to_owned(),
                    if f.kind.phases[p] { "faulted" } else { "" }.to_owned(),
                    d.map_or_else(dash, |d| si(dc_of(d) * k)),
                    d.map_or_else(dash, |d| percent(dc_of(d) / d.ac_peak)),
                    d.map_or_else(dash, |d| tau_text(d.tau)),
                    d.and_then(|d| d.x_over_r(frequency))
                        .map_or_else(dash, |x| format!("{x:.1}")),
                    d.map_or_else(dash, |d| si(d.ac_peak / 2f64.sqrt() * k)),
                    peak.map_or_else(dash, |x| si(x.value * k)),
                    peak.map_or_else(dash, |x| signed(time[x.sample])),
                    d.map_or_else(dash, |d| percent(d.fit_error)),
                ]
            })
            .collect();
        out += &format!(
            "\n{}",
            table(
                &[
                    "phase",
                    "",
                    "DC [A]",
                    "DC %",
                    "τ",
                    "X/R",
                    "AC RMS [A]",
                    "peak [A]",
                    "peak at",
                    "fit error",
                ],
                &rows
            )
        );
        out += &format!(
            "\nDC at inception in % of the AC peak; τ only for a DC part of at least {} %.\n",
            SIGNIFICANT_PART * 100.0
        );
        if r.phases.iter().any(Option::is_none) {
            out += "A phase without values in the window has no fit.\n";
        }
        print!("{out}");
        Ok(())
    }
}
