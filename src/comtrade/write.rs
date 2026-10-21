//! Writer for COMTRADE recordings: configuration (`.cfg`), sample data
//! (`.dat`) and the 2013 combined format (`.cff`).
//!
//! [`encode`] converts a loaded recording to another revision and data
//! format. Raw values are copied whenever the target format can hold them;
//! otherwise the channel is rescaled and the change is reported in
//! [`Encoded::notes`].

use std::fmt::Write as _;

use chrono::{Datelike, NaiveDateTime, Timelike};

use super::cfg::{AnalogInfo, Config, DataFormat, Revision};
use super::{Comtrade, Samples};
use crate::error::{Error, Result};

/// Line ending required by the standard.
const EOL: &str = "\r\n";

/// A recording encoded for writing.
#[derive(Debug, Clone)]
pub struct Encoded {
    /// Configuration of the written recording.
    pub config: Config,
    /// Content of the configuration file.
    pub cfg: String,
    /// Content of the data file.
    pub dat: Vec<u8>,
    /// Changes made to fit the target: rescaled channels, rounded values and
    /// fields the target revision cannot store.
    pub notes: Vec<String>,
}

impl Encoded {
    /// Content of a combined `.cff` file with the optional `.inf` and `.hdr`
    /// text. Only revision 2013 defines the combined format.
    pub fn cff(&self, info: Option<&str>, header: Option<&str>) -> Vec<u8> {
        let mut out = format!("--- file type: CFG ---{EOL}{}", self.cfg);
        for (kind, text) in [("INF", info), ("HDR", header)] {
            if let Some(text) = text {
                let _ = write!(out, "--- file type: {kind} ---{EOL}{text}");
                if !text.ends_with('\n') {
                    out.push_str(EOL);
                }
            }
        }
        let mut out = out.into_bytes();
        match self.config.data_format {
            DataFormat::Ascii => out.extend(format!("--- file type: DAT ASCII ---{EOL}").bytes()),
            _ => out.extend(format!("--- file type: DAT BINARY: {} ---{EOL}", self.dat.len()).bytes()),
        }
        out.extend(&self.dat);
        out
    }
}

/// Returns an error when `revision` cannot store `format` data.
pub fn check_target(revision: Revision, format: DataFormat) -> Result<()> {
    if matches!(format, DataFormat::Binary32 | DataFormat::Float32) && revision != Revision::Rev2013 {
        return Err(Error::Unsupported(format!(
            "{format} data needs revision 2013, not {revision}"
        )));
    }
    Ok(())
}

/// Encodes `rec` as a recording of `revision` with `format` data.
pub fn encode(rec: &Comtrade, revision: Revision, format: DataFormat) -> Result<Encoded> {
    check_target(revision, format)?;
    let source = &rec.config;
    let mut notes = Vec::new();
    let mut config = source.clone();
    config.revision = revision;
    config.data_format = format;

    match revision {
        Revision::Rev1991 => {
            // 1991 has no time multiplier: timestamps are in µs.
            config.time_mult = 1.0;
            if source.revision != Revision::Rev1991 {
                notes.push(
                    "revision 1991 has no transformer ratios, primary/secondary flag, binary channel \
                     phase and circuit or time codes; they are left out"
                        .into(),
                );
            }
            for t in [config.start_time, config.trigger_time] {
                if !(1970..=2069).contains(&t.year()) {
                    return Err(Error::Unsupported(format!(
                        "revision 1991 stores two-digit years (1970-2069), the recording is from {}",
                        t.year()
                    )));
                }
            }
        }
        Revision::Rev1999 => {
            if [
                &source.time_code,
                &source.local_code,
                &source.tmq_code,
                &source.leap_second,
            ]
            .iter()
            .any(|f| f.is_some())
            {
                notes.push("revision 1999 has no time codes or leap second field; they are left out".into());
            }
        }
        Revision::Rev2013 => {}
    }
    if revision != Revision::Rev2013 {
        (config.time_code, config.local_code, config.tmq_code, config.leap_second) = (None, None, None, None);
    }

    let columns: Vec<Column> = rec
        .analog_channels()
        .zip(&mut config.analog)
        .map(|(ch, info)| Column::new(ch.samples(), info, format, &mut notes))
        .collect();
    let timestamps = timestamps(rec, config.time_mult)?;
    let binary: Vec<_> = rec.binary_channels().map(|c| c.samples()).collect();

    let dat = match format {
        DataFormat::Ascii => ascii_dat(&timestamps, &columns, &binary, revision),
        _ => binary_dat(&timestamps, &columns, &binary, format),
    };
    Ok(Encoded {
        cfg: cfg_text(&config),
        config,
        dat,
        notes,
    })
}

/// Raw analog values of one channel prepared for the target format.
enum Column {
    /// Values for an integer format (`None`: missing).
    Int(Vec<Option<i64>>),
    /// Values for a float format; `f32` keeps short ASCII output for FLOAT32
    /// sources.
    Float(Vec<Option<f64>>, bool),
}

impl Column {
    /// Prepares `samples` for `format`, adjusting `info` (scaling and raw
    /// range) when the values have to be rescaled.
    fn new(samples: &Samples, info: &mut AnalogInfo, format: DataFormat, notes: &mut Vec<String>) -> Self {
        let raw: Vec<Option<f64>> = (0..samples.len()).map(|i| samples.raw(i)).collect();
        let limit = match format {
            // The most negative value marks a missing sample.
            DataFormat::Binary => i16::MAX as f64,
            DataFormat::Binary32 => i32::MAX as f64,
            DataFormat::Float32 => {
                if raw.iter().flatten().any(|&r| r as f32 as f64 != r) {
                    notes.push(format!(
                        "{}: values rounded to 32-bit float (about 7 significant digits)",
                        info.id
                    ));
                }
                return Self::Float(raw, true);
            }
            DataFormat::Ascii => return Self::Float(raw, matches!(samples, Samples::F32(_))),
        };

        let peak = raw.iter().flatten().fold(0.0_f64, |m, r| m.max(r.abs()));
        let integral = raw.iter().flatten().all(|r| r.fract() == 0.0);
        let factor = if (integral && peak <= limit) || peak == 0.0 {
            1.0
        } else {
            limit / peak
        };
        if factor == 1.0 {
            info.min = clamp_or(info.min, limit);
            info.max = clamp_or(info.max, limit);
        } else {
            // value = a * raw + b stays the same with raw * factor and a / factor.
            info.a /= factor;
            info.min = -limit;
            info.max = limit;
            let bits = if format == DataFormat::Binary { 16 } else { 32 };
            notes.push(format!(
                "{}: rescaled to {bits}-bit integers, resolution {:.4e} {}",
                info.id, info.a, info.unit
            ));
        }
        Self::Int(raw.iter().map(|r| r.map(|r| (r * factor).round() as i64)).collect())
    }
}

/// `v` limited to `±limit`, or `±limit` when the file gave no value.
fn clamp_or(v: f64, limit: f64) -> f64 {
    if v.is_nan() {
        limit.copysign(v)
    } else {
        v.clamp(-limit, limit)
    }
}

/// Timestamps in units of `time_mult` µs, counted from the first sample.
fn timestamps(rec: &Comtrade, time_mult: f64) -> Result<Vec<u32>> {
    let offset = rec.config.trigger_offset();
    let unit = time_mult * 1e-6;
    rec.time()
        .iter()
        .map(|t| {
            let ts = ((t + offset) / unit).round();
            if (0.0..u32::MAX as f64).contains(&ts) {
                Ok(ts as u32)
            } else {
                Err(Error::Unsupported(format!(
                    "time {:.6} s does not fit in a 32-bit timestamp with time multiplier {time_mult}",
                    t + offset
                )))
            }
        })
        .collect()
}

fn ascii_dat(timestamps: &[u32], columns: &[Column], binary: &[&super::Bits], revision: Revision) -> Vec<u8> {
    // 2013 marks a missing value with an empty field, older revisions with 99999.
    let missing = if revision == Revision::Rev2013 { "" } else { "99999" };
    let mut out = String::with_capacity(timestamps.len() * (12 + 8 * columns.len() + 2 * binary.len()));
    for (i, ts) in timestamps.iter().enumerate() {
        let _ = write!(out, "{},{ts}", i + 1);
        for col in columns {
            out.push(',');
            match col {
                Column::Int(v) => match v[i] {
                    Some(x) => {
                        let _ = write!(out, "{x}");
                    }
                    None => out.push_str(missing),
                },
                Column::Float(v, f32_source) => match v[i] {
                    Some(x) if *f32_source => {
                        let _ = write!(out, "{}", x as f32);
                    }
                    Some(x) => {
                        let _ = write!(out, "{x}");
                    }
                    None => out.push_str(missing),
                },
            }
        }
        for bits in binary {
            out.push_str(if bits.get(i) { ",1" } else { ",0" });
        }
        out.push_str(EOL);
    }
    out.into_bytes()
}

fn binary_dat(timestamps: &[u32], columns: &[Column], binary: &[&super::Bits], format: DataFormat) -> Vec<u8> {
    let words = binary.len().div_ceil(16);
    let record = 8 + columns.len() * format.analog_size().expect("binary format") + words * 2;
    let mut out = Vec::with_capacity(timestamps.len() * record);
    for (i, ts) in timestamps.iter().enumerate() {
        out.extend((i as u32 + 1).to_le_bytes());
        out.extend(ts.to_le_bytes());
        for col in columns {
            match (col, format) {
                (Column::Int(v), DataFormat::Binary) => {
                    out.extend(v[i].map_or(i16::MIN, |x| x as i16).to_le_bytes());
                }
                (Column::Int(v), _) => out.extend(v[i].map_or(i32::MIN, |x| x as i32).to_le_bytes()),
                (Column::Float(v, _), _) => out.extend(v[i].map_or(f32::NAN, |x| x as f32).to_le_bytes()),
            }
        }
        for w in 0..words {
            let word = (0..16)
                .filter(|bit| binary.get(w * 16 + bit).is_some_and(|b| b.get(i)))
                .fold(0u16, |acc, bit| acc | 1 << bit);
            out.extend(word.to_le_bytes());
        }
    }
    out
}

/// Configuration file content for `c`, in the layout of `c.revision`.
pub fn cfg_text(c: &Config) -> String {
    let rev = c.revision;
    let mut s = String::new();
    let mut line = |text: String| {
        s.push_str(&text);
        s.push_str(EOL);
    };
    line(match rev {
        Revision::Rev1991 => format!("{},{}", c.station_name, c.device_id),
        _ => format!("{},{},{rev}", c.station_name, c.device_id),
    });
    line(format!(
        "{},{}A,{}D",
        c.analog.len() + c.binary.len(),
        c.analog.len(),
        c.binary.len()
    ));
    for a in &c.analog {
        let common = format!(
            "{},{},{},{},{},{},{},{},{},{}",
            a.index,
            a.id,
            a.phase,
            a.circuit,
            a.unit,
            num(a.a),
            num(a.b),
            num(a.skew),
            num(a.min),
            num(a.max)
        );
        line(match rev {
            Revision::Rev1991 => common,
            _ => format!("{common},{},{},{}", num(a.primary), num(a.secondary), a.side),
        });
    }
    for d in &c.binary {
        let state = u8::from(d.normal_state);
        line(match rev {
            Revision::Rev1991 => format!("{},{},{state}", d.index, d.id),
            _ => format!("{},{},{},{},{state}", d.index, d.id, d.phase, d.circuit),
        });
    }
    line(num(c.line_frequency));
    if c.has_fixed_rates() {
        line(c.sample_rates.len().to_string());
        for r in &c.sample_rates {
            line(format!("{},{}", num(r.rate), r.last_sample));
        }
    } else {
        // No fixed rate: the data file timestamps give the time.
        line("0".into());
        line(format!("0,{}", c.total_samples()));
    }
    line(datetime(c.start_time, rev));
    line(datetime(c.trigger_time, rev));
    line(c.data_format.to_string());
    if rev != Revision::Rev1991 {
        line(num(c.time_mult));
    }
    if rev == Revision::Rev2013 {
        let code = |f: &Option<String>| f.clone().unwrap_or_else(|| "0".into());
        line(format!("{},{}", code(&c.time_code), code(&c.local_code)));
        line(format!("{},{}", code(&c.tmq_code), code(&c.leap_second)));
    }
    s
}

/// Formats a number field; a missing value (NaN) is left empty.
fn num(v: f64) -> String {
    if v.is_finite() { v.to_string() } else { String::new() }
}

/// `date,time` field pair: `mm/dd/yy` for 1991, `dd/mm/yyyy` later. 2013
/// allows nanoseconds, which are written only when they are needed.
fn datetime(t: NaiveDateTime, rev: Revision) -> String {
    let date = match rev {
        Revision::Rev1991 => t.format("%m/%d/%y"),
        _ => t.format("%d/%m/%Y"),
    };
    let ns = t.nanosecond() % 1_000_000_000;
    let fraction = if rev == Revision::Rev2013 && !ns.is_multiple_of(1000) {
        format!("{ns:09}")
    } else {
        format!("{:06}", ns / 1000)
    };
    format!("{date},{}.{fraction}", t.format("%H:%M:%S"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comtrade::cff;

    fn load(name: &str) -> Comtrade {
        Comtrade::open(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data")
                .join(name),
        )
        .unwrap()
    }

    /// Reads `e` back with the parser.
    fn read_back(e: &Encoded, combined: bool) -> Comtrade {
        if combined {
            let data = e.cff(Some("[x]\r\n"), Some("header"));
            let s = cff::split(&data).unwrap();
            assert_eq!(s.hdr, Some(&b"header\r\n"[..]));
            Comtrade::from_readers(s.cfg.unwrap(), s.dat.unwrap()).unwrap()
        } else {
            Comtrade::from_readers(e.cfg.as_bytes(), &e.dat[..]).unwrap()
        }
    }

    /// Every sample recording converted to every possible target reads back
    /// with the same times, binary states and values (within the resolution
    /// of a rescaled channel).
    #[test]
    fn conversions_round_trip() {
        use DataFormat::*;
        use Revision::*;
        let sources = [
            "fault_1999.cfg",
            "fault_2013.cfg",
            "old_1991.cfg",
            "combined_2013.cff",
            "reclose_1999_ascii.cfg",
        ];
        for name in sources {
            let src = load(name);
            for (rev, format) in [
                (Rev1991, Ascii),
                (Rev1991, Binary),
                (Rev1999, Ascii),
                (Rev1999, Binary),
                (Rev2013, Ascii),
                (Rev2013, Binary),
                (Rev2013, Binary32),
                (Rev2013, Float32),
            ] {
                let what = format!("{name} -> {rev} {format}");
                let e = encode(&src, rev, format).unwrap_or_else(|err| panic!("{what}: {err}"));
                let out = read_back(&e, rev == Rev2013);
                assert_eq!(out.config.revision, rev, "{what}");
                assert_eq!(out.config.data_format, format, "{what}");
                assert_eq!(out.config.station_name, src.config.station_name, "{what}");
                assert_eq!(out.config.start_time, src.config.start_time, "{what}");
                assert_eq!(out.config.trigger_time, src.config.trigger_time, "{what}");
                assert_eq!(out.len(), src.len(), "{what}");
                for (a, b) in out.time().iter().zip(src.time()) {
                    assert!((a - b).abs() < 1e-6, "{what}: time {a} != {b}");
                }
                for (a, b) in out.binary_channels().zip(src.binary_channels()) {
                    assert_eq!(a.info.id, b.info.id, "{what}");
                    assert!((0..src.len()).all(|i| a.get(i) == b.get(i)), "{what}: {}", a.info.id);
                }
                for (a, b) in out.analog_channels().zip(src.analog_channels()) {
                    // Half a step of the written resolution, plus float rounding.
                    let tol = a.info.a.abs() / 2.0 + 1e-6 * b.values().flatten().fold(1.0_f64, |m, v| m.max(v.abs()));
                    for (x, y) in a.values().zip(b.values()) {
                        match (x, y) {
                            (Some(x), Some(y)) => assert!((x - y).abs() <= tol, "{what}: {} {x} != {y}", a.info.id),
                            (x, y) => assert_eq!(x.is_some(), y.is_some(), "{what}: {}", a.info.id),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rescaling_and_dropped_fields_are_reported() {
        // FLOAT32 values do not fit 16-bit integers as they are.
        let e = encode(&load("combined_2013.cff"), Revision::Rev1999, DataFormat::Binary).unwrap();
        assert!(
            e.notes.iter().any(|n| n.starts_with("IA: rescaled to 16-bit")),
            "{:?}",
            e.notes
        );
        assert!(e.notes.iter().any(|n| n.contains("time codes")), "{:?}", e.notes);
        assert_eq!(e.config.analog[0].max, 32767.0);

        // 16-bit integers fit everywhere unchanged.
        let src = load("fault_1999.cfg");
        let e = encode(&src, Revision::Rev2013, DataFormat::Binary32).unwrap();
        assert!(e.notes.is_empty(), "{:?}", e.notes);
        assert_eq!(e.config.analog, src.config.analog);
    }

    #[test]
    fn rejects_formats_the_revision_lacks() {
        let src = load("fault_1999.cfg");
        assert!(matches!(
            encode(&src, Revision::Rev1999, DataFormat::Float32),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn cfg_layout_follows_revision() {
        let src = load("fault_2013.cfg");
        let mut c = src.config.clone();
        c.revision = Revision::Rev1991;
        c.data_format = DataFormat::Binary;
        let text = cfg_text(&c);
        let lines: Vec<&str> = text.split(EOL).collect();
        assert_eq!(lines[0], "SUBSTATION B,RELAY 21");
        assert_eq!(lines[2], "1,IA,A,LINE 4,A,0.001,0,0,-2147483647,2147483647");
        assert_eq!(lines[10], "1,START_A,0");
        assert!(text.contains("10/20/26,14:00:00.000000\r\n10/20/26,14:00:00.500000\r\nBINARY\r\n"));
        assert!(text.ends_with("BINARY\r\n"));

        let text = cfg_text(&src.config);
        assert!(text.contains("20/10/2026,14:00:00.500000\r\nBINARY32\r\n1\r\n+1h00,+1h00\r\n0,0\r\n"));
    }
}
