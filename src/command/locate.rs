//! `locate` command: single-ended fault location (reactance method and
//! Takagi) from the recording of one line end.

use anyhow::{Context as _, bail};
use serde::Serialize;

use super::{VoltageCurrent, describe_window, time_window, window_at};
use crate::analysis::Complex;
use crate::analysis::fault::{self, Fault};
use crate::analysis::impedance::Loop;
use crate::analysis::locate::{self, LoopValues};
use crate::analysis::phasor::{self, Window};
use crate::args::{LineImpedance, LocateArgs};
use crate::comtrade::{Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, si, table, time};

/// A complex value in JSON.
#[derive(Serialize)]
struct JsonComplex {
    re: f64,
    im: f64,
}

impl From<Complex> for JsonComplex {
    fn from(c: Complex) -> Self {
        Self {
            re: round12(c.re),
            im: round12(c.im),
        }
    }
}

#[derive(Serialize)]
struct JsonLine {
    z1: JsonComplex,
    z0: Option<JsonComplex>,
    length_km: f64,
    k0: JsonComplex,
}

#[derive(Serialize)]
struct JsonWindow {
    from_s: f64,
    to_s: f64,
    samples: usize,
}

#[derive(Serialize)]
struct JsonLoop {
    name: &'static str,
    /// Loop voltage in V (RMS phasor).
    v: JsonComplex,
    /// Loop current in A (RMS phasor), ground loops compensated.
    i: JsonComplex,
    /// Apparent impedance in Ω primary.
    z: Option<JsonComplex>,
}

#[derive(Serialize)]
struct JsonResult {
    method: &'static str,
    distance_km: Option<f64>,
    percent: Option<f64>,
    note: Option<&'static str>,
}

#[derive(Serialize)]
struct JsonFound {
    fault_type: String,
    inception_s: f64,
    window: JsonWindow,
    #[serde(rename = "loop")]
    fault_loop: Option<JsonLoop>,
    results: Vec<JsonResult>,
}

#[derive(Serialize)]
struct Json {
    file: String,
    frequency: f64,
    voltage: [String; 3],
    current: [String; 3],
    line: JsonLine,
    found: bool,
    #[serde(flatten)]
    fault: Option<JsonFound>,
}

/// One method's result: distance in km, with a note when it lies off the line.
struct Located {
    method: &'static str,
    km: Option<f64>,
}

impl Located {
    fn percent(&self, length: f64) -> Option<f64> {
        self.km.map(|d| d / length * 100.0)
    }

    fn note(&self, length: f64) -> Option<&'static str> {
        match self.km {
            None => Some("no result (no loop or superimposed current)"),
            Some(d) if d < 0.0 => Some("behind the relay"),
            Some(d) if d > length => Some("beyond the line end (or wrong line data)"),
            Some(_) => None,
        }
    }
}

fn complex(z: LineImpedance) -> Complex {
    Complex { re: z.r, im: z.x }
}

/// `0.0300+j0.4000`.
fn rectangular(z: Complex) -> String {
    let sign = if z.im < 0.0 { '-' } else { '+' };
    format!("{:.4}{sign}j{:.4}", z.re, z.im.abs())
}

/// `si` of a rectangular value: `-1.286+j11.51`.
fn rectangular_si(z: Complex) -> String {
    let sign = if z.im < 0.0 { '-' } else { '+' };
    format!("{}{sign}j{}", si(z.re), si(z.im.abs()))
}

/// The loop to measure: `--loop`, else from the faulted phases.
fn choose_loop(option: Option<&str>, f: &Fault) -> anyhow::Result<Loop> {
    match option {
        Some(s) => Loop::parse(s).with_context(|| format!("unknown loop '{s}' (AN, BN, CN, AB, BC, CA)")),
        None => locate::fault_loop(f.kind).context("no faulted phase found"),
    }
}

/// Loop values from the six phasors at `window` and the pre-fault currents,
/// in V and A; `None` with a missing value.
fn measure(c: &Comtrade, vc: &VoltageCurrent, l: Loop, window: &Window, f: &Fault, k0: Complex) -> Option<LoopValues> {
    let phasors = |w: &Window| -> Vec<Option<Complex>> {
        vc.channels
            .iter()
            .map(|ch| phasor::phasor(ch, c.time(), w, Side::Primary).ok())
            .collect()
    };
    let (v, i) = vc.scaled(&phasors(window))?;
    let (_, pre) = vc.scaled(&phasors(&f.pre_window))?;
    Some(locate::loop_values(l, v, i, pre, k0))
}

impl LocateArgs {
    /// Runs the `locate` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        if ctx.global.secondary {
            bail!("the line data is primary; locate works on primary values only (leave out --secondary)");
        }
        let c = Comtrade::open(&self.file)?;
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        let vc = VoltageCurrent::new(&c, &self.voltage, &self.current)?;
        let z1 = complex(self.z1);
        let z0 = self.z0.map(complex);
        let k0 = z0.map_or_else(Complex::default, |z0| locate::k0(z1, z0));
        let (t0, t1) = time_window(&c, self.from, self.to)?;
        let range = c.index_range(t0, t1);
        let currents = [vc.channels[3], vc.channels[4], vc.channels[5]];
        let found = fault::analyse(currents, c.time(), range.clone(), frequency, Side::Primary);

        let file = c.name();
        let (voltage, current) = vc.ids();
        let line = JsonLine {
            z1: z1.into(),
            z0: z0.map(Into::into),
            length_km: self.length,
            k0: k0.into(),
        };
        let (first, last) = c.time_span();
        let span = last - first;
        let signed = |t: f64| {
            let s = time(t, span);
            if t > 0.0 { format!("+{s}") } else { s }
        };
        let k0_text = if z0.is_some() {
            format!(", k0 {:.3} ∠ {:.1}°", k0.abs(), k0.arg().to_degrees())
        } else {
            String::new()
        };
        let mut head = vec![
            ("File", file.clone()),
            (
                "Channels",
                format!(
                    "{} [{}] · {} [{}]",
                    voltage.join(" "),
                    vc.pair.voltage.unit,
                    current.join(" "),
                    vc.pair.current.unit
                ),
            ),
            (
                "Line",
                format!(
                    "z1 {} Ω/km{}, {} km{k0_text}, primary",
                    rectangular(z1),
                    z0.map_or_else(String::new, |z| format!(", z0 {} Ω/km", rectangular(z))),
                    self.length
                ),
            ),
        ];

        let Some(f) = found else {
            if self.json {
                let out = Json {
                    file,
                    frequency,
                    voltage,
                    current,
                    line,
                    found: false,
                    fault: None,
                };
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                head.push(("Searched", format!("{} .. {}", signed(t0), signed(t1))));
                print!("{}\nNo fault found.\n", key_values(&head));
            }
            return Ok(());
        };

        let l = choose_loop(self.fault_loop.as_deref(), &f)?;
        if locate::is_ground(l) && z0.is_none() {
            bail!(
                "the {} loop needs the zero sequence impedance of the line; give it with --z0",
                l.name()
            );
        }
        let window = match self.at {
            Some(at) => window_at(&c, at, self.cycles, frequency)?,
            None => {
                let samples = (self.cycles * f.per_cycle as f64).round() as usize;
                locate::window(&f, range.end, samples)
            }
        };
        let values = measure(&c, &vc, l, &window, &f, k0);
        let results = [
            Located {
                method: "reactance",
                km: values.as_ref().and_then(|m| locate::reactance(m, z1)),
            },
            Located {
                method: "Takagi",
                km: values.as_ref().and_then(|m| locate::takagi(m, z1)),
            },
        ];
        let time_of = |i: usize| c.time()[i];
        let inception = time_of(f.inception);
        let (w0, w1) = (time_of(window.start()), time_of(window.end));

        if self.json {
            let rms = |z: Complex| JsonComplex::from(z.scale(1.0 / 2f64.sqrt()));
            let out = Json {
                file,
                frequency,
                voltage,
                current,
                line,
                found: true,
                fault: Some(JsonFound {
                    fault_type: f.kind.name(),
                    inception_s: round12(inception),
                    window: JsonWindow {
                        from_s: round12(w0),
                        to_s: round12(w1),
                        samples: window.samples,
                    },
                    fault_loop: values.map(|m| JsonLoop {
                        name: l.name(),
                        v: rms(m.v),
                        i: rms(m.i),
                        z: (m.i.abs() > 0.0).then(|| (m.v / m.i).into()),
                    }),
                    results: results
                        .iter()
                        .map(|r| JsonResult {
                            method: r.method,
                            distance_km: r.km.map(round12),
                            percent: r.percent(self.length).map(round12),
                            note: r.note(self.length),
                        })
                        .collect(),
                }),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        head.push(("Fault", format!("{} at {}", f.kind.name(), signed(inception))));
        head.push(("Window", describe_window(&window, w0, w1, self.cycles, frequency)));
        let Some(m) = values else {
            head.push(("Loop", l.name().to_owned()));
            print!(
                "{}\nA value is missing in the window; no location. Try another --at.\n",
                key_values(&head)
            );
            return Ok(());
        };
        let rms = |z: Complex| z.abs() / 2f64.sqrt();
        let z = (m.i.abs() > 0.0).then(|| m.v / m.i);
        head.push((
            "Loop",
            format!(
                "{}: V {}V, I {}A, Z {}",
                l.name(),
                si(rms(m.v)),
                si(rms(m.i)),
                z.map_or_else(|| "-".to_owned(), |z| format!("{} Ω", rectangular_si(z)))
            ),
        ));
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.method.to_owned(),
                    r.km.map_or_else(|| "-".to_owned(), |d| format!("{d:.2} km")),
                    r.percent(self.length)
                        .map_or_else(|| "-".to_owned(), |p| format!("{p:.1} %")),
                    r.note(self.length).unwrap_or_default().to_owned(),
                ]
            })
            .collect();
        print!(
            "{}\n{}",
            key_values(&head),
            table(&["method", "distance", "of line", "note"], &rows)
        );
        Ok(())
    }
}
