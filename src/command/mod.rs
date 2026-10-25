//! The subcommands (`sprcmtrd <command>`): dispatch and the channel and time
//! window selection they share. Formatting lives in [`crate::format`].

pub mod check;
pub mod completions;
pub mod convert;
pub mod cut;
pub mod events;
pub mod export;
pub mod info;
pub mod list;
pub mod phasors;
pub mod stats;
pub mod trend;
pub mod view;

use anyhow::bail;

use crate::args::Command;
use crate::comtrade::Comtrade;
use crate::context::Context;
use crate::format::time;

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
            Self::Trend(a) => a.run(ctx),
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
