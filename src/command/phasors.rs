//! `phasors` command: fundamental phasors and symmetrical components at a time.

use anyhow::{Context as _, bail};
use serde::Serialize;

use super::info::key_values;
use super::{Context, rate, select_channels, si, table, time};
use crate::args::PhasorsArgs;
use crate::comtrade::phasor::{self, Complex, Polar};
use crate::comtrade::{Comtrade, Side, round12};

#[derive(Serialize)]
struct JsonPhasor {
    index: usize,
    id: String,
    unit: String,
    #[serde(flatten)]
    value: Option<Polar>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct JsonSequence {
    circuit: String,
    unit: String,
    channels: [String; 3],
    zero: Polar,
    positive: Polar,
    negative: Polar,
    /// Negative / positive sequence in %.
    unbalance: Option<f64>,
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    /// Window end in s relative to the trigger.
    at_s: f64,
    /// Window start in s relative to the trigger.
    from_s: f64,
    samples: usize,
    reference: String,
    phasors: Vec<JsonPhasor>,
    sequences: Vec<JsonSequence>,
}

impl PhasorsArgs {
    /// Runs the `phasors` command.
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
            bail!("{} is a binary channel; phasors need analog channels", ch.info.id);
        }
        if analog.is_empty() {
            bail!("the recording has no analog channels");
        }
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        if self.cycles <= 0.0 {
            bail!("--cycles must be above 0");
        }

        let end = c.index_at(self.at / 1e3).context("the recording has no samples")?;
        let samples = phasor::window(c.time(), end, self.cycles, frequency).unwrap_or(0);
        if samples < 4 {
            bail!(
                "{} cycles at {frequency} Hz are only {samples} samples here; use more --cycles",
                self.cycles
            );
        }
        let Some(start) = (end + 1).checked_sub(samples) else {
            let earliest = c.time().get(samples - 1).copied().unwrap_or(0.0);
            bail!(
                "the window of {samples} samples starts before the recording; use --at {:.1} or later",
                earliest * 1e3
            );
        };

        let channels: Vec<_> = analog
            .iter()
            .map(|&i| c.analog(i).expect("selected index is valid"))
            .collect();
        let values: Vec<Result<Complex, phasor::PhasorError>> = channels
            .iter()
            .map(|ch| phasor::phasor(ch, c.time(), end, samples, frequency, side))
            .collect();

        // The reference: the given channel (also outside the selection), else
        // the first selected one.
        let (reference_name, reference) = match &self.reference {
            Some(r) => {
                let (a, _) = select_channels(&c, std::slice::from_ref(r))?;
                let Some(&i) = a.first() else {
                    bail!("the reference {r} is not an analog channel");
                };
                let ch = c.analog(i).expect("selected index is valid");
                let p = phasor::phasor(ch, c.time(), end, samples, frequency, side)
                    .with_context(|| format!("reference {}", ch.info.id))?;
                (format!("A{} {}", ch.info.index, ch.info.id), p)
            }
            None => {
                let ch = channels[0];
                let p = values[0].clone().with_context(|| format!("reference {}", ch.info.id))?;
                (format!("A{} {}", ch.info.index, ch.info.id), p)
            }
        };
        let angle0 = reference.arg();

        let infos: Vec<_> = channels.iter().map(|ch| &ch.info).collect();
        let sequences: Vec<JsonSequence> = phasor::groups(&infos)
            .into_iter()
            .filter_map(|g| {
                let [a, b, c] = g.phases.map(|i| values[i].clone().ok());
                let s = phasor::symmetrical(a?, b?, c?);
                let positive = s.positive.abs();
                Some(JsonSequence {
                    circuit: g.circuit,
                    unit: g.unit,
                    channels: g.phases.map(|i| channels[i].info.id.clone()),
                    zero: rounded(Polar::new(s.zero, angle0)),
                    positive: rounded(Polar::new(s.positive, angle0)),
                    negative: rounded(Polar::new(s.negative, angle0)),
                    unbalance: (positive > 0.0).then(|| round12(s.negative.abs() / positive * 100.0)),
                })
            })
            .collect();
        let phasors: Vec<JsonPhasor> = channels
            .iter()
            .zip(&values)
            .map(|(ch, v)| JsonPhasor {
                index: ch.info.index,
                id: ch.info.id.clone(),
                unit: ch.info.unit.clone(),
                value: v.as_ref().ok().map(|&p| rounded(Polar::new(p, angle0))),
                error: v.as_ref().err().map(ToString::to_string),
            })
            .collect();

        let (t0, t1) = (c.time()[start], c.time()[end]);
        let name = c
            .path
            .as_ref()
            .map(|p| p.main_file().display().to_string())
            .unwrap_or_default();
        let side_name = match side {
            Side::Primary => "primary",
            Side::Secondary => "secondary",
        };

        if self.json {
            let out = Json {
                file: name,
                side: side_name,
                frequency,
                at_s: round12(t1),
                from_s: round12(t0),
                samples,
                reference: reference_name,
                phasors,
                sequences,
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        let span = (t1 - t0).max(1e-3);
        let mut out = key_values(&[
            ("File", name),
            (
                "Window",
                format!(
                    "{} .. {} ({} {}, {samples} samples, {})",
                    time(t0, span),
                    time(t1, span),
                    self.cycles,
                    if self.cycles == 1.0 { "cycle" } else { "cycles" },
                    rate(frequency)
                ),
            ),
            ("Reference", reference_name),
            ("Values", format!("{side_name}, RMS")),
        ]);
        // An angle means nothing for a signal that is (almost) zero.
        let largest = phasors
            .iter()
            .filter_map(|p| p.value.map(|v| v.magnitude))
            .fold(0.0, f64::max);
        let rows: Vec<Vec<String>> = phasors
            .iter()
            .map(|p| {
                let (magnitude, angle) = match (&p.value, &p.error) {
                    (Some(v), _) if v.magnitude <= 1e-9 * largest => (si(v.magnitude), "-".into()),
                    (Some(v), _) => (si(v.magnitude), format!("{:.1}°", v.angle)),
                    (None, Some(e)) => ("-".into(), e.clone()),
                    (None, None) => ("-".into(), String::new()),
                };
                vec![format!("A{} {}", p.index, p.id), p.unit.clone(), magnitude, angle]
            })
            .collect();
        out += &format!("\n{}", table(&["channel", "unit", "magnitude", "angle"], &rows));

        if !sequences.is_empty() {
            // An angle means nothing for a component that is (almost) zero.
            let polar = |p: &Polar, scale: f64| {
                if p.magnitude <= 1e-6 * scale {
                    "0".to_owned()
                } else {
                    format!("{} ∠ {:.1}°", si(p.magnitude), p.angle)
                }
            };
            let rows: Vec<Vec<String>> = sequences
                .iter()
                .map(|s| {
                    let scale = s.positive.magnitude.max(s.negative.magnitude).max(s.zero.magnitude);
                    vec![
                        s.channels.join(" "),
                        s.circuit.clone(),
                        s.unit.clone(),
                        polar(&s.zero, scale),
                        polar(&s.positive, scale),
                        polar(&s.negative, scale),
                        s.unbalance.map_or_else(|| "-".into(), |u| format!("{u:.1} %")),
                    ]
                })
                .collect();
            out += &format!(
                "\nSymmetrical components\n{}",
                table(
                    &["phases", "circuit", "unit", "zero", "positive", "negative", "unbalance"],
                    &rows
                )
            );
        }
        print!("{out}");
        Ok(())
    }
}

/// `p` with its values rounded to 12 significant digits (float noise).
fn rounded(p: Polar) -> Polar {
    Polar {
        magnitude: round12(p.magnitude),
        angle: round12(p.angle),
    }
}
