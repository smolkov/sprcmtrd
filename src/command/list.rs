//! `list` command: recordings in a directory.

use std::io::{IsTerminal, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::Context;
use chrono::NaiveDateTime;
use regex::{Regex, RegexBuilder};

use super::{bytes, rate, table, time};
use crate::args::SortKey;
use crate::comtrade::discover::{self, Visit};
use crate::comtrade::{Comtrade, Config, RecordingPath};
use crate::term::{self, LiveTable, Spinner};

struct Row {
    name: String,
    rec: RecordingPath,
    config: Result<Config, String>,
    size: Option<u64>,
}

/// Which recordings `list` shows. The content filters (station, time,
/// duration) need the configuration, so recordings that fail to parse are
/// left out when one of them is set.
#[derive(Debug, Clone, Default)]
pub struct Filters {
    /// Regular expression on the file name.
    pub name: Option<Regex>,
    /// Regular expression on the station name.
    pub station: Option<Regex>,
    /// Earliest start time.
    pub since: Option<NaiveDateTime>,
    /// Latest start time.
    pub until: Option<NaiveDateTime>,
    /// Shortest duration in s; recordings without a known duration are left out.
    pub min_duration: Option<f64>,
    /// Longest duration in s; recordings without a known duration are left out.
    pub max_duration: Option<f64>,
    /// Only recordings that load completely (configuration and data).
    pub valid: bool,
}

impl Filters {
    /// Whether a filter on the configuration's content is set.
    fn by_content(&self) -> bool {
        self.station.is_some()
            || self.since.is_some()
            || self.until.is_some()
            || self.min_duration.is_some()
            || self.max_duration.is_some()
    }

    /// Whether the configuration `c` passes the content filters.
    fn matches(&self, c: &Config) -> bool {
        let duration = c.duration();
        self.station.as_ref().is_none_or(|re| re.is_match(&c.station_name))
            && self.since.is_none_or(|t| c.start_time >= t)
            && self.until.is_none_or(|t| c.start_time <= t)
            && self.min_duration.is_none_or(|d| duration.is_some_and(|x| x >= d))
            && self.max_duration.is_none_or(|d| duration.is_some_and(|x| x <= d))
    }
}

/// Compiles the regular expression of the option `option`, if given.
pub fn regex(pattern: Option<&str>, option: &str, case_insensitive: bool) -> anyhow::Result<Option<Regex>> {
    pattern
        .map(|p| RegexBuilder::new(p).case_insensitive(case_insensitive).build())
        .transpose()
        .with_context(|| format!("invalid {option} regular expression"))
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
/// found (station … size); the file column takes 18 to 48 characters.
const LIVE_WIDTHS: [usize; 9] = [12, 4, 8, 19, 8, 8, 3, 3, 8];

/// Runs the `list` command. Subdirectories are searched when `recursive`;
/// only recordings that pass `filters` are shown.
///
/// The search runs on its own thread. In a terminal without `sort`, rows are
/// printed as they are found; otherwise they are sorted (by start time when
/// `sort` is `None`) and printed as one table at the end. While searching, a
/// spinner is shown on stderr when it is a terminal.
pub fn run(dir: &Path, recursive: bool, filters: Filters, sort: Option<SortKey>) -> anyhow::Result<()> {
    let what = if filters.by_content() || filters.name.is_some() {
        "COMTRADE recordings matching the filters"
    } else if filters.valid {
        "valid COMTRADE recordings"
    } else {
        "COMTRADE recordings"
    };
    let live = sort.is_none() && std::io::stdout().is_terminal();
    let table_live = LiveTable::new(&LIVE_WIDTHS, 18..=48, term::terminal_width());
    let mut spinner = Spinner::stderr();
    let mut found = 0;
    let mut current = String::from(".");

    let (tx, rx) = mpsc::channel();
    let search = {
        let dir = dir.to_path_buf();
        std::thread::spawn(move || search(&dir, recursive, &filters, &tx))
    };

    let mut rows = Vec::new();
    let mut out = std::io::stdout().lock();
    loop {
        // A printed row cleared the spinner; draw it again right away.
        let mut redraw = false;
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Found::Dir(d)) => {
                current = d.strip_prefix(dir).unwrap_or(&d).display().to_string();
                if current.is_empty() {
                    current = ".".into();
                }
            }
            Ok(Found::Row(row)) if live => {
                spinner.clear();
                if found == 0 {
                    // Errors follow the file name, the error column is not used.
                    writeln!(out, "{}", table_live.row(&HEADERS[..ERROR]))?;
                }
                writeln!(out, "{}", live_line(&table_live, &row_cells(&row, true)))?;
                out.flush()?;
                found += 1;
                redraw = true;
            }
            Ok(Found::Row(row)) => {
                rows.push(*row);
                found += 1;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        spinner.set(format!("searching · {found} found · {current}"));
        if redraw {
            spinner.draw();
        } else {
            // Also when messages keep arriving faster than the timeout above.
            spinner.tick();
        }
    }
    spinner.clear();
    search.join().expect("search thread panicked")?;

    if found == 0 {
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
fn search(dir: &Path, recursive: bool, filters: &Filters, tx: &mpsc::Sender<Found>) -> anyhow::Result<()> {
    let flow = |sent: Result<(), _>| match sent {
        Ok(()) => ControlFlow::Continue(()),
        Err(_) => ControlFlow::Break(()),
    };
    // Stopping early only happens when nobody is listening any more.
    let _ = discover::walk(dir, recursive, &mut |visit| match visit {
        Visit::Dir(d) => flow(tx.send(Found::Dir(d.to_path_buf()))),
        Visit::Recording(rec) => {
            if filters.name.as_ref().is_some_and(|f| !f.is_match(&rec.name())) {
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
            if filters.by_content() && !row.config.as_ref().is_ok_and(|c| filters.matches(c)) {
                return ControlFlow::Continue(());
            }
            // A parsable configuration is not enough: the data must load too,
            // so the recording opens in the viewer.
            if filters.valid && (row.config.is_err() || Comtrade::open_recording(&row.rec).is_err()) {
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

/// A live row of `cells`. A recording that failed to parse shows its error
/// right after the file name instead of empty columns; the full text is in
/// the sorted table.
fn live_line(table: &LiveTable, cells: &[String]) -> String {
    match cells.get(ERROR).filter(|e| !e.is_empty()) {
        Some(error) => table.row_with_text(&cells[0], error),
        None => table.row(&cells[..ERROR]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_lines_have_fixed_columns() {
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
            live_line(&LiveTable::new(&LIVE_WIDTHS, 18..=48, Some(width)), &cells)
        };
        let short = row("a.cfg", "", 120);
        assert!(short.ends_with("7.2 kB"));
        assert!(short.contains("  1999  BINARY32  2024-03-15"));
        // A 110 column terminal shows the whole row.
        assert!(row("a.cfg", "", 110).ends_with("7.2 kB"));
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
        let filters = Filters {
            name: Some(Regex::new("^fault").unwrap()),
            ..Filters::default()
        };
        search(&data, false, &filters, &tx).unwrap();
        drop(tx);
        let mut names: Vec<String> = rx
            .iter()
            .filter_map(|f| match f {
                Found::Row(r) => Some(r.name),
                Found::Dir(_) => None,
            })
            .collect();
        names.sort();
        assert_eq!(names, ["fault_1999.cfg", "fault_2013.cfg", "fault_2013_uppercase.cfg"]);
    }
}
