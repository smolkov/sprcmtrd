//! `compare` command: two recordings of the same event side by side, with
//! the phasors of paired channels and the differential current of two line
//! ends.

use anyhow::{Context as _, anyhow, bail};
use serde::Serialize;

use super::{factor, group, select_channels, window_at};
use crate::analysis::compare::{self, Align, Differential};
use crate::analysis::fault::PHASES;
use crate::analysis::impedance::{Quantity, find_current, unit_factor};
use crate::analysis::phasor::{self, Polar, Window};
use crate::analysis::{Complex, sequence::Group};
use crate::args::{AlignArg, CompareArgs};
use crate::comtrade::{AnalogChannel, AnalogInfo, Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, rate, si, table, time};

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

/// The window of B that ends at A's window end `t` (A's time), `None` when B
/// has no samples there.
fn window_b(b: &Comtrade, t_b: f64, cycles: f64, frequency: f64) -> Option<Window> {
    let (b0, b1) = b.time_span();
    let n = b.len();
    let dt = if n > 1 { (b1 - b0) / (n - 1) as f64 } else { 0.0 };
    if t_b < b0 - dt / 2.0 || t_b > b1 + dt / 2.0 {
        return None;
    }
    Window::ending_at(b, t_b, cycles, frequency).ok()
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

fn file_name(c: &Comtrade) -> String {
    c.path
        .as_ref()
        .map(|p| p.main_file().display().to_string())
        .unwrap_or_default()
}

/// One compared pair: phasors (peak) in A's unit, B's in A's time frame.
struct Row<'a> {
    a: &'a AnalogChannel,
    b: &'a AnalogChannel,
    pa: Option<Complex>,
    pb: Option<Complex>,
}

impl CompareArgs {
    /// Runs the `compare` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let a = Comtrade::open(&self.a)?;
        let b = Comtrade::open(&self.b)?;
        let side = if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        };
        let frequency = match self.frequency {
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
        if self.cycles <= 0.0 {
            bail!("--cycles must be above 0");
        }

        // Channel pairs, limited to --channels of A.
        let ids = |c: &Comtrade| c.analog_channels().map(|ch| ch.info.id.clone()).collect::<Vec<_>>();
        let (ids_a, ids_b) = (ids(&a), ids(&b));
        let mut pairs = compare::pairs(&str_refs(&ids_a), &str_refs(&ids_b), &self.pair).map_err(|e| anyhow!(e))?;
        if !self.channels.is_empty() {
            let (only, binary) = select_channels(&a, &self.channels)?;
            if !binary.is_empty() {
                bail!("--channels takes analog channels of A");
            }
            pairs.retain(|(pa, _)| only.contains(pa));
            if pairs.is_empty() {
                bail!("none of --channels has a channel with the same id in B; pair them with --pair");
            }
        }
        let scales: Vec<f64> = pairs
            .iter()
            .map(|&(pa, pb)| {
                let (ca, cb) = (analog(&a, pa), analog(&b, pb));
                unit_scale(&ca.info.unit, &cb.info.unit).with_context(|| {
                    format!(
                        "{} has the unit '{}' in A but {} has '{}' in B",
                        ca.info.id, ca.info.unit, cb.info.id, cb.info.unit
                    )
                })
            })
            .collect::<anyhow::Result<_>>()?;

        // Current groups: for the differential and for the fault alignment.
        let align = match self.align {
            AlignArg::Time => Align::Time,
            AlignArg::Trigger => Align::Trigger,
            AlignArg::Fault => Align::Fault,
        };
        let need_currents = self.differential || align == Align::Fault;
        let currents = if need_currents {
            Some((
                Currents::new(&a, &self.current_a, "--current-a", "A")?,
                Currents::new(&b, &self.current_b, "--current-b", "B")?,
            ))
        } else {
            None
        };
        let fault_offset = match (&currents, align) {
            (Some((ca, cb)), Align::Fault) => Some(
                compare::fault_offset((&a, ca.channels), (&b, cb.channels), frequency, side).map_err(|e| anyhow!(e))?,
            ),
            _ => None,
        };
        let shift = self.shift / 1e3;
        let d = compare::offset(&a, &b, align, fault_offset, shift).map_err(|e| anyhow!(e))?;

        // Windows ending at the same instant.
        let wa = window_at(&a, self.at, self.cycles, frequency)?;
        let t_end = a.time()[wa.end];
        let wb = window_b(&b, t_end - d, self.cycles, frequency);
        let at_a = |ch: &AnalogChannel| phasor::phasor(ch, a.time(), &wa, side).ok();
        let at_b = |ch: &AnalogChannel| {
            wb.as_ref()
                .and_then(|w| phasor::phasor(ch, b.time(), w, side).ok())
                .map(|p| compare::rotate(p, d, frequency))
        };
        let rows: Vec<Row> = pairs
            .iter()
            .zip(&scales)
            .map(|(&(pa, pb), &k)| {
                let (ca, cb) = (analog(&a, pa), analog(&b, pb));
                Row {
                    a: ca,
                    b: cb,
                    pa: at_a(ca),
                    pb: at_b(cb).map(|p| p.scale(k)),
                }
            })
            .collect();

        // The angle reference: a channel of A, else the first pair's A.
        let (reference_name, reference) = match &self.reference {
            Some(r) => {
                let (found, _) = select_channels(&a, std::slice::from_ref(r))?;
                let Some(&i) = found.first() else {
                    bail!("the reference {r} is not an analog channel of A");
                };
                let ch = analog(&a, i);
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
            let (ca, cb) = currents.as_ref().expect("current groups are found for --differential");
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

        let side_name = match side {
            Side::Primary => "primary",
            Side::Secondary => "secondary",
        };
        let rms = |x: f64| round12(x / 2f64.sqrt());
        if self.json {
            let json_window = |c: &Comtrade, w: &Window| JsonWindow {
                from_s: round12(c.time()[w.start()]),
                to_s: round12(c.time()[w.end]),
                samples: w.samples,
            };
            let out = Json {
                a: JsonRecording {
                    file: file_name(&a),
                    station: a.config.station_name.clone(),
                },
                b: JsonRecording {
                    file: file_name(&b),
                    station: b.config.station_name.clone(),
                },
                side: side_name,
                frequency,
                alignment: JsonAlignment {
                    mode: align.name(),
                    offset_s: round12(d),
                    shift_s: round12(shift),
                },
                window_a: json_window(&a, &wa),
                window_b: wb.as_ref().map(|w| json_window(&b, w)),
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
        let (a0, a1, sa) = span(&wa, &a);
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
                let (b0, b1, sb) = span(w, &b);
                format!("; {} .. {} in B ({} samples)", time(b0, sb), time(b1, sb), w.samples)
            }
            None => "; B has no samples there".to_owned(),
        };
        let signed = |t: f64| {
            let s = time(t, t.abs().max(0.01));
            if t > 0.0 { format!("+{s}") } else { s }
        };
        let mode = match align {
            Align::Time => "absolute time",
            Align::Trigger => "triggers",
            Align::Fault => "fault inception",
        };
        let mut alignment = format!("{mode}: B's trigger at {} in A's time", signed(d));
        if shift != 0.0 {
            alignment += &format!(" (with --shift {})", signed(shift));
        }
        let recording = |c: &Comtrade| {
            let station = c.config.station_name.trim();
            if station.is_empty() {
                file_name(c)
            } else {
                format!("{} ({station})", file_name(c))
            }
        };
        let head = vec![
            ("A", recording(&a)),
            ("B", recording(&b)),
            ("Alignment", alignment),
            ("Window", window_text),
            (
                "Values",
                format!("{side_name}; RMS, angles relative to A {reference_name}"),
            ),
        ];
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
}
