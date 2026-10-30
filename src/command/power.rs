//! `power` command: active, reactive and apparent power and power factor of
//! a three-phase circuit, at one moment or over time.

use anyhow::bail;
use serde::Serialize;

use super::{VoltageCurrent, describe_window, trend_steps, window_at};
use crate::analysis::Complex;
use crate::analysis::phasor;
use crate::analysis::power::{self, ThreePhase};
use crate::args::{PowerArgs, TableFormat};
use crate::comtrade::{Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, rate, si, table, time};

/// Phase names of the rows and columns.
const PHASES: [&str; 3] = ["A", "B", "C"];

/// Power of one phase or of the circuit in W, var and VA.
#[derive(Serialize)]
struct JsonPower {
    #[serde(skip_serializing_if = "Option::is_none")]
    phase: Option<&'static str>,
    /// Phase voltage in V (RMS).
    #[serde(skip_serializing_if = "Option::is_none")]
    voltage: Option<f64>,
    /// Phase current in A (RMS).
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<f64>,
    p: Option<f64>,
    q: Option<f64>,
    s: Option<f64>,
    pf: Option<f64>,
}

impl JsonPower {
    fn new(phase: Option<&'static str>, s: Option<Complex>, vi: Option<(Complex, Complex)>) -> Self {
        let rms = |c: Complex| round12(c.abs() / 2f64.sqrt());
        Self {
            phase,
            voltage: vi.map(|(v, _)| rms(v)),
            current: vi.map(|(_, i)| rms(i)),
            p: s.map(|s| round12(s.re)),
            q: s.map(|s| round12(s.im)),
            s: s.map(|s| round12(s.abs())),
            pf: s.and_then(power::power_factor).map(round12),
        }
    }
}

/// Phases A, B, C, then the total.
fn json_rows(values: Option<&Values>, with_vi: bool) -> (Vec<JsonPower>, JsonPower) {
    let phases = PHASES
        .iter()
        .enumerate()
        .map(|(k, &name)| {
            let vi = values.filter(|_| with_vi).map(|x| (x.v[k], x.i[k]));
            JsonPower::new(Some(name), values.map(|x| x.power.phases[k]), vi)
        })
        .collect();
    (phases, JsonPower::new(None, values.map(|x| x.power.total), None))
}

#[derive(Serialize)]
struct JsonStep {
    time_s: f64,
    phases: Vec<JsonPower>,
    total: JsonPower,
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    samples: usize,
    voltage: [String; 3],
    current: [String; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    from_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    at_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    phases: Option<Vec<JsonPower>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<JsonPower>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    steps: Vec<JsonStep>,
}

/// Phase voltages and currents (peak phasors in V and A) and their power.
struct Values {
    v: [Complex; 3],
    i: [Complex; 3],
    power: ThreePhase,
}

/// Power from the six phasors of `vc`'s channels; `None` if one is missing.
fn values(vc: &VoltageCurrent, phasors: &[Option<Complex>]) -> Option<Values> {
    let (v, i) = vc.scaled(phasors)?;
    Some(Values {
        v,
        i,
        power: power::three_phase(v, i),
    })
}

/// Power factor with `lag` (inductive) or `lead` (capacitive).
fn pf_text(s: Complex) -> String {
    match power::power_factor(s) {
        None => "-".into(),
        Some(pf) if s.im > 0.0 => format!("{pf:.3} lag"),
        Some(pf) if s.im < 0.0 => format!("{pf:.3} lead"),
        Some(pf) => format!("{pf:.3}"),
    }
}

/// Direction of the active power: `forward` in the direction the currents
/// are measured, `reverse` against it.
fn flow(total: Complex) -> &'static str {
    if total.re > 0.0 {
        "forward"
    } else if total.re < 0.0 {
        "reverse"
    } else {
        "none"
    }
}

impl PowerArgs {
    /// Runs the `power` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        };
        let vc = VoltageCurrent::new(&c, &self.voltage, &self.current)?;
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }

        let (voltage, current) = vc.ids();
        let mut json = Json {
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
            samples: 0,
            voltage,
            current,
            from_s: None,
            at_s: None,
            phases: None,
            total: None,
            flow: None,
            steps: Vec::new(),
        };
        let header = |window: &str| {
            key_values(&[
                ("File", json.file.clone()),
                ("Window", window.to_owned()),
                (
                    "Channels",
                    format!(
                        "{} [{}] · {} [{}]",
                        json.voltage.join(" "),
                        vc.pair.voltage.unit,
                        json.current.join(" "),
                        vc.pair.current.unit
                    ),
                ),
                ("Values", format!("{} W, var, VA", json.side)),
                (
                    "Convention",
                    "P > 0 flows as the currents are measured, Q > 0 inductive".to_owned(),
                ),
            ])
        };

        if self.trend {
            let (window, result) = trend_steps(
                &c,
                &vc.channels,
                self.from,
                self.to,
                self.step,
                self.cycles,
                frequency,
                side,
            )?;
            let rows: Vec<(f64, Option<Values>)> =
                result.steps.iter().map(|s| (s.time, values(&vc, &s.phasors))).collect();
            json.samples = window.samples;
            match self.format {
                TableFormat::Json => {
                    json.steps = rows
                        .iter()
                        .map(|(t, x)| {
                            let (phases, total) = json_rows(x.as_ref(), false);
                            JsonStep {
                                time_s: round12(*t),
                                phases,
                                total,
                            }
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                TableFormat::Csv => print!("{}", trend_csv(&rows)),
                TableFormat::Text => {
                    print!(
                        "{}",
                        header(&format!(
                            "{} {} ({} samples) at {}, {} rows",
                            self.cycles,
                            if self.cycles == 1.0 { "cycle" } else { "cycles" },
                            window.samples,
                            rate(frequency),
                            rows.len()
                        ))
                    );
                    print!("\n{}", trend_text(&rows, self.phases));
                }
            }
            return Ok(());
        }

        let window = window_at(&c, self.at, self.cycles, frequency)?;
        let phasors: Vec<Option<Complex>> = vc
            .channels
            .iter()
            .map(|ch| phasor::phasor(ch, c.time(), &window, side).ok())
            .collect();
        let x = values(&vc, &phasors);
        let (t0, t1) = (c.time()[window.start()], c.time()[window.end]);
        json.samples = window.samples;
        match self.format {
            TableFormat::Json => {
                let (phases, total) = json_rows(x.as_ref(), true);
                json.from_s = Some(round12(t0));
                json.at_s = Some(round12(t1));
                json.phases = Some(phases);
                json.total = Some(total);
                json.flow = x.as_ref().map(|x| flow(x.power.total));
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            TableFormat::Csv => {
                let (phases, total) = json_rows(x.as_ref(), true);
                let f = |v: Option<f64>| v.map_or_else(String::new, |v| v.to_string());
                let mut out = String::from("phase,voltage_v,current_a,p_w,q_var,s_va,pf\n");
                for j in phases.iter().chain([&total]) {
                    out += &format!(
                        "{},{},{},{},{},{},{}\n",
                        j.phase.unwrap_or("total"),
                        f(j.voltage),
                        f(j.current),
                        f(j.p),
                        f(j.q),
                        f(j.s),
                        f(j.pf)
                    );
                }
                print!("{out}");
            }
            TableFormat::Text => {
                print!("{}", header(&describe_window(&window, t0, t1, self.cycles, frequency)));
                let Some(x) = x else {
                    println!("\nNo power: a sample in the window has no value.");
                    return Ok(());
                };
                let rms = |c: Complex, factor: f64| si(c.abs() / 2f64.sqrt() / factor);
                let row = |name: &str, s: Complex, vi: Option<(String, String)>| {
                    let (v, i) = vi.unwrap_or_default();
                    vec![name.to_owned(), v, i, si(s.re), si(s.im), si(s.abs()), pf_text(s)]
                };
                let mut rows: Vec<Vec<String>> = (0..3)
                    .map(|k| {
                        // V and I in the channels' units, as in the plots.
                        let vi = (rms(x.v[k], vc.voltage_factor), rms(x.i[k], vc.current_factor));
                        row(PHASES[k], x.power.phases[k], Some(vi))
                    })
                    .collect();
                rows.push(row("total", x.power.total, None));
                let v = format!("V [{}]", vc.pair.voltage.unit);
                let i = format!("I [{}]", vc.pair.current.unit);
                print!(
                    "\n{}",
                    table(&["phase", &v, &i, "P [W]", "Q [var]", "S [VA]", "PF"], &rows)
                );
                let total = x.power.total;
                println!(
                    "\nFlow: {}",
                    match flow(total) {
                        "forward" => format!("forward, {} W as the currents are measured", si(total.re)),
                        "reverse" => format!("reverse, {} W against the current direction", si(-total.re)),
                        _ => "no active power".to_owned(),
                    }
                );
            }
        }
        Ok(())
    }
}

/// CSV of `--trend`: P and Q per phase, then the totals and the power factor.
fn trend_csv(rows: &[(f64, Option<Values>)]) -> String {
    let mut out = String::from("time_ms");
    for p in PHASES {
        out += &format!(",{p}_p_w,{p}_q_var");
    }
    out += ",p_w,q_var,s_va,pf\n";
    for (t, x) in rows {
        out += &round12(t * 1e3).to_string();
        match x {
            Some(x) => {
                for s in x.power.phases {
                    out += &format!(",{},{}", round12(s.re), round12(s.im));
                }
                let s = x.power.total;
                let pf = power::power_factor(s).map_or_else(String::new, |pf| round12(pf).to_string());
                out += &format!(",{},{},{},{pf}", round12(s.re), round12(s.im), round12(s.abs()));
            }
            None => out += &",".repeat(10),
        }
        out.push('\n');
    }
    out
}

/// Text table of `--trend`: the totals, with `phases` P and Q per phase first.
fn trend_text(rows: &[(f64, Option<Values>)], phases: bool) -> String {
    let mut headers = vec!["time".to_owned()];
    if phases {
        for p in PHASES {
            headers.push(format!("{p} P [W]"));
            headers.push(format!("{p} Q [var]"));
        }
    }
    headers.extend(["P [W]", "Q [var]", "S [VA]", "PF"].map(String::from));
    let span = (rows.last().map_or(0.0, |r| r.0) - rows[0].0).max(1e-3);
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|(t, x)| {
            let mut r = vec![time(*t, span)];
            match x {
                Some(x) => {
                    if phases {
                        for s in x.power.phases {
                            r.extend([si(s.re), si(s.im)]);
                        }
                    }
                    let s = x.power.total;
                    r.extend([si(s.re), si(s.im), si(s.abs()), pf_text(s)]);
                }
                None => r.resize(headers.len(), "-".to_owned()),
            }
            r
        })
        .collect();
    let names: Vec<&str> = headers.iter().map(String::as_str).collect();
    table(&names, &table_rows)
}
