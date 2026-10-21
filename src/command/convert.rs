//! `convert` command: writes a recording as another revision, data format or
//! file layout (`.cfg` + `.dat` or combined `.cff`).

use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

use super::bytes;
use crate::args::{FormatArg, RevisionArg};
use crate::comtrade::{Comtrade, DataFormat, Revision, write};

impl From<FormatArg> for DataFormat {
    fn from(f: FormatArg) -> Self {
        match f {
            FormatArg::Ascii => Self::Ascii,
            FormatArg::Binary => Self::Binary,
            FormatArg::Binary32 => Self::Binary32,
            FormatArg::Float32 => Self::Float32,
        }
    }
}

impl From<RevisionArg> for Revision {
    fn from(r: RevisionArg) -> Self {
        match r {
            RevisionArg::Rev1991 => Self::Rev1991,
            RevisionArg::Rev1999 => Self::Rev1999,
            RevisionArg::Rev2013 => Self::Rev2013,
        }
    }
}

/// Runs the `convert` command. `format` and `rev` default to the source's,
/// except that revision 2013 is chosen when the format or a `.cff` output
/// needs it. Existing files are only replaced with `force`, and never the
/// source recording itself.
pub fn run(
    file: &Path,
    output: &Path,
    format: Option<FormatArg>,
    rev: Option<RevisionArg>,
    force: bool,
) -> anyhow::Result<()> {
    let ext = output
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let combined = match ext.to_ascii_lowercase().as_str() {
        "cff" => true,
        "cfg" => false,
        _ => bail!("output must end in .cfg or .cff: {}", output.display()),
    };

    let src = Comtrade::open(file)?;
    let format = format.map_or(src.config.data_format, DataFormat::from);
    let needs_2013 = combined || matches!(format, DataFormat::Binary32 | DataFormat::Float32);
    let revision = match rev {
        Some(r) => Revision::from(r),
        None if needs_2013 => Revision::Rev2013,
        None => src.config.revision,
    };
    if combined && revision != Revision::Rev2013 {
        bail!("the combined .cff format needs revision 2013, not {revision}");
    }
    if let Err(e) = write::check_target(revision, format) {
        bail!("{}", e.to_string().trim_start_matches("unsupported: "));
    }

    let encoded = write::encode(&src, revision, format)?;
    let files: Vec<(PathBuf, Vec<u8>)> = if combined {
        let cff = encoded.cff(src.info.as_deref(), src.header.as_deref());
        vec![(output.to_path_buf(), cff)]
    } else {
        // Sibling files take the case of the given extension (NAME.CFG -> NAME.DAT).
        let sibling = |e: &str| {
            let e = if ext.chars().all(|c| c.is_ascii_uppercase()) {
                e.to_ascii_uppercase()
            } else {
                e.to_owned()
            };
            output.with_extension(e)
        };
        let mut files = vec![
            (output.to_path_buf(), encoded.cfg.clone().into_bytes()),
            (sibling("dat"), encoded.dat.clone()),
        ];
        for (e, text) in [("hdr", &src.header), ("inf", &src.info)] {
            if let Some(text) = text {
                files.push((sibling(e), text.clone().into_bytes()));
            }
        }
        files
    };

    check_outputs(&src, &files, force)?;
    for (path, data) in &files {
        std::fs::write(path, data).with_context(|| format!("writing {}", path.display()))?;
    }

    // Read the result back, so a recording that this program (or another
    // reader following the standard) can't open is never reported as done.
    let back = Comtrade::open(output).context("reading back the converted recording")?;
    if back.len() != src.len()
        || back.analog_channels().len() != src.analog_channels().len()
        || back.binary_channels().len() != src.binary_channels().len()
    {
        bail!(
            "the converted recording reads back with {} samples instead of {}",
            back.len(),
            src.len()
        );
    }

    println!(
        "{} ({} {}) -> {} ({revision} {format})",
        file.display(),
        src.config.revision,
        src.config.data_format,
        output.display()
    );
    for (path, data) in &files {
        println!("  wrote {}  {}", path.display(), bytes(data.len() as u64));
    }
    for note in &encoded.notes {
        println!("  note: {note}");
    }
    println!(
        "  read back: {} samples, {} analog, {} binary channels",
        back.len(),
        back.analog_channels().len(),
        back.binary_channels().len()
    );
    Ok(())
}

/// Refuses to write over the source recording, and over other existing
/// files unless `force`.
fn check_outputs(src: &Comtrade, files: &[(PathBuf, Vec<u8>)], force: bool) -> anyhow::Result<()> {
    let sources: Vec<PathBuf> = src
        .path
        .iter()
        .flat_map(|p| [&p.cfg, &p.dat, &p.cff, &p.hdr, &p.inf])
        .flatten()
        .filter_map(|p| p.canonicalize().ok())
        .collect();
    let mut existing = Vec::new();
    for (path, _) in files {
        let Ok(real) = path.canonicalize() else {
            continue; // does not exist yet
        };
        if sources.contains(&real) {
            bail!(
                "{} is part of the source recording; choose another output name",
                path.display()
            );
        }
        existing.push(path.display().to_string());
    }
    if !existing.is_empty() && !force {
        bail!("{} already exists (use --force to overwrite)", existing.join(", "));
    }
    Ok(())
}
