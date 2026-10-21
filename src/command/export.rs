//! `export` command: samples as CSV or JSON. The formats are written by
//! [`crate::comtrade::export`]; this module handles the command line.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};

use super::{Context, bytes, select_channels, time_window};
use crate::args::ExportFormat;
use crate::comtrade::export::{Export, Format};
use crate::comtrade::{Comtrade, Side};

/// Options of the `export` command.
#[derive(Debug, Clone)]
pub struct Options {
    /// Any file of the recording.
    pub file: PathBuf,
    /// Output format; `None` chooses from the output file name.
    pub format: Option<ExportFormat>,
    /// Channel selection (indices or ids).
    pub channels: Vec<String>,
    /// Window start in ms relative to the trigger.
    pub from_ms: Option<f64>,
    /// Window end in ms relative to the trigger.
    pub to_ms: Option<f64>,
    /// Keep every Nth sample.
    pub every: usize,
    /// Absolute time of day instead of ms relative to the trigger.
    pub absolute: bool,
    /// Output file; stdout when `None`.
    pub output: Option<PathBuf>,
}

impl From<ExportFormat> for Format {
    fn from(f: ExportFormat) -> Self {
        match f {
            ExportFormat::Csv => Self::Csv,
            ExportFormat::Json => Self::Json,
        }
    }
}

/// Runs the `export` command.
pub fn run(ctx: &Context, opts: &Options) -> anyhow::Result<()> {
    let c = Comtrade::open(&opts.file)?;
    let (analog, binary) = select_channels(&c, &opts.channels)?;
    let (t0, t1) = time_window(&c, opts.from_ms, opts.to_ms)?;
    let export = Export {
        analog: analog
            .iter()
            .map(|&i| c.analog(i).expect("selected index is valid"))
            .collect(),
        binary: binary
            .iter()
            .map(|&i| c.binary(i).expect("selected index is valid"))
            .collect(),
        samples: c.index_range(t0, t1),
        every: opts.every,
        side: if ctx.global.secondary {
            Side::Secondary
        } else {
            Side::Primary
        },
        absolute: opts.absolute,
        ..Export::new(&c)
    };
    let format = opts.format.map_or_else(
        || {
            let json = opts
                .output
                .as_ref()
                .and_then(|p| p.extension())
                .is_some_and(|e| e.eq_ignore_ascii_case("json"));
            if json { Format::Json } else { Format::Csv }
        },
        Format::from,
    );

    match &opts.output {
        Some(path) => {
            refuse_source(&c, path)?;
            let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
            let mut out = BufWriter::new(file);
            export
                .write(format, &mut out)
                .and_then(|()| out.flush())
                .with_context(|| format!("writing {}", path.display()))?;
            let size = std::fs::metadata(path).map(|m| bytes(m.len())).unwrap_or_default();
            eprintln!(
                "wrote {} samples × {} channels to {} ({size})",
                export.len(),
                export.analog.len() + export.binary.len(),
                path.display()
            );
        }
        None => {
            let mut out = BufWriter::new(io::stdout().lock());
            let result = export.write(format, &mut out).and_then(|()| out.flush());
            // A reader that stops early (`| head`) is not an error.
            if let Err(e) = result
                && e.kind() != io::ErrorKind::BrokenPipe
            {
                return Err(e.into());
            }
        }
    }
    Ok(())
}

/// Refuses to replace a file of the source recording with the export.
fn refuse_source(c: &Comtrade, output: &Path) -> anyhow::Result<()> {
    let Ok(out) = output.canonicalize() else {
        return Ok(()); // does not exist yet
    };
    let source = c
        .path
        .iter()
        .flat_map(|p| [&p.cfg, &p.dat, &p.cff, &p.hdr, &p.inf])
        .flatten()
        .any(|p| p.canonicalize().is_ok_and(|p| p == out));
    if source {
        bail!(
            "{} is part of the source recording; choose another output name",
            output.display()
        );
    }
    Ok(())
}
