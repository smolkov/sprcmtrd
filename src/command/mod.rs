//! Command dispatch and shared output formatting.

pub mod completions;
pub mod convert;
pub mod export;
pub mod info;
pub mod list;
pub mod view;

use anyhow::bail;

use crate::args::{Cli, Command, GlobalOpts};
use crate::comtrade::Comtrade;
use crate::config::AppConfig;

/// Settings available to all commands.
#[derive(Debug, Clone)]
pub struct Context {
    /// Global command line options.
    pub global: GlobalOpts,
    /// Configuration file settings.
    pub config: AppConfig,
    /// Where the configuration overlay saves settings (`None`: no config directory).
    pub config_path: Option<std::path::PathBuf>,
}

/// Runs the subcommand given on the command line, or the TUI when there is none.
pub async fn run(cli: Cli, config: AppConfig) -> anyhow::Result<()> {
    let ctx = Context {
        global: cli.global,
        config,
        config_path: AppConfig::default_path(),
    };
    match cli.command {
        Some(cmd) => cmd.run(&ctx).await,
        None => crate::tui::run(cli.path, &ctx).await,
    }
}

impl Command {
    /// Runs the command. CLI commands do synchronous work.
    pub async fn run(self, ctx: &Context) -> anyhow::Result<()> {
        match self {
            Self::Info { file, json, no_extra } => info::run(&file, json, no_extra),
            Self::List {
                dir,
                no_recursive,
                filter,
                valid,
                sort,
            } => list::run(&dir, !no_recursive, filter.as_deref(), valid, sort),
            Self::View {
                file,
                channels,
                from,
                to,
                width,
                height,
            } => view::run(
                ctx,
                &view::Options {
                    file,
                    channels,
                    from_ms: from,
                    to_ms: to,
                    width,
                    height,
                },
            ),
            Self::Export {
                file,
                format,
                channels,
                from,
                to,
                every,
                absolute,
                output,
            } => export::run(
                ctx,
                &export::Options {
                    file,
                    format,
                    channels,
                    from_ms: from,
                    to_ms: to,
                    every: every as usize,
                    absolute,
                    output,
                },
            ),
            Self::Convert {
                file,
                output,
                format,
                rev,
                force,
            } => convert::run(&file, &output, format, rev, force),
            Self::Completions { shell, output } => completions::run(shell, output.as_deref()),
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
        bail!("empty time window: --from must be before --to");
    }
    Ok((t0, t1))
}

/// Formats `v` with 4 significant digits and an SI prefix (`12.34k`).
pub fn si(v: f64) -> String {
    if !v.is_finite() {
        return "-".into();
    }
    let a = v.abs();
    let (scaled, prefix) = match a {
        0.0 => (v, ""),
        a if a >= 1e9 => (v / 1e9, "G"),
        a if a >= 1e6 => (v / 1e6, "M"),
        a if a >= 1e3 => (v / 1e3, "k"),
        a if a >= 1.0 => (v, ""),
        a if a >= 1e-3 => (v * 1e3, "m"),
        _ => (v * 1e6, "µ"),
    };
    let s = scaled.abs();
    let decimals = if s >= 100.0 {
        1
    } else if s >= 10.0 {
        2
    } else {
        3
    };
    format!("{scaled:.decimals$}{prefix}")
}

/// Formats a time in seconds as ms or s, choosing precision from `span`.
pub fn time(t: f64, span: f64) -> String {
    if span.abs() >= 2.0 {
        format!("{t:.3} s")
    } else {
        let ms = t * 1e3;
        let decimals = match span.abs() * 1e3 {
            s if s >= 100.0 => 1,
            s if s >= 10.0 => 2,
            _ => 3,
        };
        format!("{ms:.decimals$} ms")
    }
}

/// Formats a frequency in Hz (`4.8 kHz`).
pub fn rate(hz: f64) -> String {
    if hz >= 1e3 {
        format!("{} kHz", trim_float(hz / 1e3))
    } else {
        format!("{} Hz", trim_float(hz))
    }
}

/// Formats a byte count (`2.1 MB`).
pub fn bytes(n: u64) -> String {
    let n = n as f64;
    match n {
        n if n >= 1e9 => format!("{:.1} GB", n / 1e9),
        n if n >= 1e6 => format!("{:.1} MB", n / 1e6),
        n if n >= 1e3 => format!("{:.1} kB", n / 1e3),
        n => format!("{n} B"),
    }
}

/// Formats a float without trailing zeros (`50`, `4.8`).
pub fn trim_float(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Renders rows as a left-aligned text table with a header line.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (w, cell) in widths.iter_mut().zip(row) {
            *w = (*w).max(cell.chars().count());
        }
    }
    let line = |cells: &mut dyn Iterator<Item = &str>| {
        let mut s = String::new();
        for (i, (cell, w)) in cells.zip(&widths).enumerate() {
            if i > 0 {
                s.push_str("  ");
            }
            s.push_str(cell);
            s.extend(std::iter::repeat_n(' ', w - cell.chars().count()));
        }
        s.trim_end().to_owned() + "\n"
    };
    let mut out = line(&mut headers.iter().copied());
    for row in rows {
        out += &line(&mut row.iter().map(String::as_str));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_numbers() {
        assert_eq!(si(12345.0), "12.35k");
        assert_eq!(si(-0.5), "-500.0m");
        assert_eq!(si(0.0), "0.000");
        assert_eq!(time(0.0124, 0.05), "12.40 ms");
        assert_eq!(time(3.5, 10.0), "3.500 s");
        assert_eq!(rate(4800.0), "4.8 kHz");
        assert_eq!(rate(50.0), "50 Hz");
        assert_eq!(bytes(2_100_000), "2.1 MB");
    }

    #[test]
    fn renders_table() {
        let t = table(&["a", "bb"], &[vec!["xyz".into(), "1".into()]]);
        assert_eq!(t, "a    bb\nxyz  1\n");
    }
}
