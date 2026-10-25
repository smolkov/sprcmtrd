//! Summary of a recording's configuration, shared by `info`, `view` and the
//! TUI info overlay.

use super::{rate, si, time, trim_float};
use crate::comtrade::Config;

/// Key/value summary of a configuration, shared by `info`, `view` and the TUI.
pub fn summary(file: &str, c: &Config) -> Vec<(&'static str, String)> {
    let offset = c.trigger_offset();
    let duration = c.duration();
    let span = duration.unwrap_or(offset.abs());
    let rates = c
        .sample_rates
        .iter()
        .scan(0, |first, r| {
            let s = format!(
                "{} (samples {}-{})",
                if r.rate > 0.0 {
                    rate(r.rate)
                } else {
                    "timestamps".into()
                },
                *first + 1,
                r.last_sample
            );
            *first = r.last_sample;
            Some(s)
        })
        .collect::<Vec<_>>()
        .join(", ");
    vec![
        ("File", file.to_owned()),
        ("Station", c.station_name.clone()),
        ("Device", c.device_id.clone()),
        ("Revision", c.revision.to_string()),
        ("Format", c.data_format.to_string()),
        ("Start", c.start_time.format("%Y-%m-%d %H:%M:%S%.6f").to_string()),
        (
            "Trigger",
            format!(
                "{} ({}{})",
                c.trigger_time.format("%Y-%m-%d %H:%M:%S%.6f"),
                if offset >= 0.0 { "+" } else { "" },
                time(offset, span)
            ),
        ),
        (
            "Duration",
            duration.map_or_else(|| "from timestamps".into(), |d| time(d, d)),
        ),
        ("Frequency", rate(c.line_frequency)),
        ("Sample rates", rates),
        ("Samples", c.total_samples().to_string()),
        (
            "Channels",
            format!("{} analog, {} binary", c.analog.len(), c.binary.len()),
        ),
    ]
}

/// Rows of the analog channel table.
pub fn analog_rows(c: &Config) -> (Vec<&'static str>, Vec<Vec<String>>) {
    let headers = vec![
        "#", "id", "phase", "circuit", "unit", "a", "b", "prim/sec", "min", "max",
    ];
    let rows = c
        .analog
        .iter()
        .map(|a| {
            vec![
                a.index.to_string(),
                a.id.clone(),
                a.phase.clone(),
                a.circuit.clone(),
                a.unit.clone(),
                format!("{}", a.a),
                format!("{}", a.b),
                format!("{}/{} {}", trim_float(a.primary), trim_float(a.secondary), a.side),
                si(a.min),
                si(a.max),
            ]
        })
        .collect();
    (headers, rows)
}

/// Rows of the binary channel table.
pub fn binary_rows(c: &Config) -> (Vec<&'static str>, Vec<Vec<String>>) {
    let headers = vec!["#", "id", "phase", "circuit", "normal"];
    let rows = c
        .binary
        .iter()
        .map(|b| {
            vec![
                b.index.to_string(),
                b.id.clone(),
                b.phase.clone(),
                b.circuit.clone(),
                u8::from(b.normal_state).to_string(),
            ]
        })
        .collect();
    (headers, rows)
}
