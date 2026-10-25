//! `convert` command: writes a recording as another revision, data format or
//! file layout (`.cfg` + `.dat` or combined `.cff`).

use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};

use crate::args::{ConvertArgs, FormatArg, RevisionArg};
use crate::comtrade::{Comtrade, DataFormat, Revision, write};
use crate::context::Context;
use crate::format::bytes;

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

impl ConvertArgs {
    /// Runs the `convert` command. `format` and `rev` default to the source's,
    /// except that revision 2013 is chosen when the format or a `.cff` output
    /// needs it. Existing files are only replaced with `force`, and never the
    /// source recording itself.
    pub fn run(self, _ctx: &Context) -> anyhow::Result<()> {
        let Self {
            file,
            output,
            format,
            rev,
            force,
        } = self;
        let src = Comtrade::open(&file)?;
        let saved = save(
            &src,
            &src,
            &output,
            format.map(DataFormat::from),
            rev.map(Revision::from),
            force,
        )?;
        println!(
            "{} ({} {}) -> {} ({} {})",
            file.display(),
            src.config.revision,
            src.config.data_format,
            output.display(),
            saved.revision,
            saved.format
        );
        saved.print();
        Ok(())
    }
}

/// A recording written by [`save`].
pub struct Saved {
    /// Revision of the written recording.
    pub revision: Revision,
    /// Data format of the written recording.
    pub format: DataFormat,
    /// Files written with their sizes in bytes.
    pub files: Vec<(PathBuf, usize)>,
    /// Changes made to fit the target, see [`write::Encoded::notes`].
    pub notes: Vec<String>,
    /// The recording as read back from disk.
    pub back: Comtrade,
}

impl Saved {
    /// Prints the files, notes and read-back result as indented lines.
    pub fn print(&self) {
        for (path, size) in &self.files {
            println!("  wrote {}  {}", path.display(), bytes(*size as u64));
        }
        for note in &self.notes {
            println!("  note: {note}");
        }
        println!(
            "  read back: {} samples, {} analog, {} binary channels",
            self.back.len(),
            self.back.analog_channels().len(),
            self.back.binary_channels().len()
        );
    }
}

/// Writes `rec` to `output` (`NAME.cfg` with its siblings, or `NAME.cff`) in
/// `format` and `rev`, which default to the recording's (2013 when the format
/// or `.cff` needs it), and reads it back. `source` is the recording `rec`
/// was made from; its files are never overwritten, other existing files only
/// with `force`.
pub fn save(
    source: &Comtrade,
    rec: &Comtrade,
    output: &Path,
    format: Option<DataFormat>,
    rev: Option<Revision>,
    force: bool,
) -> anyhow::Result<Saved> {
    let ext = output
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let combined = match ext.to_ascii_lowercase().as_str() {
        "cff" => true,
        "cfg" => false,
        _ => bail!("output must end in .cfg or .cff: {}", output.display()),
    };

    let format = format.unwrap_or(rec.config.data_format);
    let needs_2013 = combined || matches!(format, DataFormat::Binary32 | DataFormat::Float32);
    let revision = match rev {
        Some(r) => r,
        None if needs_2013 => Revision::Rev2013,
        None => rec.config.revision,
    };
    if combined && revision != Revision::Rev2013 {
        bail!("the combined .cff format needs revision 2013, not {revision}");
    }
    if let Err(e) = write::check_target(revision, format) {
        bail!("{}", e.to_string().trim_start_matches("unsupported: "));
    }

    let encoded = write::encode(rec, revision, format)?;
    let files: Vec<(PathBuf, Vec<u8>)> = if combined {
        let cff = encoded.cff(rec.info.as_deref(), rec.header.as_deref());
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
        for (e, text) in [("hdr", &rec.header), ("inf", &rec.info)] {
            if let Some(text) = text {
                files.push((sibling(e), text.clone().into_bytes()));
            }
        }
        files
    };

    check_outputs(source, &files, force)?;
    for (path, data) in &files {
        std::fs::write(path, data).with_context(|| format!("writing {}", path.display()))?;
    }

    // Read the result back, so a recording that this program (or another
    // reader following the standard) can't open is never reported as done.
    let back = Comtrade::open(output).context("reading back the written recording")?;
    if back.len() != rec.len()
        || back.analog_channels().len() != rec.analog_channels().len()
        || back.binary_channels().len() != rec.binary_channels().len()
    {
        bail!(
            "the written recording reads back with {} samples instead of {}",
            back.len(),
            rec.len()
        );
    }
    Ok(Saved {
        revision,
        format,
        files: files.iter().map(|(p, d)| (p.clone(), d.len())).collect(),
        notes: encoded.notes,
        back,
    })
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
