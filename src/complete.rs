//! Dynamic shell completion (`COMPLETE=<shell> sprcmtrd`).
//!
//! The shell calls the binary on every Tab; clap completes subcommands,
//! options and values, and [`recordings`] completes arguments that take a
//! recording with the COMTRADE files in the typed directory and its
//! subdirectories.

use std::ffi::OsStr;
use std::ops::ControlFlow;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};
use std::time::{Duration, Instant};

use clap_complete::engine::CompletionCandidate;

use crate::comtrade::discover::{self, Visit};

/// Most recordings offered at once; a shell can't show more usefully.
const MAX_RECORDINGS: usize = 500;

/// Time the search may take, so Tab in a large tree (`~`) answers quickly.
const BUDGET: Duration = Duration::from_millis(200);

/// Completer for arguments that take a recording: the subdirectories of the
/// typed directory, then the recordings (`.cfg` / `.cff`) in it and below
/// whose path starts with the typed text.
pub fn recordings(current: &OsStr) -> Vec<CompletionCandidate> {
    let Ok(cwd) = std::env::current_dir() else {
        return Vec::new();
    };
    let home = std::env::home_dir();
    candidates(
        &current.to_string_lossy(),
        &cwd,
        home.as_deref(),
        MAX_RECORDINGS,
        BUDGET,
    )
    .into_iter()
    .map(CompletionCandidate::new)
    .collect()
}

/// Completion candidates for the typed text `current`, as the shell expects
/// them: starting with the typed directory as written, directories ending
/// with a separator.
fn candidates(current: &str, cwd: &Path, home: Option<&Path>, max: usize, budget: Duration) -> Vec<String> {
    // The typed text up to the last separator is the directory to search,
    // the rest is the start of a name in it.
    let split = current.rfind(['/', MAIN_SEPARATOR]).map_or(0, |i| i + 1);
    let (typed_dir, name) = current.split_at(split);
    let root = match typed_dir.strip_prefix("~/") {
        Some(rest) => match home {
            Some(home) => home.join(rest),
            None => return Vec::new(),
        },
        // An absolute typed directory replaces `cwd` in the join.
        None => cwd.join(typed_dir),
    };
    let show = |p: &Path| format!("{typed_dir}{}", p.strip_prefix(&root).unwrap_or(p).display());

    // Subdirectories whose name continues the typed text; hidden ones only
    // when a name starting with `.` is typed, as `list` skips them.
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().unwrap_or_default().to_string_lossy();
            n.starts_with(name) && (name.starts_with('.') || !n.starts_with('.'))
        })
        .collect();
    dirs.sort();
    let mut out: Vec<String> = dirs.iter().map(|d| format!("{}{MAIN_SEPARATOR}", show(d))).collect();

    // Recordings directly in `root`, then in the matching subdirectories and
    // below, until `max` are found or the time is up.
    let deadline = Instant::now() + budget;
    let mut found = 0;
    let mut collect = |visit: Visit| {
        if found >= max || Instant::now() > deadline {
            return ControlFlow::Break(());
        }
        if let Visit::Recording(rec) = visit {
            let path = show(rec.main_file());
            if path.starts_with(current) {
                out.push(path);
                found += 1;
            }
        }
        ControlFlow::Continue(())
    };
    let flow = discover::walk(&root, false, &mut collect);
    if flow.is_ok_and(|f| f.is_continue()) {
        for dir in &dirs {
            if !discover::walk(dir, true, &mut collect).is_ok_and(|f| f.is_continue()) {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO_LIMIT: Duration = Duration::from_secs(60);

    fn tree() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let d = tmp.path();
        for dir in ["a/b", "sub", ".hidden"] {
            std::fs::create_dir_all(d.join(dir)).unwrap();
        }
        for f in [
            "top.cfg",
            "top.dat",
            "notes.txt",
            "a/x.cfg",
            "a/b/y.cff",
            "sub/z.txt",
            ".hidden/h.cfg",
        ] {
            std::fs::write(d.join(f), "").unwrap();
        }
        tmp
    }

    fn native(p: &str) -> String {
        p.replace('/', std::path::MAIN_SEPARATOR_STR)
    }

    #[test]
    fn offers_directories_then_recordings_below() {
        let tmp = tree();
        let d = tmp.path();
        let all = candidates("", d, None, 100, NO_LIMIT);
        let want: Vec<String> = ["a/", "sub/", "top.cfg", "a/x.cfg", "a/b/y.cff"].map(native).into();
        assert_eq!(all, want);

        // Only what continues the typed text; the directory is kept as typed.
        assert_eq!(
            candidates("a", d, None, 100, NO_LIMIT),
            ["a/", "a/x.cfg", "a/b/y.cff"].map(native)
        );
        assert_eq!(
            candidates("a/b", d, None, 100, NO_LIMIT),
            ["a/b/", "a/b/y.cff"].map(|p| {
                // The typed part keeps its `/`, the found part is native.
                format!("a/{}", native(&p[2..]))
            })
        );
        assert_eq!(candidates("t", d, None, 100, NO_LIMIT), ["top.cfg"]);
        assert!(candidates("nothing", d, None, 100, NO_LIMIT).is_empty());

        // Hidden directories (and what is in them) only when asked for.
        assert_eq!(
            candidates(".h", d, None, 100, NO_LIMIT),
            [".hidden/", ".hidden/h.cfg"].map(native)
        );

        // Absolute paths and `~`.
        let abs = format!("{}/a/", d.display());
        assert_eq!(
            candidates(&abs, Path::new("/"), None, 100, NO_LIMIT),
            [
                format!("{abs}b{MAIN_SEPARATOR}"),
                format!("{abs}x.cfg"),
                format!("{abs}{}", native("b/y.cff")),
            ]
        );
        assert_eq!(
            candidates("~/a/x", Path::new("/"), Some(d), 100, NO_LIMIT),
            ["~/a/x.cfg"]
        );
    }

    #[test]
    fn stops_at_the_limits() {
        let tmp = tree();
        let d = tmp.path();
        assert_eq!(
            candidates("", d, None, 1, NO_LIMIT),
            ["a/", "sub/", "top.cfg"].map(native)
        );
        // No time left: directories only.
        assert_eq!(candidates("", d, None, 100, Duration::ZERO), ["a/", "sub/"].map(native));
    }
}
