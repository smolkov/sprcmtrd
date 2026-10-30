//! `impedance` command: apparent impedance of the fault loops, at one moment
//! or over time.

use anyhow::{Context as _, bail};
use serde::Serialize;

use super::{VoltageCurrent, describe_window, trend_steps, window_at};
use crate::analysis::Complex;
use crate::analysis::impedance::{self, Loop};
use crate::analysis::phasor;
use crate::args::{ImpedanceArgs, TableFormat};
use crate::comtrade::{Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, rate, si, table, time};

/// Loop voltage (V), loop current (A) and impedance (Ω) of one loop.
type LoopValue = (Complex, Complex, Complex);

/// The voltage and current channels with the loops to measure.
struct Setup<'a> {
    vc: VoltageCurrent<'a>,
    k0: Complex,
    loops: Vec<Loop>,
}

impl Setup<'_> {
    /// Impedance in Ω of each loop from the six phasors (`None` where a
    /// phasor or the loop current is missing).
    fn impedances(&self, phasors: &[Option<Complex>]) -> Vec<Option<LoopValue>> {
        let scaled = self.vc.scaled(phasors);
        self.loops
            .iter()
            .map(|&l| {
                let (v, i) = scaled?;
                let (lv, li) = impedance::loop_quantities(l, v, i, self.k0);
                Some((lv, li, clean(impedance::impedance(lv, li)?)))
            })
            .collect()
    }
}

#[derive(Serialize)]
struct JsonLoop {
    #[serde(rename = "loop")]
    name: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    r: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    magnitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    angle: Option<f64>,
    /// Loop voltage in V (RMS).
    #[serde(skip_serializing_if = "Option::is_none")]
    voltage: Option<f64>,
    /// Loop current in A (RMS).
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<f64>,
}

impl JsonLoop {
    fn new(l: Loop, value: Option<LoopValue>) -> Self {
        let rms = |c: Complex| round12(c.abs() / 2f64.sqrt());
        Self {
            name: l.name(),
            r: value.map(|(_, _, z)| round12(z.re)),
            x: value.map(|(_, _, z)| round12(z.im)),
            magnitude: value.map(|(_, _, z)| round12(z.abs())),
            angle: value.map(|(_, _, z)| round12(z.arg().to_degrees())),
            voltage: value.map(|(v, _, _)| rms(v)),
            current: value.map(|(_, i, _)| rms(i)),
        }
    }
}

#[derive(Serialize)]
struct JsonStep {
    time_s: f64,
    loops: Vec<JsonLoop>,
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    samples: usize,
    k0_magnitude: f64,
    k0_angle: f64,
    voltage: [String; 3],
    current: [String; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    from_s: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    at_s: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    loops: Vec<JsonLoop>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    steps: Vec<JsonStep>,
}

impl ImpedanceArgs {
    /// Runs the `impedance` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        };
        let vc = VoltageCurrent::new(&c, &self.voltage, &self.current)?;
        let loops = if self.loops.is_empty() {
            Loop::ALL.to_vec()
        } else {
            self.loops
                .iter()
                .map(|s| Loop::parse(s).with_context(|| format!("unknown loop '{s}' (AN, BN, CN, AB, BC, CA)")))
                .collect::<anyhow::Result<_>>()?
        };
        let (voltage_ids, current_ids) = vc.ids();
        let pair = vc.pair.clone();
        let setup = Setup {
            vc,
            k0: Complex::from_polar(self.k0.magnitude, self.k0.angle.to_radians()),
            loops,
        };
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }

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
            k0_magnitude: self.k0.magnitude,
            k0_angle: self.k0.angle,
            voltage: voltage_ids,
            current: current_ids,
            from_s: None,
            at_s: None,
            loops: Vec::new(),
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
                        pair.voltage.unit,
                        json.current.join(" "),
                        pair.current.unit
                    ),
                ),
                (
                    "Compensation",
                    if self.k0.magnitude == 0.0 {
                        "none (k0 = 0; give the line's k0 with --k0)".to_owned()
                    } else {
                        format!("k0 = {} ∠ {:.1}°", self.k0.magnitude, self.k0.angle)
                    },
                ),
                ("Values", format!("{} Ω", json.side)),
            ])
        };

        if self.trend {
            let (window, result) = trend_steps(
                &c,
                &setup.vc.channels,
                self.from,
                self.to,
                self.step,
                self.cycles,
                frequency,
                side,
            )?;
            let rows: Vec<(f64, Vec<Option<LoopValue>>)> = result
                .steps
                .iter()
                .map(|s| (s.time, setup.impedances(&s.phasors)))
                .collect();
            json.samples = window.samples;
            match self.format {
                TableFormat::Json => {
                    json.steps = rows
                        .iter()
                        .map(|(t, z)| JsonStep {
                            time_s: round12(*t),
                            loops: setup.loops.iter().zip(z).map(|(&l, z)| JsonLoop::new(l, *z)).collect(),
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&json)?);
                }
                TableFormat::Csv => {
                    let mut out = String::from("time_ms");
                    for l in &setup.loops {
                        let n = l.name().replace('-', "");
                        out += &format!(",{n}_r_ohm,{n}_x_ohm");
                    }
                    out.push('\n');
                    for (t, z) in &rows {
                        out += &round12(t * 1e3).to_string();
                        for z in z {
                            match z {
                                Some((_, _, z)) => out += &format!(",{},{}", round12(z.re), round12(z.im)),
                                None => out += ",,",
                            }
                        }
                        out.push('\n');
                    }
                    print!("{out}");
                }
                TableFormat::Text => {
                    let mut headers = vec!["time".to_owned()];
                    for l in &setup.loops {
                        headers.push(format!("{} R", l.name()));
                        headers.push(format!("{} X", l.name()));
                    }
                    let span = rows.last().map_or(0.0, |r| r.0) - rows[0].0;
                    let table_rows: Vec<Vec<String>> = rows
                        .iter()
                        .map(|(t, z)| {
                            let mut r = vec![time(*t, span.max(1e-3))];
                            for z in z {
                                match z {
                                    Some((_, _, z)) => r.extend([si(z.re), si(z.im)]),
                                    None => r.extend(["-".to_owned(), "-".to_owned()]),
                                }
                            }
                            r
                        })
                        .collect();
                    let names: Vec<&str> = headers.iter().map(String::as_str).collect();
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
                    print!("\n{}", table(&names, &table_rows));
                }
            }
            return Ok(());
        }

        let window = window_at(&c, self.at, self.cycles, frequency)?;
        let phasors: Vec<Option<Complex>> = setup
            .vc
            .channels
            .iter()
            .map(|ch| phasor::phasor(ch, c.time(), &window, side).ok())
            .collect();
        let values = setup.impedances(&phasors);
        let (t0, t1) = (c.time()[window.start()], c.time()[window.end]);
        json.samples = window.samples;
        match self.format {
            TableFormat::Json => {
                json.from_s = Some(round12(t0));
                json.at_s = Some(round12(t1));
                json.loops = setup
                    .loops
                    .iter()
                    .zip(&values)
                    .map(|(&l, v)| JsonLoop::new(l, *v))
                    .collect();
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            TableFormat::Csv => {
                let mut out = String::from("loop,voltage_v,current_a,r_ohm,x_ohm,z_ohm,angle_deg\n");
                for (l, v) in setup.loops.iter().zip(&values) {
                    let j = JsonLoop::new(*l, *v);
                    let f = |x: Option<f64>| x.map_or_else(String::new, |x| x.to_string());
                    out += &format!(
                        "{},{},{},{},{},{},{}\n",
                        l.name(),
                        f(j.voltage),
                        f(j.current),
                        f(j.r),
                        f(j.x),
                        f(j.magnitude),
                        f(j.angle)
                    );
                }
                print!("{out}");
            }
            TableFormat::Text => {
                print!("{}", header(&describe_window(&window, t0, t1, self.cycles, frequency)));
                let headers = [
                    format!("V [{}]", pair.voltage.unit),
                    format!("I [{}]", pair.current.unit),
                ];
                let rows: Vec<Vec<String>> = setup
                    .loops
                    .iter()
                    .zip(&values)
                    .map(|(l, v)| match v {
                        // Loop V and I in the channels' units, so they can be compared with the plots.
                        Some((lv, li, z)) => vec![
                            l.name().to_owned(),
                            si(lv.abs() / 2f64.sqrt() / setup.vc.voltage_factor),
                            si(li.abs() / 2f64.sqrt() / setup.vc.current_factor),
                            si(z.re),
                            si(z.im),
                            si(z.abs()),
                            format!("{:.1}°", z.arg().to_degrees()),
                        ],
                        None => vec![
                            l.name().to_owned(),
                            "-".into(),
                            "-".into(),
                            "no loop current or missing value".into(),
                        ],
                    })
                    .collect();
                print!(
                    "\n{}",
                    table(
                        &["loop", &headers[0], &headers[1], "R [Ω]", "X [Ω]", "|Z| [Ω]", "φ"],
                        &rows
                    )
                );
            }
        }
        Ok(())
    }
}

/// `z` with a part below 1e-6 of |Z| set to 0: residue of integer samples,
/// which would show as `-31.65µ` next to 199.6 Ω (and as an angle of -0.0°).
fn clean(z: Complex) -> Complex {
    let small = |v: f64| if v.abs() <= 1e-6 * z.abs() { 0.0 } else { v };
    Complex {
        re: small(z.re),
        im: small(z.im),
    }
}
