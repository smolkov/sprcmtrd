//! Parser for COMTRADE configuration (`.cfg`) files, revisions 1991, 1999 and 2013.

use std::fmt;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;

use crate::error::{Error, Result};

/// COMTRADE standard revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Revision {
    /// IEEE C37.111-1991 (no revision year in the file).
    Rev1991,
    /// IEEE C37.111-1999.
    Rev1999,
    /// IEEE C37.111-2013 / IEC 60255-24:2013.
    Rev2013,
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Rev1991 => "1991",
            Self::Rev1999 => "1999",
            Self::Rev2013 => "2013",
        })
    }
}

/// Encoding of the sample data file (`ft` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DataFormat {
    /// Comma separated text.
    Ascii,
    /// Little endian, 16-bit signed analog values.
    Binary,
    /// Little endian, 32-bit signed analog values.
    Binary32,
    /// Little endian, 32-bit IEEE float analog values.
    Float32,
}

impl DataFormat {
    /// Size of one analog value in a binary record, `None` for ASCII.
    pub fn analog_size(self) -> Option<usize> {
        match self {
            Self::Ascii => None,
            Self::Binary => Some(2),
            Self::Binary32 | Self::Float32 => Some(4),
        }
    }
}

impl fmt::Display for DataFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ascii => "ASCII",
            Self::Binary => "BINARY",
            Self::Binary32 => "BINARY32",
            Self::Float32 => "FLOAT32",
        })
    }
}

/// Whether channel values are expressed on the primary or the secondary side
/// of the instrument transformer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Side {
    /// Primary (network) side.
    Primary,
    /// Secondary (relay) side.
    Secondary,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Primary => "P",
            Self::Secondary => "S",
        })
    }
}

/// One sampling rate segment.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SampleRate {
    /// Sample rate in Hz; `0` means timestamps in the data file must be used.
    pub rate: f64,
    /// Number (1-based) of the last sample recorded with this rate.
    pub last_sample: usize,
}

/// Metadata of an analog channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalogInfo {
    /// Channel index `An` from the file.
    pub index: usize,
    /// Channel identifier.
    pub id: String,
    /// Phase identification.
    pub phase: String,
    /// Circuit component being monitored.
    pub circuit: String,
    /// Unit of the scaled value.
    pub unit: String,
    /// Multiplier: value = `a * raw + b`.
    pub a: f64,
    /// Offset: value = `a * raw + b`.
    pub b: f64,
    /// Time skew between channels in µs.
    pub skew: f64,
    /// Minimum raw data value.
    pub min: f64,
    /// Maximum raw data value.
    pub max: f64,
    /// Transformer primary ratio factor.
    pub primary: f64,
    /// Transformer secondary ratio factor.
    pub secondary: f64,
    /// Side the scaled value refers to.
    pub side: Side,
}

impl AnalogInfo {
    /// Factor that converts a scaled value into the requested `side`.
    pub fn side_factor(&self, side: Side) -> f64 {
        if side == self.side || self.primary == 0.0 || self.secondary == 0.0 {
            return 1.0;
        }
        match side {
            Side::Primary => self.primary / self.secondary,
            Side::Secondary => self.secondary / self.primary,
        }
    }
}

/// Metadata of a binary (status / digital) channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BinaryInfo {
    /// Channel index `Dn` from the file.
    pub index: usize,
    /// Channel identifier.
    pub id: String,
    /// Phase identification.
    pub phase: String,
    /// Circuit component being monitored.
    pub circuit: String,
    /// Normal (steady) state of the channel.
    pub normal_state: bool,
}

/// Parsed configuration file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Config {
    /// Station name.
    pub station_name: String,
    /// Recording device identification.
    pub device_id: String,
    /// Standard revision.
    pub revision: Revision,
    /// Nominal line frequency in Hz.
    pub line_frequency: f64,
    /// Sample rate segments (at least one).
    pub sample_rates: Vec<SampleRate>,
    /// Time of the first sample.
    pub start_time: NaiveDateTime,
    /// Time of the trigger point.
    pub trigger_time: NaiveDateTime,
    /// Encoding of the data file.
    pub data_format: DataFormat,
    /// Multiplier for timestamps in the data file.
    pub time_mult: f64,
    /// Time code (2013).
    pub time_code: Option<String>,
    /// Local code (2013).
    pub local_code: Option<String>,
    /// Time quality code (2013).
    pub tmq_code: Option<String>,
    /// Leap second indicator (2013).
    pub leap_second: Option<String>,
    /// Analog channels.
    pub analog: Vec<AnalogInfo>,
    /// Binary channels.
    pub binary: Vec<BinaryInfo>,
}

impl Config {
    /// Parses configuration text.
    pub fn parse(text: &str) -> Result<Self> {
        Parser::new(text).parse()
    }

    /// Total number of samples declared by the sample rate segments.
    pub fn total_samples(&self) -> usize {
        self.sample_rates.last().map_or(0, |r| r.last_sample)
    }

    /// Returns `true` when all segments have a non-zero sample rate, so time
    /// can be computed from the sample number.
    pub fn has_fixed_rates(&self) -> bool {
        self.sample_rates.iter().all(|r| r.rate > 0.0)
    }

    /// Duration of the recording in seconds computed from the sample rates;
    /// `None` when timestamps must be used.
    pub fn duration(&self) -> Option<f64> {
        if !self.has_fixed_rates() {
            return None;
        }
        let mut prev = 0;
        let mut secs = 0.0;
        for r in &self.sample_rates {
            secs += r.last_sample.saturating_sub(prev) as f64 / r.rate;
            prev = r.last_sample;
        }
        // N samples span N-1 intervals; the segment sum counts N.
        Some((secs - 1.0 / self.sample_rates[0].rate).max(0.0))
    }

    /// Seconds from the first sample to the trigger.
    pub fn trigger_offset(&self) -> f64 {
        let d = self.trigger_time - self.start_time;
        d.num_microseconds()
            .map_or(d.num_seconds() as f64, |us| us as f64 * 1e-6)
    }

    /// Size in bytes of one record in a binary data file, `None` for ASCII.
    pub fn record_size(&self) -> Option<usize> {
        let size = self.data_format.analog_size()?;
        Some(8 + self.analog.len() * size + self.binary.len().div_ceil(16) * 2)
    }
}

struct Parser<'a> {
    lines: std::iter::Peekable<std::iter::Enumerate<std::str::Lines<'a>>>,
    line_no: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.lines().enumerate().peekable(),
            line_no: 0,
        }
    }

    fn err(&self, message: impl Into<String>) -> Error {
        Error::Cfg {
            line: self.line_no,
            message: message.into(),
        }
    }

    fn next_line(&mut self, what: &str) -> Result<Vec<&'a str>> {
        match self.lines.next() {
            Some((i, line)) => {
                self.line_no = i + 1;
                Ok(split_fields(line))
            }
            None => {
                self.line_no += 1;
                Err(self.err(format!("unexpected end of file, expected {what}")))
            }
        }
    }

    /// Next line if it exists and is not empty.
    fn optional_line(&mut self) -> Option<Vec<&'a str>> {
        let (i, line) = self.lines.next()?;
        self.line_no = i + 1;
        (!line.trim().is_empty()).then(|| split_fields(line))
    }

    fn parse(mut self) -> Result<Config> {
        let f = self.next_line("station line")?;
        let station_name = field(&f, 0).to_owned();
        let device_id = field(&f, 1).to_owned();
        let revision = match field(&f, 2) {
            "" => Revision::Rev1991,
            y => match y.parse::<u32>() {
                Ok(y) if y < 1999 => Revision::Rev1991,
                Ok(y) if y < 2013 => Revision::Rev1999,
                Ok(_) => Revision::Rev2013,
                Err(_) => return Err(self.err(format!("invalid revision year '{y}'"))),
            },
        };

        let f = self.next_line("channel count line")?;
        let total: usize = self.num(field(&f, 0), "total channel count")?;
        let n_analog: usize = self.count(field(&f, 1), 'A')?;
        let n_digital: usize = self.count(field(&f, 2), 'D')?;
        if total != n_analog + n_digital {
            return Err(self.err(format!(
                "channel count {total} does not match {n_analog}A + {n_digital}D"
            )));
        }

        let mut analog = Vec::with_capacity(n_analog);
        for _ in 0..n_analog {
            let f = self.next_line("analog channel line")?;
            analog.push(self.analog(&f)?);
        }
        let mut binary = Vec::with_capacity(n_digital);
        for _ in 0..n_digital {
            let f = self.next_line("binary channel line")?;
            binary.push(self.binary(&f)?);
        }

        let f = self.next_line("line frequency")?;
        let line_frequency = self.num_or(field(&f, 0), 0.0, "line frequency")?;

        let f = self.next_line("number of sample rates")?;
        let nrates: usize = self.num(field(&f, 0), "number of sample rates")?;
        let mut sample_rates = Vec::with_capacity(nrates.max(1));
        for _ in 0..nrates.max(1) {
            let f = self.next_line("sample rate line")?;
            let rate = self.num_or(field(&f, 0), 0.0, "sample rate")?;
            let last_sample = self.num(field(&f, 1), "last sample number")?;
            if let Some(prev) = sample_rates.last().map(|r: &SampleRate| r.last_sample)
                && last_sample < prev
            {
                return Err(self.err("last sample numbers must increase"));
            }
            sample_rates.push(SampleRate { rate, last_sample });
        }

        let f = self.next_line("start time")?;
        let start_time = self.datetime(&f, revision)?;
        let f = self.next_line("trigger time")?;
        let trigger_time = self.datetime(&f, revision)?;

        let f = self.next_line("data file type")?;
        let data_format = match field(&f, 0).to_ascii_uppercase().as_str() {
            "ASCII" => DataFormat::Ascii,
            "BINARY" => DataFormat::Binary,
            "BINARY32" => DataFormat::Binary32,
            "FLOAT32" => DataFormat::Float32,
            other => return Err(self.err(format!("unknown data file type '{other}'"))),
        };

        let mut time_mult = 1.0;
        let (mut time_code, mut local_code, mut tmq_code, mut leap_second) = (None, None, None, None);
        if let Some(f) = self.optional_line() {
            time_mult = self.num_or(field(&f, 0), 1.0, "time multiplier")?;
            if time_mult <= 0.0 {
                time_mult = 1.0;
            }
        }
        if let Some(f) = self.optional_line() {
            time_code = non_empty(field(&f, 0));
            local_code = non_empty(field(&f, 1));
        }
        if let Some(f) = self.optional_line() {
            tmq_code = non_empty(field(&f, 0));
            leap_second = non_empty(field(&f, 1));
        }

        Ok(Config {
            station_name,
            device_id,
            revision,
            line_frequency,
            sample_rates,
            start_time,
            trigger_time,
            data_format,
            time_mult,
            time_code,
            local_code,
            tmq_code,
            leap_second,
            analog,
            binary,
        })
    }

    fn analog(&self, f: &[&str]) -> Result<AnalogInfo> {
        if f.len() < 6 {
            return Err(self.err("analog channel line needs at least 6 fields"));
        }
        let side = match field(f, 12).to_ascii_uppercase().as_str() {
            "S" => Side::Secondary,
            _ => Side::Primary,
        };
        Ok(AnalogInfo {
            index: self.num(field(f, 0), "analog channel index")?,
            id: field(f, 1).to_owned(),
            phase: field(f, 2).to_owned(),
            circuit: field(f, 3).to_owned(),
            unit: field(f, 4).to_owned(),
            a: self.num_or(field(f, 5), 1.0, "multiplier a")?,
            b: self.num_or(field(f, 6), 0.0, "offset b")?,
            skew: self.num_or(field(f, 7), 0.0, "skew")?,
            min: self.num_or(field(f, 8), f64::NAN, "min")?,
            max: self.num_or(field(f, 9), f64::NAN, "max")?,
            primary: self.num_or(field(f, 10), 1.0, "primary")?,
            secondary: self.num_or(field(f, 11), 1.0, "secondary")?,
            side,
        })
    }

    fn binary(&self, f: &[&str]) -> Result<BinaryInfo> {
        // 1991: Dn,ch_id,y   1999+: Dn,ch_id,ph,ccbm,y
        let (id, phase, circuit, state) = match f.len() {
            0..=1 => return Err(self.err("binary channel line needs at least 2 fields")),
            2 => (field(f, 1), "", "", ""),
            3 => (field(f, 1), "", "", field(f, 2)),
            _ => (field(f, 1), field(f, 2), field(f, 3), field(f, 4)),
        };
        Ok(BinaryInfo {
            index: self.num(field(f, 0), "binary channel index")?,
            id: id.to_owned(),
            phase: phase.to_owned(),
            circuit: circuit.to_owned(),
            normal_state: state == "1",
        })
    }

    fn datetime(&self, f: &[&str], revision: Revision) -> Result<NaiveDateTime> {
        let date = field(f, 0);
        let time = field(f, 1);
        let parts: Vec<&str> = date.split('/').map(str::trim).collect();
        if parts.len() != 3 {
            return Err(self.err(format!("invalid date '{date}'")));
        }
        let nums: Vec<u32> = parts
            .iter()
            .map(|p| p.parse::<u32>())
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| self.err(format!("invalid date '{date}'")))?;
        // 1991 uses mm/dd/yy, later revisions dd/mm/yyyy. Swap when the
        // order is clearly the other one.
        let (mut day, mut month) = match revision {
            Revision::Rev1991 => (nums[1], nums[0]),
            _ => (nums[0], nums[1]),
        };
        if month > 12 && day <= 12 {
            std::mem::swap(&mut day, &mut month);
        }
        let year = match nums[2] {
            y @ 0..=69 => 2000 + y,
            y @ 70..=99 => 1900 + y,
            y => y,
        };
        let date = NaiveDate::from_ymd_opt(year as i32, month, day)
            .ok_or_else(|| self.err(format!("invalid date '{date}'")))?;
        let time = parse_time(time).ok_or_else(|| self.err(format!("invalid time '{time}'")))?;
        Ok(date.and_time(time))
    }

    fn count(&self, s: &str, suffix: char) -> Result<usize> {
        let digits = s
            .strip_suffix(suffix)
            .or_else(|| s.strip_suffix(suffix.to_ascii_lowercase()))
            .unwrap_or(s);
        self.num(digits, &format!("{suffix} channel count"))
    }

    fn num<T: std::str::FromStr>(&self, s: &str, what: &str) -> Result<T> {
        s.parse().map_err(|_| self.err(format!("invalid {what} '{s}'")))
    }

    fn num_or(&self, s: &str, default: f64, what: &str) -> Result<f64> {
        if s.is_empty() { Ok(default) } else { self.num(s, what) }
    }
}

fn split_fields(line: &str) -> Vec<&str> {
    line.split(',').map(str::trim).collect()
}

fn field<'a>(f: &[&'a str], i: usize) -> &'a str {
    f.get(i).copied().unwrap_or("")
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

/// Parses `hh:mm:ss[.fffffffff]`.
fn parse_time(s: &str) -> Option<NaiveTime> {
    let mut it = s.split(':');
    let h: u32 = it.next()?.trim().parse().ok()?;
    let m: u32 = it.next()?.trim().parse().ok()?;
    let sec = it.next()?.trim();
    if it.next().is_some() {
        return None;
    }
    let (whole, frac) = sec.split_once('.').unwrap_or((sec, ""));
    let s: u32 = whole.parse().ok()?;
    let nanos = if frac.is_empty() {
        0
    } else {
        let digits: String = frac.chars().take(9).collect();
        let n: u32 = digits.parse().ok()?;
        n * 10u32.pow(9 - digits.len() as u32)
    };
    NaiveTime::from_hms_nano_opt(h, m, s, nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG_1999: &str = "\
STATION A,RELAY 1,1999
3,2A,1D
1,IA,A,LINE,A,0.01,0,0,-32767,32767,600,1,P
2,VA,A,LINE,kV,0.005,0.1,0,-32767,32767,110,0.1,S
1,TRIP,,,0
50
1
1000,20
01/02/2020,10:00:00.000000
01/02/2020,10:00:00.010000
BINARY
1
";

    #[test]
    fn parses_1999() {
        let c = Config::parse(CFG_1999).unwrap();
        assert_eq!(c.station_name, "STATION A");
        assert_eq!(c.revision, Revision::Rev1999);
        assert_eq!(c.analog.len(), 2);
        assert_eq!(c.analog[1].unit, "kV");
        assert_eq!(c.analog[1].side, Side::Secondary);
        assert_eq!(c.binary[0].id, "TRIP");
        assert_eq!(c.total_samples(), 20);
        assert_eq!(c.data_format, DataFormat::Binary);
        assert_eq!(c.start_time.date(), NaiveDate::from_ymd_opt(2020, 2, 1).unwrap());
        assert!((c.trigger_offset() - 0.01).abs() < 1e-9);
        assert_eq!(c.record_size(), Some(8 + 2 * 2 + 2));
        assert!((c.duration().unwrap() - 0.019).abs() < 1e-9);
    }

    #[test]
    fn parses_1991_without_revision() {
        let cfg = "\
OLD,DEV
2,1A,1D
1,IA,A,,A,1,0,0,-100,100
1,TRIP,0
60
1
1000,10
02/28/95,23:59:59.5
02/28/95,23:59:59.6
ASCII
";
        let c = Config::parse(cfg).unwrap();
        assert_eq!(c.revision, Revision::Rev1991);
        assert_eq!(c.analog[0].primary, 1.0);
        assert_eq!(c.binary[0].id, "TRIP");
        assert_eq!(c.start_time.date(), NaiveDate::from_ymd_opt(1995, 2, 28).unwrap());
        assert_eq!(c.time_mult, 1.0);
    }

    #[test]
    fn parses_2013_extra_lines_and_crlf() {
        let cfg = CFG_1999.replace("1999", "2013").replace('\n', "\r\n") + "-5h30,-5h30\r\nA,0\r\n";
        let c = Config::parse(&cfg).unwrap();
        assert_eq!(c.revision, Revision::Rev2013);
        assert_eq!(c.time_code.as_deref(), Some("-5h30"));
        assert_eq!(c.tmq_code.as_deref(), Some("A"));
    }

    #[test]
    fn rejects_wrong_channel_count() {
        let cfg = CFG_1999.replace("3,2A,1D", "4,2A,1D");
        match Config::parse(&cfg) {
            Err(Error::Cfg { line, .. }) => assert_eq!(line, 2),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn rejects_truncated_file() {
        let cfg: String = CFG_1999.lines().take(6).collect::<Vec<_>>().join("\n");
        assert!(matches!(Config::parse(&cfg), Err(Error::Cfg { line: 7, .. })));
    }

    #[test]
    fn side_factor_converts() {
        let c = Config::parse(CFG_1999).unwrap();
        assert_eq!(c.analog[0].side_factor(Side::Primary), 1.0);
        assert!((c.analog[0].side_factor(Side::Secondary) - 1.0 / 600.0).abs() < 1e-12);
        assert!((c.analog[1].side_factor(Side::Primary) - 1100.0).abs() < 1e-9);
    }
}
