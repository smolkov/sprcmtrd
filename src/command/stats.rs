//! `stats` command: statistics per channel over a time window.

use serde::Serialize;

use super::{select_channels, time_window};
use crate::analysis::stats::{self, AnalogStats, BinaryStats};
use crate::args::StatsArgs;
use crate::comtrade::{Comtrade, Side};
use crate::context::Context;
use crate::format::{key_values, si, table, time};

#[derive(Serialize)]
struct Json {
    file: String,
    /// `primary` or `secondary`.
    side: &'static str,
    /// Window start in s relative to the trigger.
    from_s: f64,
    /// Window end in s relative to the trigger.
    to_s: f64,
    samples: usize,
    analog: Vec<AnalogStats>,
    binary: Vec<BinaryStats>,
}

impl StatsArgs {
    /// Runs the `stats` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let Self {
            file,
            channels,
            from: from_ms,
            to: to_ms,
            json,
        } = self;
        let c = Comtrade::open(&file)?;
        let side = if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        };
        let (analog, binary) = select_channels(&c, &channels)?;
        let (t0, t1) = time_window(&c, from_ms, to_ms)?;
        let range = c.index_range(t0, t1);
        let analog: Vec<AnalogStats> = analog
            .iter()
            .map(|&i| {
                stats::analog(
                    c.analog(i).expect("selected index is valid"),
                    c.time(),
                    range.clone(),
                    side,
                )
            })
            .collect();
        let binary: Vec<BinaryStats> = binary
            .iter()
            .map(|&i| stats::binary(c.binary(i).expect("selected index is valid"), c.time(), range.clone()))
            .collect();
        let name = c
            .path
            .as_ref()
            .map(|p| p.main_file().display().to_string())
            .unwrap_or_default();
        let side_name = match side {
            Side::Primary => "primary",
            Side::Secondary => "secondary",
        };

        if json {
            let out = Json {
                file: name,
                side: side_name,
                from_s: t0,
                to_s: t1,
                samples: range.len(),
                analog,
                binary,
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        let span = t1 - t0;
        let at = |t: Option<f64>| t.map_or_else(|| "-".into(), |t| time(t, span));
        let value = |v: Option<f64>| v.map_or_else(|| "-".into(), si);
        let mut out = key_values(&[
            ("File", name),
            (
                "Window",
                format!("{} .. {} ({} samples)", time(t0, span), time(t1, span), range.len()),
            ),
            ("Values", side_name.into()),
        ]);
        if !analog.is_empty() {
            let headers = [
                "channel", "unit", "min", "at", "max", "at", "mean", "rms", "peak", "at", "missing",
            ];
            let rows: Vec<Vec<String>> = analog
                .iter()
                .map(|s| {
                    vec![
                        format!("A{} {}", s.index, s.id),
                        s.unit.clone(),
                        value(s.min),
                        at(s.min_time),
                        value(s.max),
                        at(s.max_time),
                        value(s.mean),
                        value(s.rms),
                        value(s.peak),
                        at(s.peak_time),
                        s.missing.to_string(),
                    ]
                })
                .collect();
            out += &format!("\nAnalog channels\n{}", table(&headers, &rows));
        }
        if !binary.is_empty() {
            let headers = ["channel", "first", "last", "changes", "first change", "time set"];
            let rows: Vec<Vec<String>> = binary
                .iter()
                .map(|s| {
                    vec![
                        format!("D{} {}", s.index, s.id),
                        u8::from(s.first).to_string(),
                        u8::from(s.last).to_string(),
                        s.changes.to_string(),
                        at(s.first_change),
                        time(s.time_set, span),
                    ]
                })
                .collect();
            out += &format!("\nBinary channels\n{}", table(&headers, &rows));
        }
        print!("{out}");
        Ok(())
    }
}
