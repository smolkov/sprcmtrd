//! `check` command: validates one recording or all recordings in a directory.

use std::ops::ControlFlow;
use std::path::Path;

use serde::Serialize;

use super::{Context, table};
use crate::comtrade::check::{self, Report, Severity};
use crate::comtrade::discover::{self, RecordingPath, Visit};
use crate::term::{Palette, Role, Spinner};

/// Returned when a recording failed the check, so `main` exits with the
/// code for invalid content (4).
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Failed(String);

#[derive(Serialize)]
struct JsonReport<'a> {
    file: String,
    /// `ok`, `warning` or `error`.
    status: &'static str,
    #[serde(flatten)]
    report: &'a Report,
}

/// Runs the `check` command on a recording file, or on all recordings in a
/// directory (and its subdirectories when `recursive`). Fails when a
/// recording has an error, or a warning with `strict`.
pub fn run(ctx: &Context, path: &Path, recursive: bool, strict: bool, json: bool) -> anyhow::Result<()> {
    // Searching and checking a large directory takes a while: show progress.
    let mut spinner = Spinner::stderr();
    let recordings: Vec<(String, RecordingPath)> = if path.is_dir() {
        let mut found = Vec::new();
        // The callback never breaks, so the flow is always Continue.
        let _ = discover::walk(path, recursive, &mut |visit| {
            if let Visit::Recording(rec) = visit {
                let name = rec.main_file().strip_prefix(path).unwrap_or(rec.main_file());
                found.push((name.display().to_string(), rec));
            }
            spinner.set(format!("searching · {} found", found.len()));
            spinner.tick();
            ControlFlow::Continue(())
        })?;
        found
    } else {
        let rec = RecordingPath::from_file(path)?;
        vec![(rec.main_file().display().to_string(), rec)]
    };
    let total = recordings.len();
    let mut reports: Vec<(String, Report)> = Vec::with_capacity(total);
    for (i, (name, rec)) in recordings.into_iter().enumerate() {
        spinner.set(format!("checking {}/{total} · {name}", i + 1));
        spinner.tick();
        reports.push((name, check::check(&rec)));
    }
    spinner.clear();
    let count = |s: Option<Severity>| reports.iter().filter(|(_, r)| r.status() == s).count();
    let (ok, warnings, errors) = (
        count(None),
        count(Some(Severity::Warning)),
        count(Some(Severity::Error)),
    );

    if json {
        let out: Vec<JsonReport> = reports
            .iter()
            .map(|(file, report)| JsonReport {
                file: file.clone(),
                status: status_name(report.status()),
                report,
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        let palette = Palette::stdout(ctx.theme(), ctx.global.colors_disabled());
        print!("{}", text(&reports, &palette));
        if path.is_dir() && !reports.is_empty() {
            let n = reports.len();
            let recordings = if n == 1 { "recording" } else { "recordings" };
            // Counts above zero in their color.
            let count = |n: usize, what: &str, role: Role| {
                let s = format!("{n} {what}");
                if n > 0 { palette.paint(&s, role) } else { s }
            };
            println!(
                "\n{n} {recordings}: {}, {}, {}",
                count(ok, "ok", Role::Ok),
                count(warnings, "with warnings", Role::Warning),
                count(errors, "with errors", Role::Error)
            );
        }
    }

    if reports.is_empty() {
        eprintln!("no COMTRADE recordings found in {}", path.display());
    }
    if errors > 0 || (strict && warnings > 0) {
        let what = if errors > 0 {
            format!("{errors} of {} with errors", reports.len())
        } else {
            format!("{warnings} of {} with warnings (--strict)", reports.len())
        };
        return Err(Failed(format!("check failed: {what}")).into());
    }
    Ok(())
}

fn status_name(s: Option<Severity>) -> &'static str {
    match s {
        None => "ok",
        Some(Severity::Note) => unreachable!("notes don't set a status"),
        Some(Severity::Warning) => "warning",
        Some(Severity::Error) => "error",
    }
}

/// Status label with a symbol, so it reads without color, and its color.
fn label(severity: Option<Severity>) -> (&'static str, Role) {
    match severity {
        None => ("✓ OK", Role::Ok),
        Some(Severity::Note) => ("i NOTE", Role::Note),
        Some(Severity::Warning) => ("! WARNING", Role::Warning),
        Some(Severity::Error) => ("✗ ERROR", Role::Error),
    }
}

/// A table with one row per finding (one `OK` row for a recording without
/// findings); the file is named on its first row only.
fn text(reports: &[(String, Report)], palette: &Palette) -> String {
    let mut rows = Vec::new();
    let mut roles = Vec::new();
    for (file, report) in reports {
        if report.issues.is_empty() {
            let (text, role) = label(None);
            rows.push(vec![text.to_owned(), file.clone(), String::new(), String::new()]);
            roles.push(role);
        }
        for (n, i) in report.issues.iter().enumerate() {
            let (text, role) = label(Some(i.severity));
            rows.push(vec![
                text.to_owned(),
                if n == 0 { file.clone() } else { String::new() },
                i.channel.clone().unwrap_or_default(),
                i.message.clone(),
            ]);
            roles.push(role);
        }
    }
    let plain = table(&["status", "file", "channel", "finding"], &rows);
    // Color after aligning: escape codes would count as width. Each line
    // starts with its status label.
    let mut lines = plain.lines();
    let mut out = match lines.next() {
        Some(header) => palette.paint(header, Role::Header) + "\n",
        None => return String::new(),
    };
    for ((line, row), role) in lines.zip(&rows).zip(roles) {
        let rest = &line[row[0].len()..];
        out += &format!("{}{rest}\n", palette.paint(&row[0], role));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comtrade::check::Issue;

    #[test]
    fn one_row_per_finding() {
        let issue = |severity, channel: Option<&str>, message: &str| Issue {
            severity,
            channel: channel.map(str::to_owned),
            message: message.to_owned(),
        };
        let reports = vec![
            (
                "a.cfg".to_owned(),
                Report {
                    issues: vec![
                        issue(Severity::Error, None, "broken"),
                        issue(Severity::Note, Some("A1 IA"), "odd unit"),
                    ],
                    samples: None,
                },
            ),
            (
                "b.cfg".to_owned(),
                Report {
                    issues: vec![],
                    samples: Some(1),
                },
            ),
        ];
        assert_eq!(
            text(&reports, &Palette::plain()),
            "status   file   channel  finding\n\
             ✗ ERROR  a.cfg           broken\n\
             i NOTE          A1 IA    odd unit\n\
             ✓ OK     b.cfg\n"
        );
    }
}
