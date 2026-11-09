//! `fault` command: fault summary of a recording (inception, faulted phases,
//! duration, peak currents, operate times).

use anyhow::{Context as _, bail};
use serde::Serialize;

use super::{factor, group, select_channels, time_window};
use crate::analysis::fault::{self, Fault, Operate, PHASES, Role};
use crate::analysis::impedance::{Quantity, find_current, find_voltage};
use crate::analysis::phasor;
use crate::analysis::sequence::Group;
use crate::analysis::{Complex, Window};
use crate::args::FaultArgs;
use crate::comtrade::{AnalogChannel, AnalogInfo, BinaryChannel, Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, si, table, time as format_time};

/// RMS of a peak phasor.
fn rms(p: Complex) -> f64 {
    round12(p.abs() / 2f64.sqrt())
}

#[derive(Serialize)]
struct JsonTime {
    /// 1-based sample.
    sample: usize,
    time_s: f64,
    absolute: String,
}

#[derive(Serialize)]
struct JsonPhase {
    phase: &'static str,
    faulted: bool,
    /// RMS of the fundamental in A before the fault.
    pre_fault: Option<f64>,
    /// RMS of the fundamental in A during the fault.
    fault: Option<f64>,
    /// RMS of the superimposed current ΔI in A.
    delta: Option<f64>,
    /// Instantaneous peak in A.
    peak: Option<f64>,
    peak_time_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    voltage_pre_fault: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    voltage_fault: Option<f64>,
}

#[derive(Serialize)]
struct JsonResidual {
    pre_fault: Option<f64>,
    fault: Option<f64>,
    delta: Option<f64>,
}

#[derive(Serialize)]
struct JsonOperate {
    role: &'static str,
    time_s: f64,
    since_inception_s: f64,
    index: usize,
    id: String,
}

#[derive(Serialize)]
struct JsonFault {
    inception: JsonTime,
    fault_type: String,
    /// No current before the inception (line energized).
    energized: bool,
    phases: Vec<JsonPhase>,
    residual: JsonResidual,
    interruption: Option<JsonTime>,
    duration_s: Option<f64>,
    operate: Vec<JsonOperate>,
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    current: [String; 3],
    voltage: Option<[String; 3]>,
    from_s: f64,
    to_s: f64,
    found: bool,
    #[serde(flatten)]
    fault: Option<JsonFault>,
}

/// Three channels of a group with the factor from their unit to V or A.
pub(super) struct Phases<'a> {
    pub(super) channels: [&'a AnalogChannel; 3],
    pub(super) unit: String,
    pub(super) factor: f64,
}

impl<'a> Phases<'a> {
    fn new(c: &'a Comtrade, g: &Group, q: Quantity) -> anyhow::Result<Self> {
        Ok(Self {
            channels: g.phases.map(|i| c.analog(i).expect("channel position is valid")),
            unit: g.unit.clone(),
            factor: factor(g, q)?,
        })
    }

    pub(super) fn ids(&self) -> [String; 3] {
        self.channels.map(|ch| ch.info.id.clone())
    }

    /// RMS phasors in V or A over `window`.
    fn rms(&self, time: &[f64], window: &Window, side: Side) -> [Option<f64>; 3] {
        self.channels.map(|ch| {
            phasor::phasor(ch, time, window, side)
                .ok()
                .map(|p| rms(p) * self.factor)
        })
    }
}

/// The current group from `--current`, else found by unit.
pub(super) fn currents<'a>(c: &'a Comtrade, tokens: &[String]) -> anyhow::Result<Phases<'a>> {
    let g = if tokens.is_empty() {
        let infos: Vec<&AnalogInfo> = c.analog_channels().map(|ch| &ch.info).collect();
        find_current(&infos).context("no current A/B/C channel group found; give it with --current")?
    } else {
        group(c, tokens, "--current")?
    };
    Phases::new(c, &g, Quantity::Current)
}

/// The voltage group from `--voltage`, else found by unit (none is fine).
fn voltages<'a>(c: &'a Comtrade, tokens: &[String], circuit: &str) -> anyhow::Result<Option<Phases<'a>>> {
    let g = if tokens.is_empty() {
        let infos: Vec<&AnalogInfo> = c.analog_channels().map(|ch| &ch.info).collect();
        find_voltage(&infos, circuit)
    } else {
        Some(group(c, tokens, "--voltage")?)
    };
    g.map(|g| Phases::new(c, &g, Quantity::Voltage)).transpose()
}

/// The binary channels a role is looked for in: the one of the option, else
/// all whose id matches the role.
fn role_channels<'a>(
    c: &'a Comtrade,
    role: Role,
    token: Option<&String>,
    option: &str,
) -> anyhow::Result<Vec<&'a BinaryChannel>> {
    let Some(token) = token else {
        return Ok(c.binary_channels().filter(|b| role.matches(&b.info.id)).collect());
    };
    let (analog, binary) = select_channels(c, std::slice::from_ref(token))?;
    if !analog.is_empty() || binary.len() != 1 {
        bail!("{option} needs a binary channel (dN or id)");
    }
    Ok(vec![c.binary(binary[0]).expect("selected index is valid")])
}

impl FaultArgs {
    /// Runs the `fault` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = ctx.global.side();
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        let current = currents(&c, &self.current)?;
        let circuit = current.channels[0].info.circuit.clone();
        let voltage = voltages(&c, &self.voltage, &circuit)?;
        let roles = [
            (
                Role::Start,
                role_channels(&c, Role::Start, self.start.as_ref(), "--start")?,
                "--start",
            ),
            (
                Role::Trip,
                role_channels(&c, Role::Trip, self.trip.as_ref(), "--trip")?,
                "--trip",
            ),
            (
                Role::BreakerOpen,
                role_channels(&c, Role::BreakerOpen, self.breaker.as_ref(), "--breaker")?,
                "--breaker",
            ),
        ];

        let (t0, t1) = time_window(&c, self.from, self.to)?;
        let range = c.index_range(t0, t1);
        let time = c.time();
        let found = fault::analyse(current.channels, time, range.clone(), frequency, side);
        let operates: Vec<Operate> = found
            .as_ref()
            .map(|f| {
                let from = f.inception.saturating_sub(f.per_cycle).max(range.start);
                roles
                    .iter()
                    .filter_map(|(role, chs, _)| fault::operate(*role, chs, time, from..range.end))
                    .collect()
            })
            .unwrap_or_default();

        let file = c.name();
        let absolute = |t: f64| c.absolute_time(t).format("%Y-%m-%d %H:%M:%S%.6f").to_string();
        let side_name = side.name();

        if self.json {
            let json_time = |i: usize| JsonTime {
                sample: i + 1,
                time_s: round12(time[i]),
                absolute: absolute(time[i]),
            };
            let fault = found.as_ref().map(|f| {
                let (vpre, vfault) = voltage
                    .as_ref()
                    .map(|v| (v.rms(time, &f.pre_window, side), v.rms(time, &f.fault_window, side)))
                    .unwrap_or(([None; 3], [None; 3]));
                let k = current.factor;
                let delta = f.delta();
                let phases = (0..3)
                    .map(|p| JsonPhase {
                        phase: PHASES[p],
                        faulted: f.kind.phases[p],
                        pre_fault: f.pre[p].map(|x| rms(x) * k),
                        fault: f.fault[p].map(|x| rms(x) * k),
                        delta: delta[p].map(|x| rms(x) * k),
                        peak: f.peaks[p].map(|x| round12(x.value * k)),
                        peak_time_s: f.peaks[p].map(|x| round12(time[x.sample])),
                        voltage_pre_fault: vpre[p],
                        voltage_fault: vfault[p],
                    })
                    .collect();
                let residual = |ps: [Option<Complex>; 3]| Some(rms(ps[0]? + ps[1]? + ps[2]?) * k);
                let inception = time[f.inception];
                JsonFault {
                    inception: json_time(f.inception),
                    fault_type: f.kind.name(),
                    energized: f.energized(),
                    phases,
                    residual: JsonResidual {
                        pre_fault: residual(f.pre),
                        fault: residual(f.fault),
                        delta: residual(delta),
                    },
                    interruption: f.interruption.map(json_time),
                    duration_s: f.interruption.map(|i| round12(time[i] - inception)),
                    operate: operates
                        .iter()
                        .map(|o| JsonOperate {
                            role: o.role.name(),
                            time_s: round12(time[o.sample]),
                            since_inception_s: round12(time[o.sample] - inception),
                            index: o.channel.info.index,
                            id: o.channel.info.id.clone(),
                        })
                        .collect(),
                }
            });
            let out = Json {
                file,
                side: side_name,
                frequency,
                current: current.ids(),
                voltage: voltage.as_ref().map(Phases::ids),
                from_s: t0,
                to_s: t1,
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
        ];
        if let Some(v) = &voltage {
            head.push(("Voltages", format!("{} [{}]", v.ids().join(" "), v.unit)));
        }
        head.push((
            "Values",
            format!("{side_name}, RMS of the fundamental at {frequency} Hz"),
        ));
        let Some(f) = found else {
            head.push(("Searched", format!("{} .. {}", signed(t0), signed(t1))));
            print!("{}\nNo fault found.\n", key_values(&head));
            return Ok(());
        };
        let inception = time[f.inception];
        head.push((
            "Inception",
            format!(
                "{} ({}), sample {}",
                signed(inception),
                absolute(inception),
                f.inception + 1
            ),
        ));
        head.push(("Fault", f.kind.name()));
        head.push((
            "Duration",
            match f.interruption {
                Some(i) => format!(
                    "{}, current interrupted at {}",
                    format_time(time[i] - inception, span),
                    signed(time[i])
                ),
                None => "current not interrupted within the recording".to_owned(),
            },
        ));
        let mut out = key_values(&head);
        if f.energized() {
            out += "\nNo current flowed before the inception: the line was energized, onto a fault or onto load.\n";
        }
        out += &format!("\n{}", phase_table(&f, &current, voltage.as_ref(), time, side, &signed));
        out += &format!(
            "\nOperate times from inception\n{}",
            operate_table(&f, &operates, time, span, &signed)
        );
        let missing: Vec<String> = roles
            .iter()
            .filter(|(role, _, _)| !operates.iter().any(|o| o.role == *role))
            .map(|(role, chs, option)| {
                if chs.is_empty() {
                    format!("No {} channel found; give it with {option}.", role.name())
                } else {
                    format!("No {} after the inception.", role.name())
                }
            })
            .collect();
        if !missing.is_empty() {
            out += &format!("\n{}\n", missing.join("\n"));
        }
        print!("{out}");
        Ok(())
    }
}

/// Table of the phases (and 3I0): pre-fault, fault, ΔI, peak, and the
/// voltages when there are any.
fn phase_table(
    f: &Fault,
    current: &Phases,
    voltage: Option<&Phases>,
    time: &[f64],
    side: Side,
    signed: &dyn Fn(f64) -> String,
) -> String {
    let k = current.factor;
    // Below 1e-6 of the largest current is the residue of integer samples.
    let largest = f
        .fault
        .iter()
        .chain(&f.pre)
        .flatten()
        .map(|p| rms(*p))
        .fold(0.0, f64::max);
    let value = |p: Option<Complex>| {
        p.map_or_else(
            || "-".to_owned(),
            |p| {
                let v = rms(p);
                si(if v < 1e-6 * largest { 0.0 } else { v * k })
            },
        )
    };
    let delta = f.delta();
    let mut headers = vec![
        "phase",
        "",
        "pre-fault [A]",
        "fault [A]",
        "ΔI [A]",
        "peak [A]",
        "peak at",
    ];
    let volts = voltage.map(|v| (v.rms(time, &f.pre_window, side), v.rms(time, &f.fault_window, side)));
    if volts.is_some() {
        headers.extend(["pre-fault [V]", "fault [V]", "drop"]);
    }
    let mut rows: Vec<Vec<String>> = (0..3)
        .map(|p| {
            let mut row = vec![
                PHASES[p].to_owned(),
                if f.kind.phases[p] { "faulted" } else { "" }.to_owned(),
                value(f.pre[p]),
                value(f.fault[p]),
                value(delta[p]),
                f.peaks[p].map_or_else(|| "-".to_owned(), |x| si(x.value * k)),
                f.peaks[p].map_or_else(|| "-".to_owned(), |x| signed(time[x.sample])),
            ];
            if let Some((pre, during)) = &volts {
                let text = |v: Option<f64>| v.map_or_else(|| "-".to_owned(), si);
                let drop = match (pre[p], during[p]) {
                    (Some(a), Some(b)) if a > 0.0 => format!("{:.1} %", (1.0 - b / a) * 100.0),
                    _ => "-".to_owned(),
                };
                row.extend([text(pre[p]), text(during[p]), drop]);
            }
            row
        })
        .collect();
    let residual = |ps: [Option<Complex>; 3]| Some(ps[0]? + ps[1]? + ps[2]?);
    let mut row = vec![
        "3I0".to_owned(),
        if f.kind.ground { "ground" } else { "" }.to_owned(),
        value(residual(f.pre)),
        value(residual(f.fault)),
        value(residual(delta)),
        String::new(),
        String::new(),
    ];
    if volts.is_some() {
        row.extend([String::new(), String::new(), String::new()]);
    }
    rows.push(row);
    table(&headers, &rows)
}

/// Table of the fault sequence: inception, start, trip, breaker open and
/// interruption in time order.
fn operate_table(f: &Fault, operates: &[Operate], time: &[f64], span: f64, signed: &dyn Fn(f64) -> String) -> String {
    let inception = time[f.inception];
    let mut steps: Vec<(usize, String, String)> = vec![(f.inception, "inception".to_owned(), String::new())];
    steps.extend(operates.iter().map(|o| {
        (
            o.sample,
            o.role.name().to_owned(),
            format!("D{} {}", o.channel.info.index, o.channel.info.id),
        )
    }));
    if let Some(i) = f.interruption {
        steps.push((i, "current interrupted".to_owned(), String::new()));
    }
    steps.sort_by_key(|(sample, _, _)| *sample);
    let rows: Vec<Vec<String>> = steps
        .into_iter()
        .map(|(sample, name, channel)| {
            let since = time[sample] - inception;
            let since = if since >= 0.0 {
                format!("+{}", format_time(since, span))
            } else {
                format_time(since, span)
            };
            vec![name, since, signed(time[sample]), channel]
        })
        .collect();
    table(&["event", "since inception", "time", "channel"], &rows)
}
