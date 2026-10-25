//! `harmonics` command: harmonic spectrum and THD per channel at one moment.

use anyhow::bail;
use serde::Serialize;

use super::{select_channels, window_at};
use crate::analysis::harmonics::{self, Spectrum};
use crate::args::HarmonicsArgs;
use crate::comtrade::{AnalogChannel, Comtrade, Side, round12};
use crate::context::Context;
use crate::format::{key_values, rate, si, table, time};

#[derive(Serialize)]
struct JsonHarmonic {
    order: usize,
    /// RMS value.
    magnitude: f64,
    /// % of the fundamental.
    percent: f64,
    /// Degrees relative to the fundamental (φₕ − h·φ₁).
    angle: f64,
}

#[derive(Serialize)]
struct JsonChannel {
    index: usize,
    id: String,
    unit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fundamental: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dc: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thd: Option<f64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    harmonics: Vec<JsonHarmonic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct Json {
    file: String,
    side: &'static str,
    frequency: f64,
    /// Window start in s relative to the trigger.
    from_s: f64,
    /// Window end in s relative to the trigger.
    at_s: f64,
    samples: usize,
    max_order: usize,
    channels: Vec<JsonChannel>,
}

impl HarmonicsArgs {
    /// Runs the `harmonics` command.
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
            bail!("{} is a binary channel; harmonics need analog channels", ch.info.id);
        }
        if analog.is_empty() {
            bail!("the recording has no analog channels");
        }
        let frequency = self.frequency.unwrap_or(c.config.line_frequency);
        if frequency <= 0.0 {
            bail!("the recording has no line frequency; give it with --frequency");
        }
        let window = window_at(&c, self.at, self.cycles, frequency)?;
        let limit = harmonics::max_order(c.time(), &window);
        let max_order = (self.max_order as usize).min(limit);

        let channels: Vec<&AnalogChannel> = analog
            .iter()
            .map(|&i| c.analog(i).expect("selected index is valid"))
            .collect();
        let spectra: Vec<_> = channels
            .iter()
            .map(|ch| harmonics::spectrum(ch, c.time(), &window, max_order, side))
            .collect();
        let (t0, t1) = (c.time()[window.start()], c.time()[window.end]);
        let name = c
            .path
            .as_ref()
            .map(|p| p.main_file().display().to_string())
            .unwrap_or_default();
        let side_name = if side == Side::Secondary {
            "secondary"
        } else {
            "primary"
        };

        if self.json {
            let out = Json {
                file: name,
                side: side_name,
                frequency,
                from_s: round12(t0),
                at_s: round12(t1),
                samples: window.samples,
                max_order,
                channels: channels
                    .iter()
                    .zip(&spectra)
                    .map(|(ch, s)| json_channel(ch, s))
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        let span = (t1 - t0).max(1e-3);
        let orders = if (self.max_order as usize) > limit {
            format!(
                "up to {max_order} (higher orders are above half the sample rate, {})",
                rate(limit as f64 * frequency + frequency)
            )
        } else {
            format!("up to {max_order}")
        };
        let mut out = key_values(&[
            ("File", name),
            (
                "Window",
                format!(
                    "{} .. {} ({} {}, {} samples, {})",
                    time(t0, span),
                    time(t1, span),
                    self.cycles,
                    if self.cycles == 1.0 { "cycle" } else { "cycles" },
                    window.samples,
                    rate(frequency)
                ),
            ),
            ("Orders", orders),
            (
                "Values",
                format!("{side_name}; DC and harmonics in % of the fundamental"),
            ),
        ]);
        out += &format!("\n{}", text_table(&channels, &spectra, max_order, self.angles));
        for (ch, s) in channels.iter().zip(&spectra) {
            if let Err(e) = s {
                out += &format!("\nA{} {}: {e}", ch.info.index, ch.info.id);
            }
        }
        print!("{}", out.trim_end());
        println!();
        Ok(())
    }
}

/// Rows: fundamental (RMS), DC, each order, THD; a column per channel (and
/// its angles with `angles`).
fn text_table(
    channels: &[&AnalogChannel],
    spectra: &[Result<Spectrum, crate::analysis::phasor::PhasorError>],
    max_order: usize,
    angles: bool,
) -> String {
    let mut headers = vec!["order".to_owned()];
    for ch in channels {
        headers.push(format!("{} [{}]", ch.info.id, ch.info.unit));
        if angles {
            headers.push(format!("{} ∠", ch.info.id));
        }
    }
    let percent = |p: Option<f64>| p.map_or_else(|| "-".to_owned(), |p| format!("{p:.1} %"));
    let row = |label: &str, value: &dyn Fn(&Spectrum) -> (String, String)| {
        let mut r = vec![label.to_owned()];
        for s in spectra {
            let (v, a) = s.as_ref().map_or_else(|_| ("-".into(), "-".into()), value);
            r.push(v);
            if angles {
                r.push(a);
            }
        }
        r
    };
    let mut rows = vec![
        {
            // The fundamental as a value (unit in the header), so the
            // percentages below can be read as values too.
            let mut r = vec!["fundamental".to_owned()];
            for s in spectra {
                r.push(s.as_ref().map_or_else(|_| "-".into(), |s| si(s.fundamental_rms())));
                if angles {
                    r.push(String::new());
                }
            }
            r
        },
        row("DC", &|s| (percent(s.dc_percent()), String::new())),
    ];
    for h in 2..=max_order {
        rows.push(row(&h.to_string(), &|s| {
            (
                percent(s.percent(h)),
                // An angle means nothing for a harmonic that shows as 0.0 %.
                match (s.percent(h), s.angle(h)) {
                    (Some(p), Some(a)) if p >= 0.05 => format!("{a:.1}°"),
                    _ => "-".into(),
                },
            )
        }));
    }
    rows.push(row(&format!("THD 2–{max_order}"), &|s| {
        (percent(s.thd()), String::new())
    }));
    let names: Vec<&str> = headers.iter().map(String::as_str).collect();
    table(&names, &rows)
}

fn json_channel(ch: &AnalogChannel, s: &Result<Spectrum, crate::analysis::phasor::PhasorError>) -> JsonChannel {
    let mut out = JsonChannel {
        index: ch.info.index,
        id: ch.info.id.clone(),
        unit: ch.info.unit.clone(),
        fundamental: None,
        dc: None,
        thd: None,
        harmonics: Vec::new(),
        error: None,
    };
    match s {
        Err(e) => out.error = Some(e.to_string()),
        Ok(s) => {
            out.fundamental = Some(round12(s.fundamental_rms()));
            out.dc = Some(round12(s.dc));
            out.thd = s.thd().map(round12);
            out.harmonics = (2..=s.max_order())
                .filter_map(|h| {
                    let p = s.phasor(h)?;
                    Some(JsonHarmonic {
                        order: h,
                        magnitude: round12(p.abs() / 2f64.sqrt()),
                        percent: round12(s.percent(h).unwrap_or(0.0)),
                        angle: round12(s.angle(h)?),
                    })
                })
                .collect();
        }
    }
    out
}
