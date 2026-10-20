//! Reader for COMTRADE sample data (`.dat`) in ASCII, BINARY, BINARY32 and FLOAT32 format.

use super::cfg::{Config, DataFormat, Revision};
use crate::error::{Error, Result};

/// Marker for a missing timestamp.
pub const MISSING_TIMESTAMP: u32 = u32::MAX;

/// Raw analog samples, kept in the type used by the data file.
///
/// Missing values are stored as `i16::MIN`, `i32::MIN` or `NaN`.
#[derive(Debug, Clone, PartialEq)]
pub enum Samples {
    /// `BINARY` data.
    I16(Vec<i16>),
    /// `BINARY32` data.
    I32(Vec<i32>),
    /// `FLOAT32` data.
    F32(Vec<f32>),
    /// `ASCII` data.
    F64(Vec<f64>),
}

impl Samples {
    fn with_capacity(format: DataFormat, n: usize) -> Self {
        match format {
            DataFormat::Binary => Self::I16(Vec::with_capacity(n)),
            DataFormat::Binary32 => Self::I32(Vec::with_capacity(n)),
            DataFormat::Float32 => Self::F32(Vec::with_capacity(n)),
            DataFormat::Ascii => Self::F64(Vec::with_capacity(n)),
        }
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        match self {
            Self::I16(v) => v.len(),
            Self::I32(v) => v.len(),
            Self::F32(v) => v.len(),
            Self::F64(v) => v.len(),
        }
    }

    /// Returns `true` when there are no samples.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Raw value at `i`, `None` when missing or out of range.
    pub fn raw(&self, i: usize) -> Option<f64> {
        match self {
            Self::I16(v) => v.get(i).filter(|&&x| x != i16::MIN).map(|&x| x as f64),
            Self::I32(v) => v.get(i).filter(|&&x| x != i32::MIN).map(|&x| x as f64),
            Self::F32(v) => v.get(i).filter(|x| x.is_finite()).map(|&x| x as f64),
            Self::F64(v) => v.get(i).filter(|x| x.is_finite()).copied(),
        }
    }
}

/// Packed binary channel states, one bit per sample.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bits {
    words: Vec<u64>,
    len: usize,
}

impl Bits {
    fn with_capacity(n: usize) -> Self {
        Self {
            words: Vec::with_capacity(n.div_ceil(64)),
            len: 0,
        }
    }

    fn push(&mut self, bit: bool) {
        if self.len.is_multiple_of(64) {
            self.words.push(0);
        }
        if bit {
            *self.words.last_mut().expect("word pushed above") |= 1 << (self.len % 64);
        }
        self.len += 1;
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when there are no samples.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// State at sample `i` (`false` when out of range).
    pub fn get(&self, i: usize) -> bool {
        i < self.len && self.words[i / 64] >> (i % 64) & 1 == 1
    }

    /// Returns `true` when any sample in `range` is set.
    pub fn any(&self, range: std::ops::Range<usize>) -> bool {
        range.into_iter().any(|i| self.get(i))
    }

    /// Returns `true` when all samples in `range` are set.
    pub fn all(&self, range: std::ops::Range<usize>) -> bool {
        range.into_iter().all(|i| self.get(i))
    }
}

/// Decoded content of a data file.
#[derive(Debug, Clone)]
pub struct DatData {
    /// Timestamp column (`MISSING_TIMESTAMP` when missing).
    pub timestamps: Vec<u32>,
    /// Analog columns, same order as in the configuration.
    pub analog: Vec<Samples>,
    /// Binary columns, same order as in the configuration.
    pub binary: Vec<Bits>,
}

impl DatData {
    /// Number of samples (records).
    pub fn len(&self) -> usize {
        self.timestamps.len()
    }

    /// Returns `true` when there are no samples.
    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }
}

/// Decodes `data` according to `cfg`.
pub fn parse(cfg: &Config, data: &[u8]) -> Result<DatData> {
    let dat = match cfg.data_format {
        DataFormat::Ascii => parse_ascii(cfg, data)?,
        _ => parse_binary(cfg, data)?,
    };
    let expected = cfg.total_samples();
    if expected > 0 && dat.len() != expected {
        return Err(Error::Dat {
            sample: dat.len().min(expected) + 1,
            message: format!("data file has {} samples, configuration declares {expected}", dat.len()),
        });
    }
    Ok(dat)
}

fn new_columns(cfg: &Config, n: usize) -> DatData {
    DatData {
        timestamps: Vec::with_capacity(n),
        analog: (0..cfg.analog.len())
            .map(|_| Samples::with_capacity(cfg.data_format, n))
            .collect(),
        binary: (0..cfg.binary.len()).map(|_| Bits::with_capacity(n)).collect(),
    }
}

fn parse_binary(cfg: &Config, mut data: &[u8]) -> Result<DatData> {
    let rec = cfg.record_size().expect("binary format has a record size");
    // Some writers append a DOS end-of-file marker.
    if data.len() % rec == 1 && data.last() == Some(&0x1A) {
        data = &data[..data.len() - 1];
    }
    if !data.len().is_multiple_of(rec) {
        return Err(Error::Dat {
            sample: data.len() / rec + 1,
            message: format!("file size {} is not a multiple of the record size {rec}", data.len()),
        });
    }
    let n = data.len() / rec;
    let mut out = new_columns(cfg, n);
    let n_analog = cfg.analog.len();
    let n_words = cfg.binary.len().div_ceil(16);

    for record in data.chunks_exact(rec) {
        out.timestamps
            .push(u32::from_le_bytes(record[4..8].try_into().unwrap()));
        let mut pos = 8;
        for col in &mut out.analog {
            match col {
                Samples::I16(v) => {
                    v.push(i16::from_le_bytes(record[pos..pos + 2].try_into().unwrap()));
                    pos += 2;
                }
                Samples::I32(v) => {
                    v.push(i32::from_le_bytes(record[pos..pos + 4].try_into().unwrap()));
                    pos += 4;
                }
                Samples::F32(v) => {
                    v.push(f32::from_le_bytes(record[pos..pos + 4].try_into().unwrap()));
                    pos += 4;
                }
                Samples::F64(_) => unreachable!("ASCII data in binary reader"),
            }
        }
        debug_assert_eq!(pos, 8 + n_analog * cfg.data_format.analog_size().unwrap());
        for w in 0..n_words {
            let word = u16::from_le_bytes(record[pos + 2 * w..pos + 2 * w + 2].try_into().unwrap());
            for bit in 0..16 {
                if let Some(col) = out.binary.get_mut(w * 16 + bit) {
                    col.push(word >> bit & 1 == 1);
                }
            }
        }
    }
    Ok(out)
}

fn parse_ascii(cfg: &Config, data: &[u8]) -> Result<DatData> {
    let text = String::from_utf8_lossy(data);
    let n_analog = cfg.analog.len();
    let n_binary = cfg.binary.len();
    // 1991/1999 mark missing ASCII values with 99999, 2013 with an empty field.
    let legacy_missing = cfg.revision != Revision::Rev2013;
    let mut out = new_columns(cfg, cfg.total_samples());

    let rows = text
        .lines()
        .map(|l| l.trim_end_matches(['\x1A', ' ', '\t']))
        .filter(|l| !l.is_empty());
    for (row, line) in rows.enumerate() {
        let sample = row + 1;
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() < 2 + n_analog + n_binary {
            return Err(Error::Dat {
                sample,
                message: format!("expected {} fields, found {}", 2 + n_analog + n_binary, fields.len()),
            });
        }
        let bad = |what: &str, s: &str| Error::Dat {
            sample,
            message: format!("invalid {what} '{s}'"),
        };
        out.timestamps.push(match fields[1] {
            "" => MISSING_TIMESTAMP,
            s => s.parse::<f64>().map(|t| t as u32).map_err(|_| bad("timestamp", s))?,
        });
        for (col, s) in out.analog.iter_mut().zip(&fields[2..2 + n_analog]) {
            let value = match *s {
                "" => f64::NAN,
                "99999" if legacy_missing => f64::NAN,
                s => s.parse::<f64>().map_err(|_| bad("analog value", s))?,
            };
            let Samples::F64(v) = col else {
                unreachable!("ASCII columns are f64")
            };
            v.push(value);
        }
        for (col, s) in out.binary.iter_mut().zip(&fields[2 + n_analog..]) {
            col.push(match *s {
                "0" | "" => false,
                "1" => true,
                s => return Err(bad("binary value", s)),
            });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(format: &str, analog: usize, binary: usize, samples: usize) -> Config {
        let mut s = format!("S,D,1999\n{},{analog}A,{binary}D\n", analog + binary);
        for i in 1..=analog {
            s += &format!("{i},A{i},,,V,1,0,0,-1,1,1,1,P\n");
        }
        for i in 1..=binary {
            s += &format!("{i},D{i},,,0\n");
        }
        s += &format!("50\n1\n1000,{samples}\n01/01/2020,00:00:00\n01/01/2020,00:00:00\n{format}\n1\n");
        Config::parse(&s).unwrap()
    }

    #[test]
    fn reads_binary16_with_missing_values() {
        let c = cfg("BINARY", 2, 17, 2);
        let mut data = Vec::new();
        for (n, a, b, w0, w1) in [(1u32, 5i16, -3i16, 0b101u16, 1u16), (2, i16::MIN, 7, 0, 0)] {
            data.extend(n.to_le_bytes());
            data.extend((n * 1000).to_le_bytes());
            data.extend(a.to_le_bytes());
            data.extend(b.to_le_bytes());
            data.extend(w0.to_le_bytes());
            data.extend(w1.to_le_bytes());
        }
        let d = parse(&c, &data).unwrap();
        assert_eq!(d.len(), 2);
        assert_eq!(d.timestamps, vec![1000, 2000]);
        assert_eq!(d.analog[0].raw(0), Some(5.0));
        assert_eq!(d.analog[0].raw(1), None);
        assert_eq!(d.analog[1].raw(1), Some(7.0));
        assert!(d.binary[0].get(0));
        assert!(!d.binary[1].get(0));
        assert!(d.binary[2].get(0));
        assert!(d.binary[16].get(0));
        assert!(!d.binary[16].get(1));
    }

    #[test]
    fn reads_binary32_and_float32() {
        let c = cfg("BINARY32", 1, 0, 1);
        let mut data = vec![1, 0, 0, 0, 0, 0, 0, 0];
        data.extend((-70000i32).to_le_bytes());
        assert_eq!(parse(&c, &data).unwrap().analog[0].raw(0), Some(-70000.0));

        let c = cfg("FLOAT32", 1, 0, 1);
        let mut data = vec![1, 0, 0, 0, 0, 0, 0, 0];
        data.extend(1.5f32.to_le_bytes());
        assert_eq!(parse(&c, &data).unwrap().analog[0].raw(0), Some(1.5));
    }

    #[test]
    fn rejects_bad_binary_size() {
        let c = cfg("BINARY", 1, 0, 1);
        assert!(matches!(parse(&c, &[0; 11]), Err(Error::Dat { .. })));
        assert!(matches!(parse(&c, &[0; 20]), Err(Error::Dat { .. })));
    }

    #[test]
    fn reads_ascii() {
        let c = cfg("ASCII", 2, 2, 3);
        let data = b"1,0,1.5,-2,0,1\r\n2,1000,99999,3,1,1\r\n3,,,4,0,0\r\n\x1A";
        let d = parse(&c, data).unwrap();
        assert_eq!(d.len(), 3);
        assert_eq!(d.analog[0].raw(0), Some(1.5));
        assert_eq!(d.analog[0].raw(1), None);
        assert_eq!(d.analog[0].raw(2), None);
        assert_eq!(d.timestamps[2], MISSING_TIMESTAMP);
        assert!(d.binary[1].get(0));
        assert!(d.binary[0].get(1));
    }

    #[test]
    fn ascii_reports_sample_number() {
        let c = cfg("ASCII", 1, 0, 2);
        let err = parse(&c, b"1,0,1\n2,0,x\n").unwrap_err();
        assert!(matches!(err, Error::Dat { sample: 2, .. }));
    }

    #[test]
    fn bits_any_all() {
        let mut b = Bits::default();
        for i in 0..130 {
            b.push(i >= 100);
        }
        assert!(!b.any(0..100));
        assert!(b.any(90..101));
        assert!(b.all(100..130));
        assert!(!b.get(500));
    }
}
