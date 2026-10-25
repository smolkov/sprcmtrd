//! `info` command: summary of one recording.

use std::path::Path;

use serde::Serialize;

use crate::args::InfoArgs;
use crate::comtrade::{Comtrade, Config, RecordingPath};
use crate::context::Context;
use crate::format::{analog_rows, binary_rows, key_values, summary, table};

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

impl InfoArgs {
    /// Runs the `info` command.
    pub fn run(self, _ctx: &Context) -> anyhow::Result<()> {
        let Self { file, json, no_extra } = self;
        let rec = RecordingPath::from_file(&file)?;
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
