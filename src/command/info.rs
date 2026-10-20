//! `info` command: summary of one recording.

use std::path::Path;

use serde::Serialize;

use super::{rate, si, table, time, trim_float};
use crate::comtrade::{Comtrade, Config, RecordingPath};

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

/// Formats key/value pairs as aligned lines.
pub fn key_values(pairs: &[(&str, String)]) -> String {
    let w = pairs.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    pairs.iter().map(|(k, v)| format!("{k:<w$}  {v}\n")).collect()
}

#[derive(Serialize)]
struct Json<'a> {
    file: String,
    config: &'a Config,
    samples: usize,
    duration_s: Option<f64>,
    trigger_offset_s: f64,
    header: Option<String>,
    info: Option<String>,
}

/// Runs the `info` command.
pub fn run(file: &Path, json: bool, no_extra: bool) -> anyhow::Result<()> {
    let rec = RecordingPath::from_file(file)?;
    let config = Comtrade::config_of(&rec)?;
    let (header, info) = if no_extra { (None, None) } else { extras(&rec)? };
    let name = rec.main_file().display().to_string();

    if json {
        let out = Json {
            file: name,
            config: &config,
            samples: config.total_samples(),
            duration_s: config.duration(),
            trigger_offset_s: config.trigger_offset(),
            header,
            info,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    let mut out = key_values(&summary(&name, &config));
    if !config.analog.is_empty() {
        let (h, rows) = analog_rows(&config);
        out += &format!("\nAnalog channels\n{}", table(&h, &rows));
    }
    if !config.binary.is_empty() {
        let (h, rows) = binary_rows(&config);
        out += &format!("\nBinary channels\n{}", table(&h, &rows));
    }
    for (title, text) in [("Header (.hdr)", header), ("Information (.inf)", info)] {
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            out += &format!("\n{title}\n{}\n", text.trim_end());
        }
    }
    print!("{out}");
    Ok(())
}

/// Reads `.hdr` and `.inf` content (from files or the `.cff`).
fn extras(rec: &RecordingPath) -> anyhow::Result<(Option<String>, Option<String>)> {
    use crate::comtrade::{Error, cff, decode_text};
    let read_bytes = |p: &Path| std::fs::read(p).map_err(|e| Error::io(p, e));
    if rec.cfg.is_none() {
        let path = rec.cff.as_ref().expect("recording has cfg or cff");
        let data = read_bytes(path)?;
        let s = cff::split(&data)?;
        return Ok((s.hdr.map(decode_text), s.inf.map(decode_text)));
    }
    let read = |p: &Option<std::path::PathBuf>| -> anyhow::Result<Option<String>> {
        p.as_ref().map(|p| Ok(decode_text(&read_bytes(p)?))).transpose()
    };
    Ok((read(&rec.hdr)?, read(&rec.inf)?))
}
