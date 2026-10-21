//! COMTRADE (IEEE C37.111) recordings: configuration, sample data and file discovery.
//!
//! This module is a plain synchronous library and does not depend on the
//! command line or TUI parts of the application.

pub mod cff;
pub mod cfg;
pub mod dat;
pub mod discover;
pub mod export;
pub mod stats;
pub mod write;

use std::fs::File;
use std::io::Read;
use std::ops::{Deref, Range};
use std::path::Path;

pub use crate::error::{Error, Result};
pub use cfg::{AnalogInfo, BinaryInfo, Config, DataFormat, Revision, SampleRate, Side};
pub use dat::{Bits, Samples};
pub use discover::RecordingPath;

/// Plot column with its lowest and highest point: `(column, min, max)`.
type Bucket = (i64, (f64, f64), (f64, f64));

/// An analog channel with its samples.
#[derive(Debug, Clone)]
pub struct AnalogChannel {
    /// Channel metadata.
    pub info: AnalogInfo,
    samples: Samples,
}

impl AnalogChannel {
    /// Raw samples as stored in the data file.
    pub fn samples(&self) -> &Samples {
        &self.samples
    }

    /// Scaled value (`a * raw + b`) at sample `i`, `None` when missing.
    pub fn value(&self, i: usize) -> Option<f64> {
        self.samples.raw(i).map(|r| self.info.a * r + self.info.b)
    }

    /// Scaled value at sample `i` converted to `side`.
    pub fn value_on(&self, i: usize, side: Side) -> Option<f64> {
        let factor = self.info.side_factor(side);
        self.value(i).map(|v| v * factor)
    }

    /// Iterator over all scaled values.
    pub fn values(&self) -> impl Iterator<Item = Option<f64>> + '_ {
        (0..self.samples.len()).map(|i| self.value(i))
    }

    /// Line segments for plotting the samples in `range` into `columns`
    /// horizontal cells spanning `t0..t1`.
    ///
    /// When there are more samples than `2 * columns`, each column keeps only
    /// its minimum and maximum (in time order) so peaks are not lost. A
    /// missing sample ends the current segment, so gaps stay visible.
    pub fn plot_segments(
        &self,
        time: &[f64],
        range: Range<usize>,
        (t0, t1): (f64, f64),
        columns: usize,
        side: Side,
    ) -> Vec<Vec<(f64, f64)>> {
        let factor = self.info.side_factor(side);
        let mut segments = Vec::new();
        let mut current = Vec::new();
        let mut flush = |current: &mut Vec<(f64, f64)>| {
            if !current.is_empty() {
                segments.push(std::mem::take(current));
            }
        };

        if range.len() <= 2 * columns.max(1) {
            for i in range {
                match self.value(i) {
                    Some(v) => current.push((time[i], v * factor)),
                    None => flush(&mut current),
                }
            }
        } else {
            let width = (t1 - t0) / columns.max(1) as f64;
            let last = columns.max(1) as i64 - 1;
            let column_of = |t: f64| (((t - t0) / width).floor() as i64).clamp(0, last);
            let mut bucket: Option<Bucket> = None;
            let emit = |current: &mut Vec<(f64, f64)>, b: Bucket| {
                let (_, lo, hi) = b;
                if lo.0 <= hi.0 {
                    current.extend([lo, hi]);
                } else {
                    current.extend([hi, lo]);
                }
            };
            for i in range {
                let Some(v) = self.value(i) else {
                    if let Some(b) = bucket.take() {
                        emit(&mut current, b);
                    }
                    flush(&mut current);
                    continue;
                };
                let p = (time[i], v * factor);
                let col = column_of(p.0);
                match &mut bucket {
                    Some((c, lo, hi)) if *c == col => {
                        if p.1 < lo.1 {
                            *lo = p;
                        }
                        if p.1 > hi.1 {
                            *hi = p;
                        }
                    }
                    _ => {
                        if let Some(b) = bucket.take() {
                            emit(&mut current, b);
                        }
                        bucket = Some((col, p, p));
                    }
                }
            }
            if let Some(b) = bucket {
                emit(&mut current, b);
            }
        }
        flush(&mut current);
        segments
    }
}

/// A binary (status) channel with its samples.
#[derive(Debug, Clone)]
pub struct BinaryChannel {
    /// Channel metadata.
    pub info: BinaryInfo,
    samples: Bits,
}

impl BinaryChannel {
    /// Packed samples.
    pub fn samples(&self) -> &Bits {
        &self.samples
    }

    /// State at sample `i`.
    pub fn get(&self, i: usize) -> bool {
        self.samples.get(i)
    }

    /// State of each of `columns` horizontal cells spanning `t0..t1`.
    pub fn strip(&self, time: &[f64], (t0, t1): (f64, f64), columns: usize) -> Vec<Cell> {
        let width = (t1 - t0) / columns.max(1) as f64;
        (0..columns)
            .map(|c| {
                let a = t0 + c as f64 * width;
                let start = time.partition_point(|&t| t < a);
                let end = if c + 1 == columns {
                    time.partition_point(|&t| t <= t1)
                } else {
                    time.partition_point(|&t| t < a + width)
                };
                let range = if end > start {
                    start..end
                } else if start > 0 && start < time.len() {
                    // Zoomed in further than the sample period: hold the previous state.
                    start - 1..start
                } else {
                    return Cell::Empty;
                };
                match (self.samples.any(range.clone()), self.samples.all(range)) {
                    (true, true) => Cell::High,
                    (false, _) => Cell::Low,
                    (true, false) => Cell::Mixed,
                }
            })
            .collect()
    }

    /// Returns `true` when the state changes at least once.
    pub fn changes(&self) -> bool {
        let n = self.samples.len();
        n > 0 && {
            let first = self.samples.get(0);
            if first {
                !self.samples.all(0..n)
            } else {
                self.samples.any(0..n)
            }
        }
    }
}

/// State of a binary channel within one plot cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// No samples.
    Empty,
    /// All samples are 0.
    Low,
    /// All samples are 1.
    High,
    /// The state changes within the cell.
    Mixed,
}

/// A complete COMTRADE recording.
#[derive(Debug, Clone)]
pub struct Comtrade {
    /// Parsed configuration.
    pub config: Config,
    /// Content of the `.hdr` file / section.
    pub header: Option<String>,
    /// Content of the `.inf` file / section.
    pub info: Option<String>,
    /// Files the recording was loaded from, if loaded from disk.
    pub path: Option<RecordingPath>,
    analog: Vec<AnalogChannel>,
    binary: Vec<BinaryChannel>,
    time: Vec<f64>,
}

impl Comtrade {
    /// Parses only the configuration of the recording `path` belongs to.
    /// Does not read the sample data.
    pub fn from_cfg(path: impl AsRef<Path>) -> Result<Config> {
        let rec = RecordingPath::from_file(path)?;
        Self::config_of(&rec)
    }

    /// Parses only the configuration of `rec`.
    pub fn config_of(rec: &RecordingPath) -> Result<Config> {
        if let Some(cfg) = &rec.cfg {
            return Config::parse(&decode_text(&read_file(cfg)?));
        }
        let cff = rec.cff.as_ref().expect("recording has cfg or cff");
        let data = map_file(cff)?;
        let sections = cff::split(&data)?;
        Config::parse(&decode_text(sections.cfg.expect("checked by split")))
    }

    /// Loads the recording `path` belongs to. `path` may be the `.cfg`,
    /// `.dat`, `.cff` or any other file of the recording.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_recording(&RecordingPath::from_file(path)?)
    }

    /// Loads a recording from its located files.
    pub fn open_recording(rec: &RecordingPath) -> Result<Self> {
        let mut c = if let Some(cfg_path) = &rec.cfg {
            let dat_path = rec
                .dat
                .as_ref()
                .ok_or_else(|| Error::MissingFile(cfg_path.with_extension("dat").display().to_string()))?;
            let config = Config::parse(&decode_text(&read_file(cfg_path)?))?;
            let data = map_file(dat_path)?;
            let mut c = Self::from_parts(config, &data)?;
            c.header = rec.hdr.as_ref().map(read_text).transpose()?;
            c.info = rec.inf.as_ref().map(read_text).transpose()?;
            c
        } else {
            let cff_path = rec.cff.as_ref().expect("recording has cfg or cff");
            let data = map_file(cff_path)?;
            let s = cff::split(&data)?;
            let config = Config::parse(&decode_text(s.cfg.expect("checked by split")))?;
            let dat = s
                .dat
                .ok_or_else(|| Error::MissingFile(format!("DAT section in {}", cff_path.display())))?;
            let mut c = Self::from_parts(config, dat)?;
            c.header = s.hdr.map(decode_text);
            c.info = s.inf.map(decode_text);
            c
        };
        c.path = Some(rec.clone());
        Ok(c)
    }

    /// Parses a recording from any readers (mainly for tests).
    pub fn from_readers(mut cfg: impl Read, mut dat: impl Read) -> Result<Self> {
        let mut text = Vec::new();
        cfg.read_to_end(&mut text).map_err(|e| Error::io("<cfg>", e))?;
        let mut data = Vec::new();
        dat.read_to_end(&mut data).map_err(|e| Error::io("<dat>", e))?;
        Self::from_parts(Config::parse(&decode_text(&text))?, &data)
    }

    fn from_parts(config: Config, data: &[u8]) -> Result<Self> {
        let d = dat::parse(&config, data)?;
        let time = build_time(&config, &d.timestamps);
        let analog = config
            .analog
            .iter()
            .cloned()
            .zip(d.analog)
            .map(|(info, samples)| AnalogChannel { info, samples })
            .collect();
        let binary = config
            .binary
            .iter()
            .cloned()
            .zip(d.binary)
            .map(|(info, samples)| BinaryChannel { info, samples })
            .collect();
        Ok(Self {
            config,
            header: None,
            info: None,
            path: None,
            analog,
            binary,
            time,
        })
    }

    /// A new recording with the samples in `samples` and the channels at the
    /// positions `analog` and `binary` (0-based, in that order; positions
    /// that don't exist are skipped). Channels are numbered from 1 again.
    ///
    /// The start time moves to the first kept sample and the sample rate
    /// segments are counted again; the trigger time stays, so times relative
    /// to the trigger don't change (the trigger may lie outside the samples).
    pub fn slice(&self, samples: Range<usize>, analog: &[usize], binary: &[usize]) -> Self {
        let end = samples.end.min(self.len());
        let samples = samples.start.min(end)..end;

        let mut config = self.config.clone();
        let mut rates = Vec::new();
        let mut first = 0;
        for r in &self.config.sample_rates {
            let kept = r.last_sample.min(samples.end).saturating_sub(first.max(samples.start));
            first = r.last_sample;
            if kept > 0 {
                let last_sample = rates.last().map_or(0, |r: &SampleRate| r.last_sample) + kept;
                rates.push(SampleRate {
                    rate: r.rate,
                    last_sample,
                });
            }
        }
        if rates.last().map(|r| r.last_sample) != Some(samples.len()) {
            // The source declared no sample count; keep its first rate.
            let rate = self.config.sample_rates.first().map_or(0.0, |r| r.rate);
            rates = vec![SampleRate {
                rate,
                last_sample: samples.len(),
            }];
        }
        config.sample_rates = rates;
        if let Some(&t) = self.time.get(samples.start) {
            let since_start = t + self.config.trigger_offset();
            config.start_time =
                self.config.start_time + chrono::TimeDelta::nanoseconds((since_start * 1e9).round() as i64);
        }

        let analog: Vec<AnalogChannel> = analog
            .iter()
            .filter_map(|&i| self.analog.get(i))
            .enumerate()
            .map(|(n, ch)| AnalogChannel {
                info: AnalogInfo {
                    index: n + 1,
                    ..ch.info.clone()
                },
                samples: ch.samples.slice(samples.clone()),
            })
            .collect();
        let binary: Vec<BinaryChannel> = binary
            .iter()
            .filter_map(|&i| self.binary.get(i))
            .enumerate()
            .map(|(n, ch)| BinaryChannel {
                info: BinaryInfo {
                    index: n + 1,
                    ..ch.info.clone()
                },
                samples: ch.samples.slice(samples.clone()),
            })
            .collect();
        config.analog = analog.iter().map(|c| c.info.clone()).collect();
        config.binary = binary.iter().map(|c| c.info.clone()).collect();

        Self {
            config,
            header: self.header.clone(),
            info: self.info.clone(),
            path: None,
            analog,
            binary,
            time: self.time[samples].to_vec(),
        }
    }

    /// Analog channels.
    pub fn analog_channels(&self) -> impl ExactSizeIterator<Item = &AnalogChannel> {
        self.analog.iter()
    }

    /// Binary channels.
    pub fn binary_channels(&self) -> impl ExactSizeIterator<Item = &BinaryChannel> {
        self.binary.iter()
    }

    /// Analog channel by position (0-based).
    pub fn analog(&self, i: usize) -> Option<&AnalogChannel> {
        self.analog.get(i)
    }

    /// Binary channel by position (0-based).
    pub fn binary(&self, i: usize) -> Option<&BinaryChannel> {
        self.binary.get(i)
    }

    /// Time of each sample in seconds relative to the trigger.
    pub fn time(&self) -> &[f64] {
        &self.time
    }

    /// Number of samples.
    pub fn len(&self) -> usize {
        self.time.len()
    }

    /// Returns `true` when the recording has no samples.
    pub fn is_empty(&self) -> bool {
        self.time.is_empty()
    }

    /// Time span `(first, last)` in seconds relative to the trigger.
    pub fn time_span(&self) -> (f64, f64) {
        match (self.time.first(), self.time.last()) {
            (Some(&a), Some(&b)) => (a, b),
            _ => (0.0, 0.0),
        }
    }

    /// Indices of the samples with `t0 <= time <= t1`. Times within 1 ns of
    /// an edge count as inside, so a sample computed as -1.0000000000000009 ms
    /// is in a window starting at -1 ms.
    pub fn index_range(&self, t0: f64, t1: f64) -> Range<usize> {
        const EPS: f64 = 1e-9;
        let start = self.time.partition_point(|&t| t < t0 - EPS);
        let end = self.time.partition_point(|&t| t <= t1 + EPS);
        start..end.max(start)
    }

    /// Index of the sample closest to `t`.
    pub fn index_at(&self, t: f64) -> Option<usize> {
        if self.time.is_empty() {
            return None;
        }
        let i = self.time.partition_point(|&x| x < t);
        if i == 0 {
            return Some(0);
        }
        if i >= self.time.len() {
            return Some(self.time.len() - 1);
        }
        Some(if t - self.time[i - 1] <= self.time[i] - t {
            i - 1
        } else {
            i
        })
    }
}

/// Computes sample times in seconds relative to the trigger.
fn build_time(cfg: &Config, timestamps: &[u32]) -> Vec<f64> {
    let n = timestamps.len();
    let mut out = Vec::with_capacity(n);
    if cfg.has_fixed_rates() && !cfg.sample_rates.is_empty() {
        let rates = &cfg.sample_rates;
        let (mut seg, mut seg_t0, mut seg_i0) = (0, 0.0, 0usize);
        for i in 0..n {
            while seg + 1 < rates.len() && i >= rates[seg].last_sample {
                let last = rates[seg].last_sample;
                seg_t0 += (last.saturating_sub(1 + seg_i0)) as f64 / rates[seg].rate + 1.0 / rates[seg + 1].rate;
                seg_i0 = last;
                seg += 1;
            }
            out.push(seg_t0 + (i - seg_i0) as f64 / rates[seg].rate);
        }
    } else {
        let scale = cfg.time_mult * 1e-6;
        let mut prev: Option<f64> = None;
        let mut delta = 0.0;
        for &ts in timestamps {
            let t = if ts == dat::MISSING_TIMESTAMP {
                prev.map_or(0.0, |p| p + delta)
            } else {
                ts as f64 * scale
            };
            if let Some(p) = prev {
                delta = t - p;
            }
            prev = Some(t);
            out.push(t);
        }
    }
    let offset = cfg.trigger_offset();
    for t in &mut out {
        *t -= offset;
    }
    out
}

/// Rounds to 12 significant digits, so float noise (`0.2 * 6` =
/// `1.2000000000000002`) does not show; printing gives the shortest form.
pub fn round12(v: f64) -> f64 {
    format!("{v:.11e}").parse().unwrap_or(v)
}

/// Decodes text as UTF-8, falling back to Latin-1 for old files.
pub fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| Error::io(path, e))
}

fn read_text(path: impl AsRef<Path>) -> Result<String> {
    read_file(path.as_ref()).map(|b| decode_text(&b))
}

/// File content, memory mapped when possible.
enum FileData {
    Mapped(memmap2::Mmap),
    Owned(Vec<u8>),
}

impl Deref for FileData {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Mapped(m) => m,
            Self::Owned(v) => v,
        }
    }
}

fn map_file(path: &Path) -> Result<FileData> {
    let file = File::open(path).map_err(|e| Error::io(path, e))?;
    let len = file.metadata().map_err(|e| Error::io(path, e))?.len();
    if len == 0 {
        return Ok(FileData::Owned(Vec::new()));
    }
    // SAFETY: the map is read-only and dropped before this function's caller
    // returns; a concurrent truncation by another process is not guarded
    // against, as with any memory-mapped reader.
    match unsafe { memmap2::Mmap::map(&file) } {
        Ok(m) => Ok(FileData::Mapped(m)),
        Err(_) => read_file(path).map(FileData::Owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_text(rates: &str, nrates: usize) -> String {
        format!(
            "S,D,1999\n2,1A,1D\n1,IA,A,,A,2,1,0,-100,100,100,1,P\n1,T,,,0\n50\n{nrates}\n{rates}\n\
01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.002000\nASCII\n1\n"
        )
    }

    #[test]
    fn opens_from_readers_and_scales() {
        let cfg = cfg_text("1000,4", 1);
        let dat = "1,0,1,0\n2,1000,2,0\n3,2000,,1\n4,3000,4,1\n";
        let c = Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap();
        assert_eq!(c.len(), 4);
        assert_eq!(c.time(), &[-0.002, -0.001, 0.0, 0.001]);
        let a = c.analog(0).unwrap();
        assert_eq!(a.value(0), Some(3.0));
        assert_eq!(a.value(2), None);
        assert_eq!(a.value_on(0, Side::Secondary), Some(0.03));
        assert!(c.binary(0).unwrap().changes());
        assert_eq!(c.index_range(-0.0015, 0.0), 1..3);
        assert_eq!(c.index_at(0.0004), Some(2));
    }

    #[test]
    fn multiple_rates_and_timestamps() {
        let cfg = cfg_text("1000,2\n500,4", 2);
        let dat = "1,0,1,0\n2,0,1,0\n3,0,1,0\n4,0,1,0\n";
        let c = Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap();
        let t: Vec<f64> = c.time().iter().map(|t| (t * 1e6).round() / 1e3).collect();
        assert_eq!(t, [-2.0, -1.0, 1.0, 3.0]);

        // Slices keep the times and count the rate segments again.
        let ms = |c: &Comtrade| c.time().iter().map(|t| (t * 1e6).round() / 1e3).collect::<Vec<_>>();
        let s = c.slice(1..4, &[0], &[0]);
        assert_eq!(ms(&s), [-1.0, 1.0, 3.0]);
        assert_eq!(
            s.config.sample_rates,
            [
                SampleRate {
                    rate: 1000.0,
                    last_sample: 1
                },
                SampleRate {
                    rate: 500.0,
                    last_sample: 3
                }
            ]
        );
        assert_eq!(
            s.config.start_time,
            c.config.start_time + chrono::TimeDelta::milliseconds(1)
        );
        let s = c.slice(2..10, &[], &[0]);
        assert_eq!(ms(&s), [1.0, 3.0]);
        assert_eq!(
            s.config.sample_rates,
            [SampleRate {
                rate: 500.0,
                last_sample: 2
            }]
        );
        assert!(s.config.analog.is_empty());
        assert!(c.slice(9..12, &[0], &[]).is_empty());

        let cfg = cfg_text("0,4", 1);
        let dat = "1,0,1,0\n2,500,1,0\n3,,1,0\n4,2000,1,0\n";
        let c = Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap();
        let t: Vec<f64> = c.time().iter().map(|t| (t * 1e7).round() / 1e4).collect();
        assert_eq!(t, [-2.0, -1.5, -1.0, 0.0]);
    }

    #[test]
    fn plot_segments_decimate_and_split_on_gaps() {
        let cfg = cfg_text("1000,8", 1);
        let dat = "1,0,0,0\n2,0,5,0\n3,0,1,0\n4,0,,0\n5,0,2,0\n6,0,9,0\n7,0,3,0\n8,0,4,1\n";
        let c = Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap();
        let a = c.analog(0).unwrap();
        let span = c.time_span();
        let all = a.plot_segments(c.time(), 0..c.len(), span, 100, Side::Primary);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].len(), 3);
        // Two columns: min and max of each column survive.
        let dec = a.plot_segments(c.time(), 4..8, span, 1, Side::Primary);
        let ys: Vec<f64> = dec[0].iter().map(|p| p.1).collect();
        assert_eq!(ys, [5.0, 19.0]);

        let strip = c.binary(0).unwrap().strip(c.time(), span, 2);
        assert_eq!(strip, [Cell::Low, Cell::Mixed]);
    }

    #[test]
    fn latin1_fallback() {
        assert_eq!(decode_text(b"Gr\xfcn"), "Grün");
        assert_eq!(decode_text("Grün".as_bytes()), "Grün");
    }
}
