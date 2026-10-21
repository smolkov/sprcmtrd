//! Export of samples as CSV or JSON, for spreadsheets and scripts.
//!
//! An [`Export`] selects channels and samples of a loaded recording;
//! [`Export::write`] writes them in a [`Format`]. Analog values are scaled
//! and rounded to 12 significant digits, binary channels are 0 or 1.

use std::io::{self, Write};
use std::ops::Range;

use chrono::{NaiveDateTime, TimeDelta};
use serde::Serialize;

use super::{AnalogChannel, BinaryChannel, Comtrade, Side, round12 as round};

/// Output format of an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Comma separated values: a header row, then one row per sample.
    Csv,
    /// One JSON object with an array per column.
    Json,
}

/// Channels and samples of a recording to export.
#[derive(Debug, Clone)]
pub struct Export<'a> {
    /// Recording the channels belong to.
    pub recording: &'a Comtrade,
    /// Analog channels, in output order.
    pub analog: Vec<&'a AnalogChannel>,
    /// Binary channels, in output order.
    pub binary: Vec<&'a BinaryChannel>,
    /// Sample indices to export (limited to the recording).
    pub samples: Range<usize>,
    /// Keep every Nth sample of `samples` (plain picking, no filtering).
    pub every: usize,
    /// Side the analog values are converted to.
    pub side: Side,
    /// Date and time of day instead of ms relative to the trigger.
    pub absolute: bool,
}

impl<'a> Export<'a> {
    /// All channels and samples of `recording`, primary values, time in ms
    /// relative to the trigger.
    pub fn new(recording: &'a Comtrade) -> Self {
        Self {
            recording,
            analog: recording.analog_channels().collect(),
            binary: recording.binary_channels().collect(),
            samples: 0..recording.len(),
            every: 1,
            side: Side::Primary,
            absolute: false,
        }
    }

    /// Indices of the exported samples.
    pub fn rows(&self) -> impl Iterator<Item = usize> + '_ {
        let end = self.samples.end.min(self.recording.len());
        (self.samples.start.min(end)..end).step_by(self.every.max(1))
    }

    /// Number of exported samples.
    pub fn len(&self) -> usize {
        self.rows().count()
    }

    /// Returns `true` when no sample is exported.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Writes the export to `out`.
    pub fn write(&self, format: Format, out: &mut dyn Write) -> io::Result<()> {
        match format {
            Format::Csv => self.write_csv(out),
            Format::Json => {
                serde_json::to_writer(&mut *out, &self.json())?;
                writeln!(out)
            }
        }
    }

    /// CSV: header `sample,time_ms,<id> [<unit>]…,<binary id>…` (`time` with
    /// `absolute`); missing values are empty fields.
    fn write_csv(&self, out: &mut dyn Write) -> io::Result<()> {
        let mut header = vec![
            "sample".to_owned(),
            if self.absolute { "time" } else { "time_ms" }.to_owned(),
        ];
        for ch in &self.analog {
            header.push(csv_field(&format!("{} [{}]", ch.info.id, ch.info.unit)));
        }
        for ch in &self.binary {
            header.push(csv_field(&ch.info.id));
        }
        writeln!(out, "{}", header.join(","))?;

        let mut line = String::new();
        for row in self.rows() {
            line.clear();
            line.push_str(&(row + 1).to_string());
            line.push(',');
            line.push_str(&self.time(row));
            for ch in &self.analog {
                line.push(',');
                if let Some(v) = ch.value_on(row, self.side) {
                    line.push_str(&round(v).to_string());
                }
            }
            for ch in &self.binary {
                line.push_str(if ch.get(row) { ",1" } else { ",0" });
            }
            line.push('\n');
            out.write_all(line.as_bytes())?;
        }
        Ok(())
    }

    /// Time of sample `row`: ms relative to the trigger, or date and time.
    fn time(&self, row: usize) -> String {
        let t = self.recording.time()[row];
        if self.absolute {
            let ns = (t * 1e9).round() as i64;
            (self.recording.config.trigger_time + TimeDelta::nanoseconds(ns))
                .format("%Y-%m-%dT%H:%M:%S%.6f")
                .to_string()
        } else {
            round(t * 1e3).to_string()
        }
    }

    fn json(&self) -> JsonExport<'a> {
        let c = self.recording;
        let rows: Vec<usize> = self.rows().collect();
        let analog = self
            .analog
            .iter()
            .map(|ch| JsonAnalog {
                index: ch.info.index,
                id: &ch.info.id,
                phase: &ch.info.phase,
                circuit: &ch.info.circuit,
                unit: &ch.info.unit,
                values: rows.iter().map(|&r| ch.value_on(r, self.side).map(round)).collect(),
            })
            .collect();
        let binary = self
            .binary
            .iter()
            .map(|ch| JsonBinary {
                index: ch.info.index,
                id: &ch.info.id,
                phase: &ch.info.phase,
                circuit: &ch.info.circuit,
                values: rows.iter().map(|&r| u8::from(ch.get(r))).collect(),
            })
            .collect();
        JsonExport {
            station: &c.config.station_name,
            device: &c.config.device_id,
            revision: c.config.revision.to_string(),
            start_time: c.config.start_time,
            trigger_time: c.config.trigger_time,
            side: match self.side {
                Side::Primary => "primary",
                Side::Secondary => "secondary",
            },
            time_ms: (!self.absolute).then(|| rows.iter().map(|&r| round(c.time()[r] * 1e3)).collect()),
            time: self.absolute.then(|| rows.iter().map(|&r| self.time(r)).collect()),
            sample: rows.iter().map(|r| r + 1).collect(),
            analog,
            binary,
        }
    }
}

/// Quotes a CSV field when it contains a separator, quote or line break.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

#[derive(Serialize)]
struct JsonExport<'a> {
    station: &'a str,
    device: &'a str,
    revision: String,
    start_time: NaiveDateTime,
    trigger_time: NaiveDateTime,
    /// `primary` or `secondary`.
    side: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    time_ms: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    time: Option<Vec<String>>,
    sample: Vec<usize>,
    analog: Vec<JsonAnalog<'a>>,
    binary: Vec<JsonBinary<'a>>,
}

#[derive(Serialize)]
struct JsonAnalog<'a> {
    index: usize,
    id: &'a str,
    phase: &'a str,
    circuit: &'a str,
    unit: &'a str,
    values: Vec<Option<f64>>,
}

#[derive(Serialize)]
struct JsonBinary<'a> {
    index: usize,
    id: &'a str,
    phase: &'a str,
    circuit: &'a str,
    values: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_scaling_noise_and_quotes_fields() {
        assert_eq!(round(0.2 * 6.0).to_string(), "1.2");
        assert_eq!(round(-389.711).to_string(), "-389.711");
        assert_eq!(round(1e-9).to_string(), "0.000000001");
        assert_eq!(csv_field("IA [A]"), "IA [A]");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn rows_stay_within_the_recording() {
        let c = Comtrade::open(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/old_1991.cfg")).unwrap();
        let mut e = Export::new(&c);
        assert_eq!(e.len(), 100);
        e.samples = 90..500;
        e.every = 4;
        assert_eq!(e.rows().collect::<Vec<_>>(), [90, 94, 98]);
        e.samples = 200..300;
        assert!(e.is_empty());

        let mut csv = Vec::new();
        e.samples = 0..2;
        e.every = 1;
        e.analog.truncate(1);
        e.binary.clear();
        e.write(Format::Csv, &mut csv).unwrap();
        assert_eq!(
            String::from_utf8(csv).unwrap(),
            "sample,time_ms,IA [A]\n1,-30,0\n2,-29,124\n"
        );
    }
}
