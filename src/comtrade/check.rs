//! Validation of recordings: problems that prevent reading them correctly
//! (errors), data quality problems (warnings) and unusual content (notes).

use std::collections::HashMap;
use std::fmt;

use serde::Serialize;

use super::cfg::{Config, Revision};
use super::{Comtrade, RecordingPath};

/// How serious an [`Issue`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Unusual but allowed; does not affect the status.
    Note,
    /// The recording can be read, but something is wrong with its content.
    Warning,
    /// The recording can't be read (completely or correctly).
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Note => "note",
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
}

/// A problem found in a recording.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Issue {
    /// How serious the problem is.
    pub severity: Severity,
    /// Channel the problem belongs to (`A1 IA`, `D2 TRIP`), if any.
    pub channel: Option<String>,
    /// Description.
    pub message: String,
}

/// Result of checking one recording.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Problems found, most serious first.
    pub issues: Vec<Issue>,
    /// Number of samples, when the data could be read.
    pub samples: Option<usize>,
}

impl Report {
    /// The most serious severity found, `None` when there are no warnings
    /// or errors (notes don't count).
    pub fn status(&self) -> Option<Severity> {
        self.issues
            .iter()
            .map(|i| i.severity)
            .filter(|&s| s > Severity::Note)
            .max()
    }
}

/// Units that are common in power system recordings; others get a note.
const KNOWN_UNITS: &[&str] = &[
    "A", "kA", "mA", "V", "kV", "mV", "MV", "W", "kW", "MW", "var", "kvar", "Mvar", "VAr", "kVAr", "MVAr", "VA", "kVA",
    "MVA", "Hz", "Ohm", "ohm", "Ω", "deg", "°", "rad", "s", "ms", "%", "pu", "p.u.", "°C", "degC",
];

/// Checks the recording `rec`: its configuration first, then its data.
pub fn check(rec: &RecordingPath) -> Report {
    let mut issues = Vec::new();
    let mut samples = None;
    match Comtrade::config_of(rec) {
        Err(e) => issues.push(issue(Severity::Error, None, e.to_string())),
        Ok(config) => {
            check_config(&config, &mut issues);
            match Comtrade::open_recording(rec) {
                Err(e) => issues.push(issue(Severity::Error, None, e.to_string())),
                Ok(c) => {
                    samples = Some(c.len());
                    check_data(&c, &mut issues);
                }
            }
        }
    }
    // Most serious first, otherwise in the order found.
    issues.sort_by_key(|i| std::cmp::Reverse(i.severity));
    Report { issues, samples }
}

fn issue(severity: Severity, channel: Option<String>, message: impl Into<String>) -> Issue {
    Issue {
        severity,
        channel,
        message: message.into(),
    }
}

/// Checks that only need the configuration.
fn check_config(c: &Config, issues: &mut Vec<Issue>) {
    use Severity::*;
    let analog_numbers: Vec<usize> = c.analog.iter().map(|a| a.index).collect();
    if analog_numbers != (1..=c.analog.len()).collect::<Vec<_>>() {
        issues.push(issue(Warning, None, "analog channel numbers are not 1..n in order"));
    }
    let binary_numbers: Vec<usize> = c.binary.iter().map(|b| b.index).collect();
    if binary_numbers != (1..=c.binary.len()).collect::<Vec<_>>() {
        issues.push(issue(Warning, None, "binary channel numbers are not 1..n in order"));
    }

    let mut ids: HashMap<&str, String> = HashMap::new();
    let names = c
        .analog
        .iter()
        .map(|a| (a.id.as_str(), format!("A{}", a.index)))
        .chain(c.binary.iter().map(|b| (b.id.as_str(), format!("D{}", b.index))));
    for (id, name) in names {
        if id.is_empty() {
            issues.push(issue(Warning, Some(name), "channel has no id"));
        } else if let Some(first) = ids.get(id) {
            issues.push(issue(Warning, Some(name), format!("id {id} is also used by {first}")));
        } else {
            ids.insert(id, name);
        }
    }

    for a in &c.analog {
        let name = Some(format!("A{} {}", a.index, a.id));
        if a.unit.is_empty() {
            issues.push(issue(Warning, name.clone(), "no unit"));
        } else if !KNOWN_UNITS.contains(&a.unit.as_str()) {
            issues.push(issue(Note, name.clone(), format!("unusual unit '{}'", a.unit)));
        }
        if a.a == 0.0 {
            issues.push(issue(
                Warning,
                name.clone(),
                "multiplier a is 0, all values are the offset b",
            ));
        }
        if c.revision != Revision::Rev1991 && (a.primary == 0.0 || a.secondary == 0.0) {
            issues.push(issue(
                Warning,
                name.clone(),
                "transformer ratio is 0, primary / secondary values can't be converted",
            ));
        }
        if a.min.is_finite() && a.max.is_finite() && a.min > a.max {
            issues.push(issue(Warning, name, format!("min {} is above max {}", a.min, a.max)));
        }
    }

    if c.total_samples() == 0 {
        issues.push(issue(Warning, None, "the configuration declares no sample count"));
    }
    if c.trigger_offset() < 0.0 || c.duration().is_some_and(|d| c.trigger_offset() > d) {
        issues.push(issue(Warning, None, "the trigger time lies outside the recording"));
    }
    if ![50.0, 60.0, 16.7, 0.0]
        .iter()
        .any(|f| (c.line_frequency - f).abs() < 0.05)
    {
        issues.push(issue(
            Note,
            None,
            format!("unusual line frequency {} Hz", c.line_frequency),
        ));
    }
}

/// Checks of the sample data.
fn check_data(c: &Comtrade, issues: &mut Vec<Issue>) {
    use Severity::*;
    if let Some(i) = c.time().windows(2).position(|w| w[1] <= w[0]) {
        issues.push(issue(
            Warning,
            None,
            format!("time does not increase at sample {} (timestamps out of order)", i + 2),
        ));
    }
    for ch in c.analog_channels() {
        let a = &ch.info;
        let name = Some(format!("A{} {}", a.index, a.id));
        let samples = ch.samples();
        let (mut missing, mut first_missing) = (0, None);
        let (mut outside, mut at_limit) = (0, 0);
        for i in 0..samples.len() {
            let Some(raw) = samples.raw(i) else {
                missing += 1;
                first_missing.get_or_insert(i + 1);
                continue;
            };
            if a.min.is_finite() && a.max.is_finite() && a.min < a.max {
                if raw < a.min || raw > a.max {
                    outside += 1;
                } else if raw == a.min || raw == a.max {
                    at_limit += 1;
                }
            }
        }
        if let Some(first) = first_missing {
            issues.push(issue(
                Warning,
                name.clone(),
                format!(
                    "{missing} missing {} (first at sample {first})",
                    plural(missing, "value")
                ),
            ));
        }
        if outside > 0 {
            issues.push(issue(
                Warning,
                name.clone(),
                format!(
                    "{outside} raw {} outside the declared range {} .. {}",
                    plural(outside, "value"),
                    a.min,
                    a.max
                ),
            ));
        }
        if at_limit > 0 {
            issues.push(issue(
                Warning,
                name,
                format!(
                    "{at_limit} raw {} at the declared limit {} / {} (clipped?)",
                    plural(at_limit, "value"),
                    a.min,
                    a.max
                ),
            ));
        }
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 { word.to_owned() } else { format!("{word}s") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(name: &str) -> RecordingPath {
        RecordingPath::from_file(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn sample_recordings() {
        let r = check(&data("fault_2013.cfg"));
        assert_eq!(r.status(), None, "{:?}", r.issues);
        assert_eq!(r.samples, Some(4001));

        let r = check(&data("broken.cfg"));
        assert_eq!(r.status(), Some(Severity::Error));
        assert!(r.issues[0].message.contains("channel count 5 does not match"));
        assert_eq!(r.samples, None);

        // 99999 marks a missing value in the 1991 data file.
        let r = check(&data("old_1991.cfg"));
        assert_eq!(r.status(), Some(Severity::Warning));
        let missing = r.issues.iter().find(|i| i.message.contains("missing")).unwrap();
        assert_eq!(missing.channel.as_deref(), Some("A1 IA"));
        assert_eq!(missing.message, "1 missing value (first at sample 51)");
    }

    #[test]
    fn configuration_and_data_problems() {
        let dir = tempfile::tempdir().unwrap();
        // Channel numbers 1, 3; duplicate id; no unit; unusual unit; clipped
        // and out of range values; trigger after the end; 55 Hz.
        let cfg = "S,D,1999\n3,2A,1D\n1,IA,A,,,1,0,0,-10,10,1,1,P\n3,IA,B,,furlong,1,0,0,-10,10,0,1,P\n\
1,T,,,0\n55\n1\n1000,3\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:01.000000\nASCII\n1\n";
        let dat = "1,0,10,1,0\n2,1000,20,2,0\n3,2000,-10,3,1\n";
        std::fs::write(dir.path().join("x.cfg"), cfg).unwrap();
        std::fs::write(dir.path().join("x.dat"), dat).unwrap();
        let r = check(&RecordingPath::from_file(dir.path().join("x.cfg")).unwrap());
        let text: Vec<String> = r
            .issues
            .iter()
            .map(|i| format!("{} {} {}", i.severity, i.channel.as_deref().unwrap_or("-"), i.message))
            .collect();
        for want in [
            "warning - analog channel numbers are not 1..n in order",
            "warning A3 id IA is also used by A1",
            "warning A1 IA no unit",
            "note A3 IA unusual unit 'furlong'",
            "warning A3 IA transformer ratio is 0, primary / secondary values can't be converted",
            "warning - the trigger time lies outside the recording",
            "note - unusual line frequency 55 Hz",
            "warning A1 IA 1 raw value outside the declared range -10 .. 10",
            "warning A1 IA 2 raw values at the declared limit -10 / 10 (clipped?)",
        ] {
            assert!(text.iter().any(|t| t == want), "missing '{want}' in {text:#?}");
        }
        assert_eq!(r.status(), Some(Severity::Warning));
        // Notes come last.
        assert_eq!(r.issues.last().unwrap().severity, Severity::Note);
    }
}
