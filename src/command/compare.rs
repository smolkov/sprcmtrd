//! `compare` command: two recordings of the same event side by side, with
//! the phasors of paired channels and the differential current of two line
//! ends, at one moment, cycle by cycle (`--trend`) or as waveforms
//! (`--plot`).

use anyhow::{Context as _, anyhow, bail};
use serde::Serialize;

use super::chart::{self, LABEL_WIDTH};
use super::trend::csv_name;
use super::{factor, group, select_channels, step_grid, time_window, window_at};
use crate::analysis::compare::{self, Align, Aligned, Differential, Pair, window_b};
use crate::analysis::fault::PHASES;
use crate::analysis::impedance::{Quantity, find_current, unit_factor};
use crate::analysis::phasor::{self, Polar, Window};
use crate::analysis::{Complex, sequence::Group};
use crate::args::{AlignArg, CompareArgs, TableFormat, TrendStep};
use crate::comtrade::{AnalogChannel, AnalogInfo, Comtrade, Side, decimated_segments, round12};
use crate::context::Context;
use crate::format::{key_values, rate, si, table, time};
use crate::term;

#[derive(Serialize)]
struct JsonRecording {
    file: String,
    station: String,
}

#[derive(Serialize)]
struct JsonAlignment {
    mode: &'static str,
    /// B's trigger in A's time, s: t_A = t_B + offset.
    offset_s: f64,
    /// The part of the offset from --shift, s.
    shift_s: f64,
}

#[derive(Serialize)]
struct JsonWindow {
    from_s: f64,
    to_s: f64,
    samples: usize,
}

#[derive(Serialize)]
struct JsonPair {
    a: String,
    b: String,
    unit: String,
    a_rms: Option<f64>,
    b_rms: Option<f64>,
    ratio: Option<f64>,
    a_angle: Option<f64>,
    b_angle: Option<f64>,
    angle_difference: Option<f64>,
}

#[derive(Serialize)]
struct JsonPhase {
    phase: &'static str,
    a: f64,
    b: f64,
    diff: f64,
    restraint: f64,
    percent: Option<f64>,
}

#[derive(Serialize)]
struct JsonDifferential {
    a: [String; 3],
    b: [String; 3],
    inverted_b: bool,
    phases: Option<Vec<JsonPhase>>,
}

#[derive(Serialize)]
struct Json {
    a: JsonRecording,
    b: JsonRecording,
    side: &'static str,
    frequency: f64,
    alignment: JsonAlignment,
    window_a: JsonWindow,
    /// `null` when B has no samples at A's window.
    window_b: Option<JsonWindow>,
    reference: String,
    pairs: Vec<JsonPair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    differential: Option<JsonDifferential>,
}

#[derive(Serialize)]
struct JsonCurrents {
    a: [String; 3],
    b: [String; 3],
    inverted_b: bool,
}

#[derive(Serialize)]
struct JsonTrendPair {
    a: String,
    b: String,
    ratio: Option<f64>,
    angle_difference: Option<f64>,
}

#[derive(Serialize)]
struct JsonTrendPhase {
    phase: &'static str,
    diff: Option<f64>,
    percent: Option<f64>,
}

#[derive(Serialize)]
struct JsonStep {
    time_s: f64,
    pairs: Vec<JsonTrendPair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    differential: Option<Vec<JsonTrendPhase>>,
}

#[derive(Serialize)]
struct JsonTrend {
    a: JsonRecording,
    b: JsonRecording,
    side: &'static str,
    frequency: f64,
    alignment: JsonAlignment,
    cycles: f64,
    samples: usize,
    step_s: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    differential: Option<JsonCurrents>,
    steps: Vec<JsonStep>,
}

/// A current A/B/C group and its factor to A.
struct Currents<'a> {
    channels: [&'a AnalogChannel; 3],
    factor: f64,
}

impl<'a> Currents<'a> {
    fn new(c: &'a Comtrade, tokens: &[String], option: &str, name: &str) -> anyhow::Result<Self> {
        let g: Group = if tokens.is_empty() {
            let infos: Vec<&AnalogInfo> = c.analog_channels().map(|ch| &ch.info).collect();
            find_current(&infos)
                .with_context(|| format!("no current A/B/C channel group found in {name}; give it with {option}"))?
        } else {
            group(c, tokens, option)?
        };
        Ok(Self {
            channels: g.phases.map(|i| c.analog(i).expect("channel position is valid")),
            factor: factor(&g, Quantity::Current)?,
        })
    }

    fn ids(&self) -> [String; 3] {
        self.channels.map(|ch| ch.info.id.clone())
    }
}

/// Factor from B's unit to A's: 1 for equal units, else both voltages or
/// both currents.
fn unit_scale(a: &str, b: &str) -> Option<f64> {
    if a.trim() == b.trim() {
        return Some(1.0);
    }
    match (unit_factor(a), unit_factor(b)) {
        (Some((qa, fa)), Some((qb, fb))) if qa == qb => Some(fb / fa),
        _ => None,
    }
}

/// An angle with one decimal in (-180, 180], without `-0.0`.
fn degrees(x: f64) -> String {
    let r = (x * 10.0).round() / 10.0;
    let r = if r <= -180.0 { r + 360.0 } else { r };
    format!("{:.1}°", if r == 0.0 { 0.0 } else { r })
}

fn str_refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

fn analog(c: &Comtrade, i: usize) -> &AnalogChannel {
    c.analog(i).expect("channel position is valid")
}

/// One compared pair: phasors (peak) in A's unit, B's in A's time frame.
struct Row<'a> {
    a: &'a AnalogChannel,
    b: &'a AnalogChannel,
    pa: Option<Complex>,
    pb: Option<Complex>,
}

/// What all three outputs share: the recordings, the pairs, the alignment
/// and the current groups.
struct Setup<'a> {
    a: &'a Comtrade,
    b: &'a Comtrade,
    side: Side,
    frequency: f64,
    align: Align,
    shift: f64,
    /// t_A = t_B + d.
    d: f64,
    /// The compared pairs, B's factor to A's unit.
    pairs: Vec<Pair<'a>>,
    /// The current groups of A and B (for --differential and --align fault).
    currents: Option<(Currents<'a>, Currents<'a>)>,
}

impl<'a> Setup<'a> {
    fn new(args: &CompareArgs, a: &'a Comtrade, b: &'a Comtrade, side: Side, with_pairs: bool) -> anyhow::Result<Self> {
        let frequency = match args.frequency {
            Some(f) => f,
            None => {
                let (fa, fb) = (a.config.line_frequency, b.config.line_frequency);
                if (fa - fb).abs() > 1e-9 {
                    bail!("A is recorded at {fa} Hz, B at {fb} Hz; give the frequency with --frequency");
                }
                fa
            }
        };
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        if args.cycles <= 0.0 {
            bail!("--cycles must be above 0");
        }

        // Channel pairs, limited to --channels of A.
        let pairs = if with_pairs {
            let ids = |c: &Comtrade| c.analog_channels().map(|ch| ch.info.id.clone()).collect::<Vec<_>>();
            let (ids_a, ids_b) = (ids(a), ids(b));
            let mut pairs = compare::pairs(&str_refs(&ids_a), &str_refs(&ids_b), &args.pair).map_err(|e| anyhow!(e))?;
            if !args.channels.is_empty() {
                let (only, binary) = select_channels(a, &args.channels)?;
                if !binary.is_empty() {
                    bail!("--channels takes analog channels of A");
                }
                pairs.retain(|(pa, _)| only.contains(pa));
                if pairs.is_empty() {
                    bail!("none of --channels has a channel with the same id in B; pair them with --pair");
                }
            }
            pairs
                .iter()
                .map(|&(pa, pb)| {
                    let (ca, cb) = (analog(a, pa), analog(b, pb));
                    let scale = unit_scale(&ca.info.unit, &cb.info.unit).with_context(|| {
                        format!(
                            "{} has the unit '{}' in A but {} has '{}' in B",
                            ca.info.id, ca.info.unit, cb.info.id, cb.info.unit
                        )
                    })?;
                    Ok(Pair { a: ca, b: cb, scale })
                })
                .collect::<anyhow::Result<_>>()?
        } else {
            Vec::new()
        };

        // Current groups: for the differential and for the fault alignment.
        let align = match args.align {
            AlignArg::Time => Align::Time,
            AlignArg::Trigger => Align::Trigger,
            AlignArg::Fault => Align::Fault,
        };
        let currents = if args.differential || align == Align::Fault {
            Some((
                Currents::new(a, &args.current_a, "--current-a", "A")?,
                Currents::new(b, &args.current_b, "--current-b", "B")?,
            ))
        } else {
            None
        };
        let fault_offset = match (&currents, align) {
            (Some((ca, cb)), Align::Fault) => Some(
                compare::fault_offset((a, ca.channels), (b, cb.channels), frequency, side).map_err(|e| anyhow!(e))?,
            ),
            _ => None,
        };
        let shift = args.shift / 1e3;
        let d = compare::offset(a, b, align, fault_offset, shift).map_err(|e| anyhow!(e))?;
        Ok(Self {
            a,
            b,
            side,
            frequency,
            align,
            shift,
            d,
            pairs,
            currents,
        })
    }

    fn json_recordings(&self) -> (JsonRecording, JsonRecording) {
        let r = |c: &Comtrade| JsonRecording {
            file: c.name(),
            station: c.config.station_name.clone(),
        };
        (r(self.a), r(self.b))
    }

    fn json_alignment(&self) -> JsonAlignment {
        JsonAlignment {
            mode: self.align.name(),
            offset_s: round12(self.d),
            shift_s: round12(self.shift),
        }
    }

    /// The A, B and Alignment lines of the text header.
    fn head(&self) -> Vec<(&'static str, String)> {
        let mode = match self.align {
            Align::Time => "absolute time",
            Align::Trigger => "triggers",
            Align::Fault => "fault inception",
        };
        let mut alignment = format!("{mode}: B's trigger at {} in A's time", signed(self.d));
        if self.shift != 0.0 {
            alignment += &format!(" (with --shift {})", signed(self.shift));
        }
        let recording = |c: &Comtrade| {
            let station = c.config.station_name.trim();
            if station.is_empty() {
                c.name()
            } else {
                format!("{} ({station})", c.name())
            }
        };
        vec![
            ("A", recording(self.a)),
            ("B", recording(self.b)),
            ("Alignment", alignment),
        ]
    }

    /// The differential pairs: phases A, B, C of the current groups, B in A's
    /// unit and inverted with `invert`; the factor from A's unit to amperes.
    fn differential_pairs(&self, invert: bool) -> Option<(Vec<Pair<'a>>, f64)> {
        let (ca, cb) = self.currents.as_ref()?;
        let sign = if invert { -1.0 } else { 1.0 };
        let pairs = (0..3)
            .map(|p| Pair {
                a: ca.channels[p],
                b: cb.channels[p],
                scale: cb.factor * sign / ca.factor,
            })
            .collect();
        Some((pairs, ca.factor))
    }
}

/// Whether `ch` is a current (by its unit), for `--invert-b`.
fn is_current(ch: &AnalogChannel) -> bool {
    unit_factor(ch.info.unit.trim()).is_some_and(|(q, _)| q == Quantity::Current)
}

/// The plot of B − A for pair `p` (`--difference`): B, times `scale`, at
/// each of A's sample times in `t0..t1` (interpolated), minus A, on its own
/// y range; the label gives the largest |B − A| and its share of A's peak.
fn difference_plot(s: &Setup, p: &Pair, scale: f64, (t0, t1): (f64, f64), cols: usize, rows: usize) -> String {
    let (a, b, d, side) = (s.a, s.b, s.d, s.side);
    let range = a.index_range(t0, t1);
    let diff = |i: usize| {
        let va = p.a.value_on(i, side)?;
        let vb = compare::value_at(p.b, b.time(), a.time()[i] - d, side)?;
        Some(vb * scale - va)
    };
    let largest = range.clone().filter_map(diff).map(f64::abs).reduce(f64::max);
    let peak = range
        .clone()
        .filter_map(|i| p.a.value_on(i, side))
        .map(f64::abs)
        .fold(0.0, f64::max);
    let unit = p.a.info.unit.trim();
    let mut label = format!("{} B − A [{unit}]", p.a.info.id.trim());
    match largest {
        Some(m) => {
            // Below 1e-6 of A's peak is float residue.
            let m = if m < 1e-6 * peak { 0.0 } else { m };
            label += &format!(", max |B − A| {}", si(m));
            if peak > 0.0 {
                label += &format!(" ({:.1} % of A's peak)", 100.0 * m / peak);
            }
        }
        None => label += ", B has no samples here",
    }
    let segments = decimated_segments(a.time(), range, (t0, t1), cols * 2, diff);
    format!("{label}\n{}", chart::plot(&segments, (t0, t1), None, cols, rows))
}

/// A time with its sign: `+12.00 ms`.
fn signed(t: f64) -> String {
    let s = time(t, t.abs().max(0.01));
    if t > 0.0 { format!("+{s}") } else { s }
}

impl CompareArgs {
    /// Runs the `compare` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        if self.plot && self.format != TableFormat::Text {
            bail!("--plot draws text only; leave out --format");
        }
        if !self.trend && self.format == TableFormat::Csv {
            bail!("--format csv needs --trend; use --format json for one moment");
        }
        let a = Comtrade::open(&self.a)?;
        let b = Comtrade::open(&self.b)?;
        let side = ctx.global.side();
        // The differential trend alone, unless channels are asked for.
        let with_pairs = !(self.trend && self.differential && self.channels.is_empty() && self.pair.is_empty());
        let setup = Setup::new(&self, &a, &b, side, with_pairs)?;
        if self.plot {
            self.waveforms(&setup)
        } else if self.trend {
            self.over_time(&setup)
        } else {
            self.moment(&setup)
        }
    }

    /// Phasors at one moment (`--at`).
    fn moment(&self, s: &Setup) -> anyhow::Result<()> {
        let (a, b, d, side, frequency) = (s.a, s.b, s.d, s.side, s.frequency);
        // Windows ending at the same instant.
        let wa = window_at(a, self.at, self.cycles, frequency)?;
        let t_end = a.time()[wa.end];
        let wb = window_b(b, t_end - d, self.cycles, frequency);
        let at_a = |ch: &AnalogChannel| phasor::phasor(ch, a.time(), &wa, side).ok();
        let at_b = |ch: &AnalogChannel| {
            wb.as_ref()
                .and_then(|w| phasor::phasor(ch, b.time(), w, side).ok())
                .map(|p| compare::rotate(p, d, frequency))
        };
        let rows: Vec<Row> = s
            .pairs
            .iter()
            .map(|p| Row {
                a: p.a,
                b: p.b,
                pa: at_a(p.a),
                pb: at_b(p.b).map(|x| x.scale(p.scale)),
            })
            .collect();

        // The angle reference: a channel of A, else the first pair's A.
        let (reference_name, reference) = match &self.reference {
            Some(r) => {
                let (found, _) = select_channels(a, std::slice::from_ref(r))?;
                let Some(&i) = found.first() else {
                    bail!("the reference {r} is not an analog channel of A");
                };
                let ch = analog(a, i);
                (
                    ch.info.id.clone(),
                    at_a(ch).with_context(|| format!("the reference {} has no phasor in the window", ch.info.id))?,
                )
            }
            None => (
                rows[0].a.info.id.clone(),
                rows[0]
                    .pa
                    .with_context(|| format!("the reference {} has no phasor in the window", rows[0].a.info.id))?,
            ),
        };
        let angle0 = reference.arg();

        // Angles of values that are float residue mean nothing.
        let largest = |unit: &str| {
            rows.iter()
                .filter(|r| r.a.info.unit == unit)
                .flat_map(|r| [r.pa, r.pb])
                .flatten()
                .map(Complex::abs)
                .fold(0.0, f64::max)
        };
        struct Values {
            a: Option<Polar>,
            b: Option<Polar>,
            ratio: Option<f64>,
            difference: Option<f64>,
            a_angle: bool,
            b_angle: bool,
        }
        let values: Vec<Values> = rows
            .iter()
            .map(|r| {
                let floor = 1e-6 * largest(&r.a.info.unit);
                let pa = r.pa.map(|p| Polar::new(p, angle0));
                let pb = r.pb.map(|p| Polar::new(p, angle0));
                let a_angle = r.pa.is_some_and(|p| p.abs() > floor);
                let b_angle = r.pb.is_some_and(|p| p.abs() > floor);
                Values {
                    a: pa,
                    b: pb,
                    ratio: match (r.pa, r.pb) {
                        (Some(x), Some(y)) if x.abs() > floor => Some(y.abs() / x.abs()),
                        _ => None,
                    },
                    difference: match (r.pa, r.pb) {
                        (Some(x), Some(y)) if a_angle && b_angle => Some(Polar::new(y, x.arg()).angle),
                        _ => None,
                    },
                    a_angle,
                    b_angle,
                }
            })
            .collect();

        // The differential current per phase, in A.
        let differential = if self.differential {
            let (ca, cb) = s
                .currents
                .as_ref()
                .expect("current groups are found for --differential");
            let sign = if self.invert_b { -1.0 } else { 1.0 };
            let phases: Option<Vec<(Complex, Complex, Differential)>> = (0..3)
                .map(|p| {
                    let ia = at_a(ca.channels[p])?.scale(ca.factor);
                    let ib = at_b(cb.channels[p])?.scale(cb.factor * sign);
                    Some((ia, ib, Differential::new(ia, ib)))
                })
                .collect();
            Some((ca, cb, phases))
        } else {
            None
        };

        let side_name = s.side.name();
        let rms = |x: f64| round12(x / 2f64.sqrt());
        if self.format == TableFormat::Json {
            let json_window = |c: &Comtrade, w: &Window| JsonWindow {
                from_s: round12(c.time()[w.start()]),
                to_s: round12(c.time()[w.end]),
                samples: w.samples,
            };
            let (ja, jb) = s.json_recordings();
            let out = Json {
                a: ja,
                b: jb,
                side: side_name,
                frequency,
                alignment: s.json_alignment(),
                window_a: json_window(a, &wa),
                window_b: wb.as_ref().map(|w| json_window(b, w)),
                reference: reference_name,
                pairs: rows
                    .iter()
                    .zip(&values)
                    .map(|(r, v)| JsonPair {
                        a: r.a.info.id.clone(),
                        b: r.b.info.id.clone(),
                        unit: r.a.info.unit.clone(),
                        a_rms: v.a.map(|p| round12(p.magnitude)),
                        b_rms: v.b.map(|p| round12(p.magnitude)),
                        ratio: v.ratio.map(round12),
                        a_angle: v.a.filter(|_| v.a_angle).map(|p| round12(p.angle)),
                        b_angle: v.b.filter(|_| v.b_angle).map(|p| round12(p.angle)),
                        angle_difference: v.difference.map(round12),
                    })
                    .collect(),
                differential: differential.as_ref().map(|(ca, cb, phases)| JsonDifferential {
                    a: ca.ids(),
                    b: cb.ids(),
                    inverted_b: self.invert_b,
                    phases: phases.as_ref().map(|ps| {
                        ps.iter()
                            .enumerate()
                            .map(|(p, (ia, ib, x))| JsonPhase {
                                phase: PHASES[p],
                                a: rms(ia.abs()),
                                b: rms(ib.abs()),
                                diff: rms(x.diff),
                                restraint: rms(x.restraint),
                                percent: x.ratio().map(|r| round12(r * 100.0)),
                            })
                            .collect()
                    }),
                }),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        let span = |w: &Window, c: &Comtrade| {
            let t = c.time();
            (t[w.start()], t[w.end], (t[w.end] - t[w.start()]).max(1e-3))
        };
        let (a0, a1, sa) = span(&wa, a);
        let cycles = if self.cycles == 1.0 { "cycle" } else { "cycles" };
        let mut window_text = format!(
            "{} .. {} in A ({} {cycles}, {} samples, {})",
            time(a0, sa),
            time(a1, sa),
            self.cycles,
            wa.samples,
            rate(frequency)
        );
        window_text += &match &wb {
            Some(w) => {
                let (b0, b1, sb) = span(w, b);
                format!("; {} .. {} in B ({} samples)", time(b0, sb), time(b1, sb), w.samples)
            }
            None => "; B has no samples there".to_owned(),
        };
        let mut head = s.head();
        head.extend([
            ("Window", window_text),
            (
                "Values",
                format!("{side_name}; RMS, angles relative to A {reference_name}"),
            ),
        ]);
        let dash = || "-".to_owned();
        let angle = |p: Option<Polar>, shown: bool| p.filter(|_| shown).map_or_else(dash, |p| degrees(p.angle));
        let table_rows: Vec<Vec<String>> = rows
            .iter()
            .zip(&values)
            .map(|(r, v)| {
                vec![
                    r.a.info.id.clone(),
                    r.b.info.id.clone(),
                    r.a.info.unit.clone(),
                    v.a.map_or_else(dash, |p| si(p.magnitude)),
                    v.b.map_or_else(dash, |p| si(p.magnitude)),
                    v.ratio.map_or_else(dash, |x| format!("{x:.3}")),
                    angle(v.a, v.a_angle),
                    angle(v.b, v.b_angle),
                    v.difference.map_or_else(dash, degrees),
                ]
            })
            .collect();
        let mut out = format!(
            "{}\n{}",
            key_values(&head),
            table(
                &["A", "B", "unit", "A", "B", "B/A", "angle A", "angle B", "B − A"],
                &table_rows
            )
        );
        if let Some((ca, cb, phases)) = &differential {
            out += &format!(
                "\nDifferential: A {} · B {}{}\n",
                ca.ids().join(" "),
                cb.ids().join(" "),
                if self.invert_b {
                    " (B inverted)"
                } else {
                    ", both measured into the line"
                }
            );
            match phases {
                Some(ps) => {
                    // Below 1e-6 of the largest current is float residue.
                    let floor = 1e-6 * ps.iter().map(|(_, _, x)| x.restraint).fold(0.0, f64::max);
                    let value = |x: f64| si(if x < floor { 0.0 } else { rms(x) });
                    let rows: Vec<Vec<String>> = ps
                        .iter()
                        .enumerate()
                        .map(|(p, (ia, ib, x))| {
                            vec![
                                PHASES[p].to_owned(),
                                si(rms(ia.abs())),
                                si(rms(ib.abs())),
                                value(x.diff),
                                si(rms(x.restraint)),
                                x.ratio().map_or_else(dash, |r| format!("{:.1} %", r * 100.0)),
                            ]
                        })
                        .collect();
                    out += &table(
                        &[
                            "phase",
                            "I A [A]",
                            "I B [A]",
                            "I diff [A]",
                            "I restraint [A]",
                            "diff / restraint",
                        ],
                        &rows,
                    );
                }
                None => out += "A current has no value in the window; no differential.\n",
            }
        }
        if wb.is_none() {
            out += "\nB has no samples at this window; choose another --at or check the alignment.\n";
        } else if rows.iter().any(|r| r.pa.is_none() || r.pb.is_none()) {
            out += "\nA value is missing in the window of a channel; its row has no values.\n";
        }
        print!("{out}");
        Ok(())
    }

    /// The pairs and the differential cycle by cycle (`--trend`).
    fn over_time(&self, s: &Setup) -> anyhow::Result<()> {
        let (a, frequency) = (s.a, s.frequency);
        let (window, range, step) = step_grid(a, self.from, self.to, self.step, self.cycles, frequency)?;
        let differential = if self.differential {
            s.differential_pairs(self.invert_b)
        } else {
            None
        };
        let mut all = s.pairs.clone();
        if let Some((d, _)) = &differential {
            all.extend(d.iter().copied());
        }
        let ab = Aligned { a, b: s.b, offset: s.d };
        let steps = compare::trend(ab, &all, range, window.samples, step, self.cycles, frequency, s.side);
        if steps.is_empty() {
            bail!("no window fits in the time range; choose another --from / --to");
        }
        let n = s.pairs.len();

        // Values of float residue mean nothing: a floor per pair.
        let floors: Vec<f64> = (0..n)
            .map(|i| {
                1e-6 * steps
                    .iter()
                    .flat_map(|st| [st.phasors[i].0, st.phasors[i].1])
                    .flatten()
                    .map(Complex::abs)
                    .fold(0.0, f64::max)
            })
            .collect();
        struct Cell {
            ratio: Option<f64>,
            difference: Option<f64>,
        }
        struct Phase {
            /// RMS in A.
            diff: Option<f64>,
            percent: Option<f64>,
        }
        let amps = differential.as_ref().map_or(1.0, |(_, f)| *f);
        // Per step the differential of each phase, `None` without values.
        let diffs: Vec<Option<[Option<Differential>; 3]>> = steps
            .iter()
            .map(|st| {
                let p = st.phasors.get(n..).filter(|p| p.len() == 3)?;
                Some([0, 1, 2].map(|i| match p[i] {
                    (Some(x), Some(y)) => Some(Differential::new(x.scale(amps), y.scale(amps))),
                    _ => None,
                }))
            })
            .collect();
        let diff_floor = 1e-6
            * diffs
                .iter()
                .flatten()
                .flatten()
                .flatten()
                .map(|x| x.restraint)
                .fold(0.0, f64::max);
        let rms = |x: f64| x / 2f64.sqrt();
        let rows: Vec<(f64, Vec<Cell>, Option<Vec<Phase>>)> = steps
            .iter()
            .zip(&diffs)
            .map(|(st, df)| {
                let cells = (0..n)
                    .map(|i| {
                        let floor = floors[i];
                        match st.phasors[i] {
                            (Some(x), Some(y)) if x.abs() > floor => Cell {
                                ratio: Some(y.abs() / x.abs()),
                                difference: (y.abs() > floor).then(|| Polar::new(y, x.arg()).angle),
                            },
                            _ => Cell {
                                ratio: None,
                                difference: None,
                            },
                        }
                    })
                    .collect();
                let phases = df.map(|ps| {
                    ps.iter()
                        .map(|x| Phase {
                            diff: x.map(|x| if x.diff < diff_floor { 0.0 } else { rms(x.diff) }),
                            percent: x
                                .filter(|x| x.restraint > diff_floor)
                                .map(|x| x.diff / x.restraint * 100.0),
                        })
                        .collect()
                });
                (st.time, cells, phases)
            })
            .collect();
        let currents = s.currents.as_ref().filter(|_| self.differential);
        let dt = self.cycles / frequency / window.samples as f64;

        match self.format {
            TableFormat::Json => {
                let (ja, jb) = s.json_recordings();
                let out = JsonTrend {
                    a: ja,
                    b: jb,
                    side: s.side.name(),
                    frequency,
                    alignment: s.json_alignment(),
                    cycles: self.cycles,
                    samples: window.samples,
                    step_s: round12(step as f64 * dt),
                    differential: currents.map(|(ca, cb)| JsonCurrents {
                        a: ca.ids(),
                        b: cb.ids(),
                        inverted_b: self.invert_b,
                    }),
                    steps: rows
                        .iter()
                        .map(|(t, cells, phases)| JsonStep {
                            time_s: round12(*t),
                            pairs: s
                                .pairs
                                .iter()
                                .zip(cells)
                                .map(|(p, c)| JsonTrendPair {
                                    a: p.a.info.id.clone(),
                                    b: p.b.info.id.clone(),
                                    ratio: c.ratio.map(round12),
                                    angle_difference: c.difference.map(round12),
                                })
                                .collect(),
                            differential: phases.as_ref().map(|ps| {
                                ps.iter()
                                    .enumerate()
                                    .map(|(i, x)| JsonTrendPhase {
                                        phase: PHASES[i],
                                        diff: x.diff.map(round12),
                                        percent: x.percent.map(round12),
                                    })
                                    .collect()
                            }),
                        })
                        .collect(),
                };
                println!("{}", serde_json::to_string_pretty(&out)?);
            }
            TableFormat::Csv => {
                let mut head = vec!["time_ms".to_owned()];
                for p in &s.pairs {
                    let id = csv_name(p.a.info.id.trim());
                    head.extend([format!("{id}_ratio"), format!("{id}_angle_difference_deg")]);
                }
                if currents.is_some() {
                    for ph in PHASES {
                        head.extend([format!("{ph}_diff_A"), format!("{ph}_diff_percent")]);
                    }
                }
                let mut out = head.join(",") + "\n";
                let v = |x: Option<f64>| x.map(|x| round12(x).to_string()).unwrap_or_default();
                for (t, cells, phases) in &rows {
                    let mut row = vec![round12(t * 1e3).to_string()];
                    for c in cells {
                        row.extend([v(c.ratio), v(c.difference)]);
                    }
                    for x in phases.iter().flatten() {
                        row.extend([v(x.diff), v(x.percent)]);
                    }
                    out += &(row.join(",") + "\n");
                }
                print!("{out}");
            }
            TableFormat::Text => {
                let mut headers = vec!["time".to_owned()];
                for p in &s.pairs {
                    let id = p.a.info.id.trim();
                    headers.extend([format!("{id} B/A"), format!("{id} B − A")]);
                }
                if currents.is_some() {
                    for ph in PHASES {
                        headers.extend([format!("{ph} diff [A]"), format!("{ph} %")]);
                    }
                }
                let span = (rows.last().map_or(0.0, |r| r.0) - rows[0].0).max(1e-3);
                let dash = || "-".to_owned();
                let table_rows: Vec<Vec<String>> = rows
                    .iter()
                    .map(|(t, cells, phases)| {
                        let mut r = vec![time(*t, span)];
                        for c in cells {
                            r.push(c.ratio.map_or_else(dash, |x| format!("{x:.3}")));
                            r.push(c.difference.map_or_else(dash, degrees));
                        }
                        for x in phases.iter().flatten() {
                            r.push(x.diff.map_or_else(dash, si));
                            r.push(x.percent.map_or_else(dash, |p| format!("{p:.1} %")));
                        }
                        r
                    })
                    .collect();
                let step_text = match self.step {
                    TrendStep::Cycles(c) => format!("{c} {}", if c == 1.0 { "cycle" } else { "cycles" }),
                    TrendStep::Seconds(x) => time(x, x),
                };
                let mut head = s.head();
                head.extend([
                    (
                        "Window",
                        format!(
                            "{} {} ({} samples in A) at {}, step {step_text}, {} rows",
                            self.cycles,
                            if self.cycles == 1.0 { "cycle" } else { "cycles" },
                            window.samples,
                            rate(frequency),
                            rows.len()
                        ),
                    ),
                    (
                        "Values",
                        if s.pairs.is_empty() {
                            s.side.name().to_owned()
                        } else {
                            format!("{}; B/A of the RMS, B − A of the angles", s.side.name())
                        },
                    ),
                ]);
                if let Some((ca, cb)) = currents {
                    head.push((
                        "Differential",
                        format!(
                            "A {} · B {}{}; RMS",
                            ca.ids().join(" "),
                            cb.ids().join(" "),
                            if self.invert_b {
                                " (B inverted)"
                            } else {
                                ", both measured into the line"
                            }
                        ),
                    ));
                }
                let names: Vec<&str> = headers.iter().map(String::as_str).collect();
                let mut out = format!("{}\n{}", key_values(&head), table(&names, &table_rows));
                let missing = rows.iter().any(|(_, c, p)| {
                    c.iter().any(|c| c.ratio.is_none()) || p.iter().flatten().any(|x| x.diff.is_none())
                });
                if missing {
                    out += "\n-: B has no samples there, a value is missing, or A is 0.\n";
                }
                print!("{out}");
            }
        }
        Ok(())
    }

    /// The waveforms of each pair, A above B (`--plot`).
    fn waveforms(&self, s: &Setup) -> anyhow::Result<()> {
        let (a, b, d, side) = (s.a, s.b, s.d, s.side);
        let (t0, t1) = time_window(a, self.from, self.to)?;
        let width = self.width.map(usize::from).or_else(term::terminal_width).unwrap_or(100);
        let cols = width.saturating_sub(LABEL_WIDTH).max(16);
        let rows = self.height.max(1) as usize;

        let mut head = s.head();
        head.extend([
            (
                "Window",
                format!("{} .. {} in A's time", time(t0, t1 - t0), time(t1, t1 - t0)),
            ),
            ("Values", s.side.name().to_owned()),
        ]);
        let mut out = key_values(&head);
        for p in &s.pairs {
            let seg_a =
                p.a.plot_segments(a.time(), a.index_range(t0, t1), (t0, t1), cols * 2, side);
            // B's currents turned with --invert-b, in A's unit.
            let scale = p.scale * if self.invert_b && is_current(p.b) { -1.0 } else { 1.0 };
            let seg_b: Vec<Vec<(f64, f64)>> =
                p.b.plot_segments(
                    b.time(),
                    b.index_range(t0 - d, t1 - d),
                    (t0 - d, t1 - d),
                    cols * 2,
                    side,
                )
                .into_iter()
                .map(|seg| seg.into_iter().map(|(t, v)| (t + d, v * scale)).collect())
                .collect();
            let y = chart::y_range(seg_a.iter().chain(&seg_b));
            let y = y.0.is_finite().then_some(y);
            let unit = p.a.info.unit.trim();
            out += &format!("\n{} (A) [{unit}]\n", p.a.info.id.trim());
            out += &chart::plot(&seg_a, (t0, t1), y, cols, rows);
            let mut label = format!("{} (B) [{unit}], shifted by {}", p.b.info.id.trim(), signed(d));
            if p.scale != 1.0 {
                label += &format!(", from {}", p.b.info.unit.trim());
            }
            if scale < 0.0 {
                label += ", inverted";
            }
            out += &format!("{label}\n");
            out += &chart::plot(&seg_b, (t0, t1), y, cols, rows);
            if self.difference {
                out += &difference_plot(s, p, scale, (t0, t1), cols, rows);
            }
        }
        print!("{out}");
        Ok(())
    }
}
