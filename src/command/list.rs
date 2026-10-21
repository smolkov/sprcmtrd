//! `list` command: recordings in a directory.

use std::io::{IsTerminal, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Context;
use regex::Regex;

use super::{bytes, rate, table, time};
use crate::args::SortKey;
use crate::comtrade::discover::{self, Visit};
use crate::comtrade::{Comtrade, Config, RecordingPath};

struct Row {
    name: String,
    rec: RecordingPath,
    config: Result<Config, String>,
    size: Option<u64>,
}

/// Something the search thread found.
enum Found {
    /// A directory is being searched.
    Dir(PathBuf),
    /// A recording to list.
    Row(Box<Row>),
}

const HEADERS: [&str; 11] = [
    "file", "station", "rev", "format", "start", "duration", "rate", "#A", "#D", "size", "error",
];

/// Index of the error column.
const ERROR: usize = HEADERS.len() - 1;

/// Widths of the columns after the file name in rows printed as they are
/// found. The full table can't be measured in advance.
const LIVE_WIDTHS: [usize; 9] = [12, 4, 8, 19, 8, 8, 3, 3, 8];

const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Runs the `list` command. Subdirectories are searched when `recursive`.
/// With `valid`, only recordings that load completely (configuration and
/// sample data) are shown.
///
/// The search runs on its own thread. In a terminal without `sort`, rows are
/// printed as they are found; otherwise they are sorted (by start time when
/// `sort` is `None`) and printed as one table at the end. While searching, a
/// spinner is shown on stderr when it is a terminal.
pub fn run(
    dir: &Path,
    recursive: bool,
    filter: Option<&str>,
    valid: bool,
    sort: Option<SortKey>,
) -> anyhow::Result<()> {
    let filter = filter
        .map(Regex::new)
        .transpose()
        .context("invalid --filter regular expression")?;
    let live = sort.is_none() && std::io::stdout().is_terminal();
    let mut spinner = Spinner::new(std::io::stderr().is_terminal());
    let width = crossterm::terminal::size().map_or(usize::MAX, |(w, _)| w as usize);

    let (tx, rx) = mpsc::channel();
    let search = {
        let dir = dir.to_path_buf();
        std::thread::spawn(move || search(&dir, recursive, filter.as_ref(), valid, &tx))
    };

    let mut rows = Vec::new();
    let mut out = std::io::stdout().lock();
    loop {
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Found::Dir(d)) => {
                spinner.dir = d.strip_prefix(dir).unwrap_or(&d).display().to_string();
                // Messages can keep arriving faster than the timeout below.
                spinner.tick();
            }
            Ok(Found::Row(row)) if live => {
                spinner.clear();
                if spinner.found == 0 {
                    // Errors follow the file name, the error column is not used.
                    let mut header = HEADERS.map(String::from);
                    header[ERROR].clear();
                    writeln!(out, "{}", live_line(&header, width))?;
                }
                writeln!(out, "{}", live_line(&row_cells(&row, true), width))?;
                out.flush()?;
                spinner.found += 1;
                spinner.draw();
            }
            Ok(Found::Row(row)) => {
                rows.push(*row);
                spinner.found += 1;
                spinner.tick();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => spinner.tick(),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    spinner.clear();
    search.join().expect("search thread panicked")?;

    if spinner.found == 0 {
        let what = if valid {
            "valid COMTRADE recordings"
        } else {
            "COMTRADE recordings"
        };
        eprintln!("no {what} found in {}", dir.display());
        return Ok(());
    }
    if live {
        return Ok(());
    }

    match sort.unwrap_or(SortKey::Time) {
        SortKey::Name => rows.sort_by(|a, b| a.name.cmp(&b.name)),
        SortKey::Size => rows.sort_by_key(|r| std::cmp::Reverse(r.size)),
        SortKey::Time => rows.sort_by(|a, b| {
            let key = |r: &Row| r.config.as_ref().ok().map(|c| c.start_time);
            // Recordings that failed to parse go last.
            match (key(a), key(b)) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        }),
    }
    let table_rows: Vec<Vec<String>> = rows.iter().map(|r| row_cells(r, false)).collect();
    write!(out, "{}", table(&HEADERS, &table_rows))?;
    Ok(())
}

/// Walks `dir` and sends the directories entered and the recordings to list.
/// Stops when the receiver is gone.
fn search(
    dir: &Path,
    recursive: bool,
    filter: Option<&Regex>,
    valid: bool,
    tx: &mpsc::Sender<Found>,
) -> anyhow::Result<()> {
    let flow = |sent: Result<(), _>| match sent {
        Ok(()) => ControlFlow::Continue(()),
        Err(_) => ControlFlow::Break(()),
    };
    // Stopping early only happens when nobody is listening any more.
    let _ = discover::walk(dir, recursive, &mut |visit| match visit {
        Visit::Dir(d) => flow(tx.send(Found::Dir(d.to_path_buf()))),
        Visit::Recording(rec) => {
            if filter.is_some_and(|f| !f.is_match(&rec.name())) {
                return ControlFlow::Continue(());
            }
            let row = Row {
                name: rec
                    .main_file()
                    .strip_prefix(dir)
                    .unwrap_or(rec.main_file())
                    .display()
                    .to_string(),
                config: Comtrade::config_of(&rec).map_err(|e| e.to_string()),
                size: rec.data_size(),
                rec,
            };
            // A parsable configuration is not enough: the data must load too,
            // so the recording opens in the viewer.
            if valid && (row.config.is_err() || Comtrade::open_recording(&row.rec).is_err()) {
                return ControlFlow::Continue(());
            }
            flow(tx.send(Found::Row(Box::new(row))))
        }
    })?;
    Ok(())
}

/// Table cells of `r`. A missing data file is marked after the file name, or
/// for `live` rows (narrow file column) in the size or error cell.
fn row_cells(r: &Row, live: bool) -> Vec<String> {
    let no_data = !r.rec.has_data();
    let marker = if no_data && !live { " (!no data)" } else { "" };
    let size = match r.size {
        Some(n) => bytes(n),
        None if no_data && live => "no data".into(),
        None => "-".into(),
    };
    match &r.config {
        Ok(c) => vec![
            format!("{}{marker}", r.name),
            c.station_name.clone(),
            c.revision.to_string(),
            c.data_format.to_string(),
            c.start_time.format("%Y-%m-%d %H:%M:%S").to_string(),
            c.duration().map_or_else(|| "-".into(), |d| time(d, d)),
            c.sample_rates
                .first()
                .filter(|r| r.rate > 0.0)
                .map_or_else(|| "-".into(), |r| rate(r.rate)),
            c.analog.len().to_string(),
            c.binary.len().to_string(),
            size,
            String::new(),
        ],
        Err(e) => {
            let mut cells = vec![format!("{}{marker}", r.name)];
            cells.extend(std::iter::repeat_n("-".to_owned(), ERROR - 2));
            cells.push(size);
            cells.push(if no_data && live {
                format!("no data · {e}")
            } else {
                e.clone()
            });
            cells
        }
    }
}

/// Formats a row for a terminal `width` columns wide. The file column takes
/// what the others leave (within limits) and keeps the end of long paths. A
/// recording that failed to parse shows its error right after the file name
/// instead of empty columns. The line is cut at `width` so it never wraps; the
/// full text is in the sorted table.
fn live_line(cells: &[String], width: usize) -> String {
    let others: usize = LIVE_WIDTHS.iter().map(|w| w + 2).sum();
    let file_width = width.saturating_sub(others).clamp(18, 48);
    let mut s = String::new();
    push_cell(&mut s, &cells[0], file_width, true);
    match cells.get(ERROR).filter(|e| !e.is_empty()) {
        Some(error) => s.push_str(error),
        None => {
            for (cell, &w) in cells[1..].iter().zip(&LIVE_WIDTHS) {
                push_cell(&mut s, cell, w, false);
            }
        }
    }
    fit(s.trim_end(), width, false)
}

/// Appends `cell` fitted and padded to `width`, then the column gap.
fn push_cell(s: &mut String, cell: &str, width: usize, keep_end: bool) {
    let cell = fit(cell, width, keep_end);
    let pad = width.saturating_sub(cell.chars().count());
    s.push_str(&cell);
    s.extend(std::iter::repeat_n(' ', pad + 2));
}

/// Shortens `s` to `width` characters with `…`, cutting its start when
/// `keep_end`.
fn fit(s: &str, width: usize, keep_end: bool) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_owned();
    }
    let keep = width.saturating_sub(1);
    if keep_end {
        format!("…{}", s.chars().skip(n - keep).collect::<String>())
    } else {
        format!("{}…", s.chars().take(keep).collect::<String>())
    }
}

/// Search progress on one stderr line, redrawn in place.
struct Spinner {
    enabled: bool,
    frame: usize,
    last: Instant,
    found: usize,
    dir: String,
}

impl Spinner {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            frame: 0,
            last: Instant::now(),
            found: 0,
            dir: String::new(),
        }
    }

    /// Advances the animation, at most every 80 ms however often it's called.
    fn tick(&mut self) {
        if self.last.elapsed() >= Duration::from_millis(80) {
            self.last = Instant::now();
            self.frame += 1;
            self.draw();
        }
    }

    fn draw(&self) {
        if !self.enabled {
            return;
        }
        let dir = if self.dir.is_empty() { "." } else { &self.dir };
        let text = format!(
            "{} searching · {} found · {dir}",
            SPINNER[self.frame % SPINNER.len()],
            self.found
        );
        // A wrapped line could not be cleared with \r.
        let width = crossterm::terminal::size().map_or(80, |(w, _)| w as usize);
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r\x1b[2K{}", fit(&text, width.saturating_sub(1), false));
        let _ = err.flush();
    }

    fn clear(&self) {
        if self.enabled {
            let mut err = std::io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_lines_have_fixed_columns() {
        assert_eq!(fit("abc", 3, true), "abc");
        assert_eq!(fit("station/2024/fault.cfg", 10, true), "…fault.cfg");
        assert_eq!(fit("SUBSTATION NORTH 1", 10, false), "SUBSTATIO…");

        let row = |name: &str, error: &str, width: usize| {
            let mut cells = vec![name.to_owned()];
            cells.extend(
                [
                    "S",
                    "1999",
                    "BINARY32",
                    "2024-03-15 08:30:00",
                    "1.0 s",
                    "2 kHz",
                    "4",
                    "4",
                    "7.2 kB",
                ]
                .map(String::from),
            );
            cells.push(error.to_owned());
            live_line(&cells, width)
        };
        let short = row("a.cfg", "", 120);
        let long = row(&format!("{}/a.cfg", "x".repeat(60)), "", 120);
        // Columns after the file name line up whatever its length.
        assert_eq!(short.chars().count(), long.chars().count());
        assert!(long.starts_with('…') && long.contains("/a.cfg  S  "));
        assert!(short.ends_with("7.2 kB"));
        // A 110 column terminal shows the whole row.
        assert!(row("a.cfg", "", 110).ends_with("7.2 kB"));
        assert!(short.contains("  1999  BINARY32  2024-03-15"));
        // Errors follow the file name and never make the line wrap.
        let broken = row("broken.cfg", &"bad ".repeat(40), 80);
        assert!(broken.starts_with("broken.cfg   "));
        assert!(broken.contains("  bad bad"));
        assert_eq!(broken.chars().count(), 80);
    }

    #[test]
    fn live_rows_mark_missing_data_outside_the_file_column() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        let rec = RecordingPath::from_file(data.join("broken.cfg")).unwrap();
        let row = Row {
            name: "broken.cfg".into(),
            config: Comtrade::config_of(&rec).map_err(|e| e.to_string()),
            size: rec.data_size(),
            rec,
        };
        assert_eq!(row_cells(&row, false)[0], "broken.cfg (!no data)");
        let live = row_cells(&row, true);
        assert_eq!(live[0], "broken.cfg");
        assert!(live[ERROR].starts_with("no data · cfg line 2"));
    }

    #[test]
    fn search_sends_filtered_rows() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        let (tx, rx) = mpsc::channel();
        let filter = Regex::new("^fault").unwrap();
        search(&data, false, Some(&filter), false, &tx).unwrap();
        drop(tx);
        let mut names: Vec<String> = rx
            .iter()
            .filter_map(|f| match f {
                Found::Row(r) => Some(r.name),
                Found::Dir(_) => None,
            })
            .collect();
        names.sort();
        assert_eq!(names, ["fault_1999.cfg", "fault_2013.cfg"]);
    }
}
