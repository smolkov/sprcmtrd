//! Locating the files that belong to a recording and finding recordings in directories.

use std::collections::BTreeMap;
use std::fs;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// File kinds that make up a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ext {
    Cfg,
    Dat,
    Hdr,
    Inf,
    Cff,
}

fn ext_of(path: &Path) -> Option<Ext> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "cfg" => Ext::Cfg,
        "dat" => Ext::Dat,
        "hdr" => Ext::Hdr,
        "inf" => Ext::Inf,
        "cff" => Ext::Cff,
        _ => return None,
    })
}

/// Paths of the files of one recording (files sharing the same name stem).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordingPath {
    /// Configuration file.
    pub cfg: Option<PathBuf>,
    /// Data file.
    pub dat: Option<PathBuf>,
    /// Header file.
    pub hdr: Option<PathBuf>,
    /// Information file.
    pub inf: Option<PathBuf>,
    /// 2013 combined file.
    pub cff: Option<PathBuf>,
}

impl RecordingPath {
    /// Finds all sibling files of `path`, which may be any file of the
    /// recording or the path without extension.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if path.is_dir() {
            return Err(Error::Unsupported(format!(
                "{} is a directory, expected a COMTRADE file",
                path.display()
            )));
        }
        let stem = if ext_of(path).is_some() {
            path.file_stem()
        } else {
            path.file_name()
        }
        .ok_or_else(|| Error::MissingFile(path.display().to_string()))?
        .to_owned();
        let dir = match path.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => PathBuf::from("."),
        };

        let mut rec = Self::default();
        let entries = fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.file_stem() == Some(stem.as_os_str()) {
                rec.insert(p);
            }
        }
        if rec.cfg.is_none() && rec.cff.is_none() {
            if !path.exists() {
                return Err(Error::io(
                    path,
                    std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
                ));
            }
            return Err(Error::MissingFile(format!(
                "{}.cfg (configuration for {})",
                dir.join(&stem).display(),
                path.display()
            )));
        }
        Ok(rec)
    }

    fn insert(&mut self, p: PathBuf) {
        let slot = match ext_of(&p) {
            Some(Ext::Cfg) => &mut self.cfg,
            Some(Ext::Dat) => &mut self.dat,
            Some(Ext::Hdr) => &mut self.hdr,
            Some(Ext::Inf) => &mut self.inf,
            Some(Ext::Cff) => &mut self.cff,
            None => return,
        };
        // Prefer the exact lowercase extension when several spellings exist.
        let lower = p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.chars().all(|c| c.is_ascii_lowercase()));
        if slot.is_none() || lower {
            *slot = Some(p);
        }
    }

    /// The file that identifies the recording (`.cfg`, else `.cff`).
    pub fn main_file(&self) -> &Path {
        self.cfg
            .as_deref()
            .or(self.cff.as_deref())
            .or(self.dat.as_deref())
            .expect("recording has at least one file")
    }

    /// File name of [`Self::main_file`].
    pub fn name(&self) -> String {
        self.main_file()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// Returns `true` when sample data is available (`.dat` or `.cff`).
    pub fn has_data(&self) -> bool {
        self.cfg.is_some() && self.dat.is_some() || self.cfg.is_none() && self.cff.is_some()
    }

    /// Size in bytes of the data file (or the `.cff`), if it exists.
    pub fn data_size(&self) -> Option<u64> {
        let p = if self.cfg.is_some() {
            self.dat.as_ref()?
        } else {
            self.cff.as_ref()?
        };
        fs::metadata(p).ok().map(|m| m.len())
    }
}

/// Something found while walking directories, see [`walk`].
#[derive(Debug)]
pub enum Visit<'a> {
    /// A directory is about to be searched.
    Dir(&'a Path),
    /// A recording was found.
    Recording(RecordingPath),
}

/// Walks `dir` (and its subdirectories when `recursive`) and calls `f` for
/// every directory entered and every recording found. Recordings are
/// identified by a `.cfg` or `.cff` file. Returning [`ControlFlow::Break`]
/// from `f` stops the walk.
///
/// Recursion skips hidden directories (`.git`, ...) and does not follow
/// symbolic links to directories, so link cycles cannot loop forever.
pub fn walk(
    dir: &Path,
    recursive: bool,
    f: &mut dyn FnMut(Visit) -> ControlFlow<()>,
) -> Result<ControlFlow<()>> {
    if f(Visit::Dir(dir)).is_break() {
        return Ok(ControlFlow::Break(()));
    }
    let mut groups: BTreeMap<PathBuf, RecordingPath> = BTreeMap::new();
    let mut subdirs = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let real_dir = entry.file_type().is_ok_and(|t| t.is_dir());
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if recursive && real_dir && !hidden {
                subdirs.push(p);
            }
            continue;
        }
        if ext_of(&p).is_some() {
            groups.entry(p.with_extension("")).or_default().insert(p);
        }
    }
    for rec in groups.into_values() {
        if (rec.cfg.is_some() || rec.cff.is_some()) && f(Visit::Recording(rec)).is_break() {
            return Ok(ControlFlow::Break(()));
        }
    }
    subdirs.sort();
    for sub in subdirs {
        // Unreadable subdirectories are skipped, the top level must be readable.
        match walk(&sub, true, f) {
            Ok(ControlFlow::Break(())) => return Ok(ControlFlow::Break(())),
            Ok(ControlFlow::Continue(())) => {}
            Err(e) => tracing::warn!("skipping {}: {e}", sub.display()),
        }
    }
    Ok(ControlFlow::Continue(()))
}

/// Returns all recordings in `dir`, see [`walk`].
pub fn find_recordings(dir: &Path, recursive: bool) -> Result<Vec<RecordingPath>> {
    let mut out = Vec::new();
    // The callback never breaks, so the flow is always Continue.
    let _ = walk(dir, recursive, &mut |v| {
        if let Visit::Recording(r) = v {
            out.push(r);
        }
        ControlFlow::Continue(())
    })?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_siblings_and_recordings() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        for f in ["a.CFG", "a.dat", "a.hdr", "b.cfg", "c.dat", "notes.txt"] {
            fs::write(d.join(f), "").unwrap();
        }
        fs::create_dir(d.join("sub")).unwrap();
        fs::write(d.join("sub/x.cff"), "").unwrap();

        let r = RecordingPath::from_file(d.join("a.dat")).unwrap();
        assert_eq!(r.cfg, Some(d.join("a.CFG")));
        assert_eq!(r.hdr, Some(d.join("a.hdr")));
        assert!(r.has_data());

        let names: Vec<_> = find_recordings(d, false).unwrap().iter().map(|r| r.name()).collect();
        assert_eq!(names, ["a.CFG", "b.cfg"]);
        assert_eq!(find_recordings(d, true).unwrap().len(), 3);

        // Hidden directories and directory symlinks are not searched.
        fs::create_dir(d.join(".hidden")).unwrap();
        fs::write(d.join(".hidden/h.cfg"), "").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(d, d.join("sub/loop")).unwrap();
        assert_eq!(find_recordings(d, true).unwrap().len(), 3);

        // Breaking stops the walk.
        let mut seen = 0;
        let flow = walk(d, true, &mut |v| {
            seen += usize::from(matches!(v, Visit::Recording(_)));
            if seen == 2 { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        })
        .unwrap();
        assert!(flow.is_break());
        assert_eq!(seen, 2);

        assert!(matches!(
            RecordingPath::from_file(d.join("c.dat")),
            Err(Error::MissingFile(_))
        ));
        assert!(matches!(
            RecordingPath::from_file(d.join("zzz.cfg")),
            Err(Error::Io { .. })
        ));
    }
}
