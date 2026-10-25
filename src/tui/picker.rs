//! File picker: entries found by a background directory scan, filtered by
//! the typed text.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use regex::{Regex, RegexBuilder};
use tokio::sync::mpsc::UnboundedSender;

use super::event::Message;
use crate::comtrade::Comtrade;
use crate::comtrade::discover::{self, Visit};

/// An entry in the file picker.
#[derive(Debug, Clone)]
pub enum PickerEntry {
    /// A subdirectory.
    Dir {
        /// Directory name.
        name: String,
        /// Full path.
        path: PathBuf,
    },
    /// A recording.
    Recording {
        /// Path relative to the picker directory.
        name: String,
        /// Path of the main file.
        path: PathBuf,
        /// Station name, start time and channel counts, or the parse error.
        details: Result<(String, String, usize, usize), String>,
    },
}

impl PickerEntry {
    /// Name used for filtering and display.
    pub fn name(&self) -> &str {
        match self {
            Self::Dir { name, .. } | Self::Recording { name, .. } => name,
        }
    }
}

/// File picker state.
#[derive(Debug, Clone)]
pub struct Picker {
    /// Directory being shown.
    pub dir: PathBuf,
    /// Filter text.
    pub query: String,
    /// All entries found so far.
    pub entries: Vec<PickerEntry>,
    /// Indices into `entries` matching the filter.
    pub filtered: Vec<usize>,
    /// Selected position in `filtered`.
    pub selected: usize,
    /// Scan still running.
    pub scanning: bool,
    /// Filter or scan error.
    pub error: Option<String>,
    /// Subdirectories are searched too.
    pub recursive: bool,
    /// Only recordings that load completely are listed.
    pub valid_only: bool,
    matcher: Matcher,
    pub(super) generation: u64,
}

/// Compiled picker filter.
#[derive(Debug, Clone)]
enum Matcher {
    /// Case-insensitive substring (stored lowercase).
    Substring(String),
    /// Case-insensitive regular expression.
    Regex(Regex),
}

impl Matcher {
    fn matches(&self, name: &str) -> bool {
        match self {
            Self::Substring(q) => name.to_lowercase().contains(q.as_str()),
            Self::Regex(re) => re.is_match(name),
        }
    }
}

impl Picker {
    pub(super) fn new(dir: PathBuf, query: String, recursive: bool, valid_only: bool, generation: u64) -> Self {
        let mut p = Self {
            dir,
            query,
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            scanning: true,
            error: None,
            recursive,
            valid_only,
            matcher: Matcher::Substring(String::new()),
            generation,
        };
        p.refilter();
        p
    }

    /// Recompiles the filter after a query change and filters all entries.
    pub(super) fn refilter(&mut self) {
        self.matcher = if let Some(re) = self.query.strip_prefix('/') {
            match RegexBuilder::new(re).case_insensitive(true).build() {
                Ok(re) => Matcher::Regex(re),
                Err(e) => {
                    // Keep the previous list while the regex is invalid.
                    self.error = Some(format!("invalid regex: {}", e.to_string().lines().last().unwrap_or("")));
                    return;
                }
            }
        } else {
            Matcher::Substring(self.query.to_lowercase())
        };
        self.error = None;
        self.filtered = (0..self.entries.len())
            .filter(|&i| self.matcher.matches(self.entries[i].name()))
            .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    /// Adds a scanned entry, filtering only the new one (scans can be large).
    pub(super) fn push(&mut self, entry: PickerEntry) {
        if self.matcher.matches(entry.name()) {
            self.filtered.push(self.entries.len());
        }
        self.entries.push(entry);
    }

    /// The selected entry.
    pub fn current(&self) -> Option<&PickerEntry> {
        self.filtered.get(self.selected).map(|&i| &self.entries[i])
    }
}

/// Scans `dir` for subdirectories and recordings (in all subdirectories
/// when `recursive`), sending entries as they are found. With `valid_only`,
/// recordings that fail to load completely are skipped. Stops early when
/// `current` no longer equals `generation` (a newer scan started or the
/// picker closed).
pub(super) fn scan(
    dir: &Path,
    recursive: bool,
    valid_only: bool,
    generation: u64,
    current: &AtomicU64,
    tx: &UnboundedSender<Message>,
) {
    let send = |entry| tx.send(Message::ScanEntry { generation, entry }).is_ok();
    let stale = || current.load(Ordering::Relaxed) != generation;

    // Immediate subdirectories first, for navigation.
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    for path in dirs {
        let name = format!("{}/", path.file_name().unwrap_or_default().to_string_lossy());
        if stale() || !send(PickerEntry::Dir { name, path }) {
            return;
        }
    }

    let result = discover::walk(dir, recursive, &mut |visit| {
        if stale() {
            return ControlFlow::Break(());
        }
        let Visit::Recording(rec) = visit else {
            return ControlFlow::Continue(());
        };
        let details = Comtrade::config_of(&rec)
            .map(|c| {
                (
                    c.station_name,
                    c.start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
                    c.analog.len(),
                    c.binary.len(),
                )
            })
            .map_err(|e| e.to_string());
        // The data must load too, as when the recording is opened.
        if valid_only && (details.is_err() || Comtrade::open_recording(&rec).is_err()) {
            return ControlFlow::Continue(());
        }
        let path = rec.main_file().to_path_buf();
        let name = path.strip_prefix(dir).unwrap_or(&path).display().to_string();
        if send(PickerEntry::Recording { name, path, details }) {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    });
    let _ = tx.send(Message::ScanDone {
        generation,
        error: result.err().map(|e| e.to_string()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_names(dir: &Path, recursive: bool, valid_only: bool, generation: u64, current: u64) -> Vec<String> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        scan(dir, recursive, valid_only, generation, &AtomicU64::new(current), &tx);
        drop(tx);
        let mut names = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            if let Message::ScanEntry { entry, .. } = msg {
                names.push(entry.name().to_owned());
            }
        }
        names
    }

    #[test]
    fn picker_scan_is_recursive_and_cancellable() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::write(d.join("top.cfg"), "").unwrap();
        std::fs::write(d.join("a/b/deep.cfg"), "").unwrap();

        let deep = Path::new("a").join("b").join("deep.cfg").display().to_string();
        assert_eq!(scan_names(d, true, false, 1, 1), ["a/", "top.cfg", deep.as_str()]);
        assert_eq!(scan_names(d, false, false, 1, 1), ["a/", "top.cfg"]);
        // A newer scan has started: nothing is sent.
        assert!(scan_names(d, true, false, 1, 2).is_empty());
    }

    #[test]
    fn picker_scan_valid_only_skips_broken_recordings() {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        for f in ["broken.cfg", "fault_1999.cfg", "fault_1999.dat", "old_1991.cfg"] {
            std::fs::copy(data.join(f), d.join(f)).unwrap();
        }
        // old_1991.cfg parses but has no data file.
        assert_eq!(
            scan_names(d, false, false, 1, 1),
            ["broken.cfg", "fault_1999.cfg", "old_1991.cfg"]
        );
        assert_eq!(scan_names(d, false, true, 1, 1), ["fault_1999.cfg"]);
    }

    #[test]
    fn picker_filters_new_entries() {
        let mut p = Picker::new(PathBuf::from("/"), "/^a/b".into(), true, false, 1);
        for name in ["a/b/x.cfg", "c/a/b.cfg"] {
            p.push(PickerEntry::Recording {
                name: name.into(),
                path: name.into(),
                details: Err(String::new()),
            });
        }
        assert_eq!(p.filtered, [0]);
        p.query = "B.CFG".into();
        p.refilter();
        assert_eq!(p.filtered, [1]);
    }
}
