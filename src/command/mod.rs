//! The subcommands (`sprcmtrd <command>`): dispatch and the channel and time
//! window selection they share. Formatting lives in [`crate::format`].

pub mod check;
pub mod completions;
pub mod convert;
pub mod cut;
pub mod events;
pub mod export;
pub mod frequency;
pub mod harmonics;
pub mod impedance;
pub mod info;
pub mod list;
pub mod phasors;
pub mod power;
pub mod stats;
pub mod trend;
pub mod view;

use anyhow::{Context as _, anyhow, bail};

use crate::analysis::Complex;
use crate::analysis::impedance::{Pair, Quantity, find_pair, unit_factor};
use crate::analysis::phasor::{Window, WindowError};
use crate::analysis::sequence::Group;
use crate::analysis::trend::Trend;
use crate::args::{Command, TrendStep};
use crate::comtrade::{AnalogChannel, AnalogInfo, Comtrade, Side};
use crate::context::Context;
use crate::format::{rate, time};

impl Command {
    /// Runs the command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        match self {
            Self::Info(a) => a.run(ctx),
            Self::List(a) => a.run(ctx),
            Self::View(a) => a.run(ctx),
            Self::Check(a) => a.run(ctx),
            Self::Events(a) => a.run(ctx),
            Self::Phasors(a) => a.run(ctx),
            Self::Impedance(a) => a.run(ctx),
            Self::Power(a) => a.run(ctx),
            Self::Harmonics(a) => a.run(ctx),
            Self::Trend(a) => a.run(ctx),
            Self::Frequency(a) => a.run(ctx),
            Self::Stats(a) => a.run(ctx),
            Self::Export(a) => a.run(ctx),
            Self::Cut(a) => a.run(ctx),
            Self::Convert(a) => a.run(ctx),
            Self::Completions(a) => a.run(ctx),
        }
    }
}

/// Resolves a `--channels` selection (1-based analog indices, `dN` binary
/// indices or channel ids) into analog and binary positions. An empty
/// selection means all channels.
pub fn select_channels(c: &Comtrade, tokens: &[String]) -> anyhow::Result<(Vec<usize>, Vec<usize>)> {
    let n_analog = c.analog_channels().len();
    let n_binary = c.binary_channels().len();
    if tokens.is_empty() {
        return Ok(((0..n_analog).collect(), (0..n_binary).collect()));
    }
    let (mut analog, mut binary) = (Vec::new(), Vec::new());
    for token in tokens.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let digital_index = token.strip_prefix(['d', 'D']).and_then(|n| n.parse::<usize>().ok());
        if let Ok(n) = token.parse::<usize>() {
            if n == 0 || n > n_analog {
                bail!("analog channel {n} does not exist (1..={n_analog})");
            }
            analog.push(n - 1);
        } else if let Some(n) = digital_index {
            if n == 0 || n > n_binary {
                bail!("binary channel d{n} does not exist (d1..=d{n_binary})");
            }
            binary.push(n - 1);
        } else if let Some(i) = c.analog_channels().position(|a| a.info.id.eq_ignore_ascii_case(token)) {
            analog.push(i);
        } else if let Some(i) = c.binary_channels().position(|b| b.info.id.eq_ignore_ascii_case(token)) {
            binary.push(i);
        } else {
            bail!("unknown channel '{token}'");
        }
    }
    Ok((analog, binary))
}

/// The analysis window of `cycles` cycles of `frequency` ending at `at_ms`
/// (ms relative to the trigger), with errors that name the option to change.
pub fn window_at(c: &Comtrade, at_ms: f64, cycles: f64, frequency: f64) -> anyhow::Result<Window> {
    Window::ending_at(c, at_ms / 1e3, cycles, frequency).map_err(|e| match e {
        WindowError::TooShort(n) => {
            anyhow!("{cycles} cycles at {frequency} Hz are only {n} samples here; use more --cycles")
        }
        WindowError::BeforeStart { samples, earliest } => anyhow!(
            "the window of {samples} samples starts before the recording; use --at {:.1} or later",
            earliest * 1e3
        ),
        e => anyhow!(e),
    })
}

/// Time window in seconds relative to the trigger from `--from` / `--to` in
/// ms; the whole recording when they are not given.
pub fn time_window(c: &Comtrade, from_ms: Option<f64>, to_ms: Option<f64>) -> anyhow::Result<(f64, f64)> {
    let (first, last) = c.time_span();
    let t0 = from_ms.map_or(first, |ms| ms / 1e3);
    let t1 = to_ms.map_or(last, |ms| ms / 1e3);
    if t1 <= t0 {
        let span = last - first;
        bail!(
            "empty time window {} .. {}; the recording runs from {} to {}",
            time(t0, span),
            time(t1, span),
            time(first, span),
            time(last, span)
        );
    }
    Ok((t0, t1))
}

/// A voltage and a current A/B/C group (`impedance`, `power`): the six
/// channels and the factors from their units to volts and amperes.
pub struct VoltageCurrent<'a> {
    /// The groups, as positions of analog channels.
    pub pair: Pair,
    /// Voltages A, B, C, then currents A, B, C.
    pub channels: [&'a AnalogChannel; 6],
    /// Factor from the voltage unit to V.
    pub voltage_factor: f64,
    /// Factor from the current unit to A.
    pub current_factor: f64,
}

impl<'a> VoltageCurrent<'a> {
    /// The groups from `--voltage` / `--current` (both or neither), else
    /// found by unit.
    pub fn new(c: &'a Comtrade, voltage: &[String], current: &[String]) -> anyhow::Result<Self> {
        let pair = match (voltage.is_empty(), current.is_empty()) {
            (true, true) => {
                let infos: Vec<&AnalogInfo> = c.analog_channels().map(|ch| &ch.info).collect();
                find_pair(&infos).context(
                    "no voltage and current A/B/C channel groups found; give them with --voltage and --current",
                )?
            }
            (false, false) => Pair {
                voltage: group(c, voltage, "--voltage")?,
                current: group(c, current, "--current")?,
            },
            _ => bail!("give both --voltage and --current, or neither"),
        };
        let ch = |i: usize| c.analog(i).expect("channel position is valid");
        let [va, vb, vc] = pair.voltage.phases.map(ch);
        let [ia, ib, ic] = pair.current.phases.map(ch);
        Ok(Self {
            channels: [va, vb, vc, ia, ib, ic],
            voltage_factor: factor(&pair.voltage, Quantity::Voltage)?,
            current_factor: factor(&pair.current, Quantity::Current)?,
            pair,
        })
    }

    /// Channel ids of the voltages and of the currents.
    pub fn ids(&self) -> ([String; 3], [String; 3]) {
        let id = |k: usize| self.channels[k].info.id.clone();
        ([0, 1, 2].map(id), [3, 4, 5].map(id))
    }

    /// The phasors of the six channels (in their order) in V and A; `None`
    /// if one is missing.
    pub fn scaled(&self, phasors: &[Option<Complex>]) -> Option<([Complex; 3], [Complex; 3])> {
        let get = |k: usize, f: f64| phasors.get(k).copied().flatten().map(|p| p.scale(f));
        let v = [
            get(0, self.voltage_factor)?,
            get(1, self.voltage_factor)?,
            get(2, self.voltage_factor)?,
        ];
        let i = [
            get(3, self.current_factor)?,
            get(4, self.current_factor)?,
            get(5, self.current_factor)?,
        ];
        Some((v, i))
    }
}

/// Factor from the unit of group `g` to V or A, an error if it is not `q`.
fn factor(g: &Group, q: Quantity) -> anyhow::Result<f64> {
    match unit_factor(&g.unit) {
        Some((k, f)) if k == q => Ok(f),
        _ => Err(anyhow!(
            "unit '{}' is not a {}",
            g.unit,
            if q == Quantity::Voltage {
                "voltage (V, kV)"
            } else {
                "current (A, kA)"
            }
        )),
    }
}

/// Three analog channels from an option, in phase order A, B, C.
fn group(c: &Comtrade, tokens: &[String], option: &str) -> anyhow::Result<Group> {
    let (analog, binary) = select_channels(c, tokens)?;
    if analog.len() != 3 || !binary.is_empty() {
        bail!("{option} needs three analog channels (phases A, B, C)");
    }
    let info = |i: usize| &c.analog(i).expect("selected index is valid").info;
    Ok(Group {
        circuit: info(analog[0]).circuit.clone(),
        unit: info(analog[0]).unit.clone(),
        phases: [analog[0], analog[1], analog[2]],
    })
}

/// Window line of the header: `t0 .. t1 (1 cycle, 40 samples, 50 Hz)`.
pub fn describe_window(window: &Window, t0: f64, t1: f64, cycles: f64, frequency: f64) -> String {
    let span = (t1 - t0).max(1e-3);
    format!(
        "{} .. {} ({} {}, {} samples, {})",
        time(t0, span),
        time(t1, span),
        cycles,
        if cycles == 1.0 { "cycle" } else { "cycles" },
        window.samples,
        rate(frequency)
    )
}

/// Steps of `--trend` (`impedance`, `power`): windows of `cycles` cycles
/// every `step` within `--from` / `--to`, with the window length. An error if
/// no window fits.
#[allow(clippy::too_many_arguments)]
pub fn trend_steps(
    c: &Comtrade,
    channels: &[&AnalogChannel],
    from_ms: Option<f64>,
    to_ms: Option<f64>,
    step: TrendStep,
    cycles: f64,
    frequency: f64,
    side: Side,
) -> anyhow::Result<(Window, Trend)> {
    let (first, last) = c.time_span();
    let window = window_at(c, last * 1e3, cycles, frequency)?;
    let dt = cycles / frequency / window.samples as f64;
    let step = match step {
        TrendStep::Cycles(n) => n / frequency,
        TrendStep::Seconds(s) => s,
    };
    let (t0, t1) = time_window(c, from_ms, to_ms)?;
    let result = crate::analysis::trend::trend(
        channels,
        c.time(),
        c.index_range(t0, t1),
        window.samples,
        ((step / dt).round() as usize).max(1),
        frequency,
        0,
        side,
    );
    if result.steps.is_empty() {
        bail!(
            "no window of {} samples fits in the time range; the recording runs from {} to {}",
            window.samples,
            time(first, last - first),
            time(last, last - first)
        );
    }
    Ok((window, result))
}
