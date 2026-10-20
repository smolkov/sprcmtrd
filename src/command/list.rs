//! `list` command: recordings in a directory.

use std::path::Path;

use anyhow::Context;
use regex::Regex;

use super::{bytes, rate, table, time};
use crate::args::SortKey;
use crate::comtrade::{Comtrade, Config, RecordingPath, discover};

struct Row {
    name: String,
    rec: RecordingPath,
    config: Result<Config, String>,
    size: Option<u64>,
}

/// Runs the `list` command.
pub fn run(dir: &Path, recursive: bool, filter: Option<&str>, sort: SortKey) -> anyhow::Result<()> {
    let filter = filter
        .map(Regex::new)
        .transpose()
        .context("invalid --filter regular expression")?;
    let mut rows: Vec<Row> = discover::find_recordings(dir, recursive)?
        .into_iter()
        .filter(|r| filter.as_ref().is_none_or(|f| f.is_match(&r.name())))
        .map(|rec| Row {
            name: rec
                .main_file()
                .strip_prefix(dir)
                .unwrap_or(rec.main_file())
                .display()
                .to_string(),
            config: Comtrade::config_of(&rec).map_err(|e| e.to_string()),
            size: rec.data_size(),
            rec,
        })
        .collect();

    if rows.is_empty() {
        eprintln!("no COMTRADE recordings found in {}", dir.display());
        return Ok(());
    }

    match sort {
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

    let headers = [
        "file", "station", "rev", "start", "duration", "rate", "#A", "#D", "size", "error",
    ];
    let table_rows: Vec<Vec<String>> = rows.iter().map(row_cells).collect();
    print!("{}", table(&headers, &table_rows));
    Ok(())
}

fn row_cells(r: &Row) -> Vec<String> {
    let marker = if r.rec.has_data() { "" } else { " (!no data)" };
    let size = r.size.map(bytes).unwrap_or_else(|| "-".into());
    match &r.config {
        Ok(c) => vec![
            format!("{}{marker}", r.name),
            c.station_name.clone(),
            c.revision.to_string(),
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
            cells.extend(std::iter::repeat_n("-".to_owned(), 7));
            cells.push(size);
            cells.push(e.clone());
            cells
        }
    }
}
