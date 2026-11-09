//! `trend` command: values over time, cycle by cycle.

use crate::term::{out, outln};
use anyhow::{anyhow, bail};
use serde::Serialize;

use super::{select_channels, time_window};
use crate::analysis::frequency::{self, Candidate, Source};
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
        let side = ctx.global.side();
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
        let result = trend::trend(&channels, c.time(), range, samples, step_samples, frequency, side);
        if result.steps.is_empty() {
            bail!(
                "no window of {samples} samples fits in the time range; the recording runs from {} to {}",
                time(first, last - first),
                time(last, last - first)
            );
        }

        // The frequency, from all analog channels of the recording.
        let all: Vec<&AnalogChannel> = c.analog_channels().collect();
        let candidates: Vec<Candidate> = match &self.frequency_from {
            Some(token) => {
                let (found, _) = select_channels(&c, std::slice::from_ref(token))?;
                let Some(&i) = found.first() else {
                    bail!("--frequency-from takes an analog channel; {token} is binary");
                };
                vec![Candidate {
                    source: Source::Channel(i),
                    peak: frequency::peak(all[i], side),
                }]
            }
            None => frequency::sources(&all, side),
        };
        let frequencies: Vec<Option<(f64, usize)>> = result
            .steps
            .iter()
            .map(|st| frequency::at(&all, c.time(), st.end, samples, frequency, &candidates, side))
            .collect();
        let source_names: Vec<String> = candidates.iter().map(|k| source_name(&all, k.source)).collect();
        let mut used: Vec<usize> = frequencies.iter().flatten().map(|&(_, n)| n).collect();
        used.sort_unstable();
        used.dedup();

        let table = Table {
            frequencies: &frequencies,
            source_names: &source_names,
            show_source: used.len() > 1,
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
            TableFormat::Csv => out!("{}", table.csv(&result.steps, reference))?,
            TableFormat::Json => {
                let json = table.json(&c, &result.steps, reference, frequency, samples, step_samples, side);
                outln!("{}", serde_json::to_string_pretty(&json)?)?;
            }
            TableFormat::Text => {
                let name = c.name();
                let ch = channels[reference];
                let cycles = |n: f64| format!("{n} {}", if n == 1.0 { "cycle" } else { "cycles" });
                let step_text = match self.step {
                    TrendStep::Cycles(n) => cycles(n),
                    TrendStep::Seconds(s) => time(s, s),
                };
                out!(
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
                        ("Reference", format!("A{} {} (angles)", ch.info.index, ch.info.id)),
                        (
                            "Frequency",
                            if used.is_empty() {
                                "no voltage or current with a signal".to_owned()
                            } else {
                                used.iter()
                                    .map(|&n| source_text(&all, &source_names[n], candidates[n].source))
                                    .collect::<Vec<_>>()
                                    .join(", else ")
                            },
                        ),
                        ("Values", format!("{}, RMS of the fundamental", side.name()),),
                    ])
                )?;
                outln!("\n{}", table.text(&result.steps, reference).trim_end())?;
            }
        }
        Ok(())
    }
}

/// Start of a group's sequence column names: the common start of its channel
/// ids (`I` for IA, IB, IC), else `G1 `, `G2 `, … (with a space, so the
/// zero sequence of group 1 doesn't read `G10`).
fn label(channels: &[&AnalogChannel], g: &Group, n: usize) -> String {
    let common = common_start(g.phases.map(|i| channels[i].info.id.as_str()));
    if common.is_empty() {
        format!("G{} ", n + 1)
    } else {
        common
    }
}

/// The common start of three ids, without a trailing separator.
fn common_start(ids: [&str; 3]) -> String {
    let common: String = ids[0]
        .chars()
        .zip(ids[1].chars())
        .zip(ids[2].chars())
        .take_while(|((a, b), c)| a == b && b == c)
        .map(|((a, _), _)| a)
        .collect();
    common.trim_end_matches([' ', '_', '-']).to_owned()
}

/// Short name of a frequency source: `uL+` for the positive sequence of
/// uL1E, uL2E, uL3E (`+` alone without a common start), or the channel id.
fn source_name(channels: &[&AnalogChannel], source: Source) -> String {
    match source {
        Source::Group(p) => common_start(p.map(|i| channels[i].info.id.as_str())) + "+",
        Source::Channel(i) => channels[i].info.id.clone(),
    }
}

/// A frequency source for the header: `uL+ (uL1E uL2E uL3E)`, `A6 uL1E`.
fn source_text(channels: &[&AnalogChannel], name: &str, source: Source) -> String {
    match source {
        Source::Group(p) => format!("{name} ({})", p.map(|i| channels[i].info.id.as_str()).join(" ")),
        Source::Channel(i) => format!("A{} {}", channels[i].info.index, channels[i].info.id),
    }
}

/// What the rows show.
struct Table<'a> {
    /// Per step the frequency and the position of its source.
    frequencies: &'a [Option<(f64, usize)>],
    /// Short names of the frequency sources (`uL+`, `uL1E`).
    source_names: &'a [String],
    /// The text table has a column of the source (it changes).
    show_source: bool,
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
    Text(String),
    Missing,
}

impl Table<'_> {
    /// Column names as `(text header, CSV header)`, with the frequency
    /// source column when `source`.
    fn headers(&self, source: bool) -> Vec<(String, String)> {
        let mut h = vec![
            ("time".to_owned(), "time_ms".to_owned()),
            ("f [Hz]".to_owned(), "frequency_hz".to_owned()),
        ];
        if source {
            h.push(("f from".to_owned(), "frequency_source".to_owned()));
        }
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
            // `+`, `-` and `0` can't be mistaken for a phase (iL1, iL2);
            // words in CSV, which some tools don't allow `+` / `-` in.
            let csv = label.trim();
            for (n, word) in [("+", "pos"), ("-", "neg"), ("0", "zero")] {
                h.push((
                    format!("{label}{n} [{}]", g.unit),
                    csv_name(&format!("{csv}_{word}_{}", g.unit)),
                ));
            }
            h.push((format!("{label}-/+"), csv_name(&format!("{csv}_unbalance_pct"))));
        }
        h
    }

    /// The cells of step `k`, angles relative to the reference channel, with
    /// the frequency source when `source`.
    fn cells(&self, steps: &[Step], k: usize, reference: usize, source: bool) -> Vec<Cell> {
        let step = &steps[k];
        let angle0 = step.phasors[reference].map_or(0.0, |p| p.arg());
        let f = self.frequencies[k];
        let mut cells = vec![f.map_or(Cell::Missing, |(f, _)| Cell::Frequency(f))];
        if source {
            cells.push(f.map_or(Cell::Missing, |(_, n)| Cell::Text(self.source_names[n].clone())));
        }
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
        let headers = self.headers(self.show_source);
        let names: Vec<&str> = headers.iter().map(|(t, _)| t.as_str()).collect();
        let span = steps.last().map_or(0.0, |s| s.time) - steps.first().map_or(0.0, |s| s.time);
        let rows: Vec<Vec<String>> = steps
            .iter()
            .enumerate()
            .map(|(k, step)| {
                let mut row = vec![time(step.time, span.max(1e-3))];
                row.extend(
                    self.cells(steps, k, reference, self.show_source)
                        .into_iter()
                        .map(|c| match c {
                            Cell::Value(v) => si(v),
                            Cell::Angle(a) => format!("{a:.1}°"),
                            Cell::Percent(p) => format!("{p:.1} %"),
                            Cell::Frequency(f) => format!("{f:.3}"),
                            Cell::Text(t) => t,
                            Cell::Missing => "-".into(),
                        }),
                );
                row
            })
            .collect();
        table(&names, &rows)
    }

    fn csv(&self, steps: &[Step], reference: usize) -> String {
        let headers = self.headers(true);
        let mut out = headers.iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>().join(",") + "\n";
        for (k, step) in steps.iter().enumerate() {
            let mut row = vec![round12(step.time * 1e3).to_string()];
            row.extend(self.cells(steps, k, reference, true).into_iter().map(|c| match c {
                Cell::Value(v) | Cell::Angle(v) | Cell::Percent(v) | Cell::Frequency(v) => round12(v).to_string(),
                Cell::Text(t) => csv_name(&t),
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
            file: c.name(),
            side: side.name(),
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
                    label: label.trim().to_owned(),
                    circuit: g.circuit.clone(),
                    unit: g.unit.clone(),
                    channels: g.phases.map(|i| self.channels[i].info.id.clone()),
                })
                .collect(),
            steps: steps
                .iter()
                .zip(self.frequencies)
                .map(|(s, f)| {
                    let angle0 = s.phasors[reference].map_or(0.0, |p| p.arg());
                    JsonStep {
                        time_s: round12(s.time),
                        frequency: f.map(|(f, _)| round12(f)),
                        frequency_source: f.map(|(_, n)| self.source_names[n].clone()),
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
pub(super) fn csv_name(s: &str) -> String {
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
    frequency_source: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::sequence;
    use crate::comtrade::AnalogInfo;

    /// A recording with one current channel per id, phases A, B, C.
    fn recording(ids: [&str; 3]) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n3,3A,0D\n");
        for (i, (id, p)) in ids.iter().zip(["A", "B", "C"]).enumerate() {
            cfg += &format!("{},{id},{p},LINE,A,1,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += "50\n1\n1000,2\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        Comtrade::from_readers(cfg.as_bytes(), "1,0,1,2,3\n2,1000,1,2,3\n".as_bytes()).unwrap()
    }

    fn headers(ids: [&str; 3]) -> Vec<(String, String)> {
        let c = recording(ids);
        let channels: Vec<&AnalogChannel> = c.analog_channels().collect();
        let infos: Vec<&AnalogInfo> = channels.iter().map(|ch| &ch.info).collect();
        let groups = sequence::groups(&infos);
        let table = Table {
            frequencies: &[],
            source_names: &[],
            show_source: false,
            channels: &channels,
            groups: &groups,
            labels: groups.iter().enumerate().map(|(n, g)| label(&channels, g, n)).collect(),
            angles: false,
            rms: false,
        };
        table.headers(false).split_off(5)
    }

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn sequence_columns_differ_from_the_phases() {
        // iL1 iL2 iL3 used to give iL1 iL2 iL0, the names of the phases.
        assert_eq!(
            headers(["iL1", "iL2", "iL3"]),
            pairs(&[
                ("iL+ [A]", "iL_pos_A"),
                ("iL- [A]", "iL_neg_A"),
                ("iL0 [A]", "iL_zero_A"),
                ("iL-/+", "iL_unbalance_pct"),
            ])
        );
        assert_eq!(
            headers(["IA", "IB", "IC"])[0],
            ("I+ [A]".to_owned(), "I_pos_A".to_owned())
        );
        // No common start: G1 with a space, so the zero sequence isn't G10.
        assert_eq!(
            headers(["X", "Y", "Z"]),
            pairs(&[
                ("G1 + [A]", "G1_pos_A"),
                ("G1 - [A]", "G1_neg_A"),
                ("G1 0 [A]", "G1_zero_A"),
                ("G1 -/+", "G1_unbalance_pct"),
            ])
        );
    }
}
