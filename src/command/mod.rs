//! Command dispatch and shared output formatting.

pub mod info;
pub mod list;
pub mod view;

use crate::args::{Cli, Command, GlobalOpts};
use crate::config::AppConfig;

/// Settings available to all commands.
#[derive(Debug, Clone)]
pub struct Context {
    /// Global command line options.
    pub global: GlobalOpts,
    /// Configuration file settings.
    pub config: AppConfig,
}

/// Runs the subcommand given on the command line, or the TUI when there is none.
pub async fn run(cli: Cli, config: AppConfig) -> anyhow::Result<()> {
    let ctx = Context {
        global: cli.global,
        config,
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
            Self::Info {
                file,
                json,
                no_extra,
            } => info::run(&file, json, no_extra),
            Self::List {
                dir,
                no_recursive,
                filter,
                sort,
            } => list::run(&dir, !no_recursive, filter.as_deref(), sort),
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
        }
    }
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
