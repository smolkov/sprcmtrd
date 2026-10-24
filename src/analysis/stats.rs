//! Statistics of channels over a range of samples: min, max, mean, RMS and
//! peak of analog channels, state changes of binary channels.
//!
//! Results are rounded to 12 significant digits ([`round12`](crate::comtrade::round12)), so float
//! noise from scaling and summing does not show.

use std::ops::Range;

use serde::Serialize;

use crate::comtrade::{AnalogChannel, BinaryChannel, Side, round12};

/// Statistics of an analog channel. Missing samples are left out; the
/// values are `None` when no sample has a value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalogStats {
    /// Channel index `An`.
    pub index: usize,
    /// Channel identifier.
    pub id: String,
    /// Unit of the values.
    pub unit: String,
    /// Samples with a value.
    pub samples: usize,
    /// Samples without a value.
    pub missing: usize,
    /// Smallest value.
    pub min: Option<f64>,
    /// Time of the first sample with the smallest value (s, relative to the trigger).
    pub min_time: Option<f64>,
    /// Largest value.
    pub max: Option<f64>,
    /// Time of the first sample with the largest value.
    pub max_time: Option<f64>,
    /// Arithmetic mean (the DC part).
    pub mean: Option<f64>,
    /// Root mean square over the samples (true RMS of the window).
    pub rms: Option<f64>,
    /// Value with the largest magnitude, with its sign.
    pub peak: Option<f64>,
    /// Time of the first sample with the peak magnitude.
    pub peak_time: Option<f64>,
}

/// Statistics of a binary channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BinaryStats {
    /// Channel index `Dn`.
    pub index: usize,
    /// Channel identifier.
    pub id: String,
    /// State at the first sample.
    pub first: bool,
    /// State at the last sample.
    pub last: bool,
    /// Number of state changes.
    pub changes: usize,
    /// Time of the first sample with a changed state (s, relative to the trigger).
    pub first_change: Option<f64>,
    /// Time spent in state 1 (s), counting each sample until the next one.
    pub time_set: f64,
}

/// Statistics of `ch` over the samples in `range`, with values on `side`.
/// `time` holds the sample times of the recording.
pub fn analog(ch: &AnalogChannel, time: &[f64], range: Range<usize>, side: Side) -> AnalogStats {
    let range = clamp(range, time.len());
    let mut s = AnalogStats {
        index: ch.info.index,
        id: ch.info.id.clone(),
        unit: ch.info.unit.clone(),
        samples: 0,
        missing: 0,
        min: None,
        min_time: None,
        max: None,
        max_time: None,
        mean: None,
        rms: None,
        peak: None,
        peak_time: None,
    };
    let (mut sum, mut sum_sq) = (0.0, 0.0);
    for i in range {
        let Some(v) = ch.value_on(i, side) else {
            s.missing += 1;
            continue;
        };
        s.samples += 1;
        sum += v;
        sum_sq += v * v;
        if s.min.is_none_or(|m| v < m) {
            (s.min, s.min_time) = (Some(v), Some(time[i]));
        }
        if s.max.is_none_or(|m| v > m) {
            (s.max, s.max_time) = (Some(v), Some(time[i]));
        }
        if s.peak.is_none_or(|p| v.abs() > p.abs()) {
            (s.peak, s.peak_time) = (Some(v), Some(time[i]));
        }
    }
    if s.samples > 0 {
        let n = s.samples as f64;
        let mean = sum / n;
        // The sum of a symmetric signal leaves rounding residue (1e-14 A).
        let floor = 1e-12 * s.peak.unwrap_or(0.0).abs();
        s.mean = Some(if mean.abs() <= floor { 0.0 } else { mean });
        s.rms = Some((sum_sq / n).sqrt());
    }
    for v in [
        &mut s.min,
        &mut s.min_time,
        &mut s.max,
        &mut s.max_time,
        &mut s.mean,
        &mut s.rms,
        &mut s.peak,
        &mut s.peak_time,
    ] {
        *v = v.map(round12);
    }
    s
}

/// Statistics of `ch` over the samples in `range`.
pub fn binary(ch: &BinaryChannel, time: &[f64], range: Range<usize>) -> BinaryStats {
    let range = clamp(range, time.len());
    let mut s = BinaryStats {
        index: ch.info.index,
        id: ch.info.id.clone(),
        first: range.clone().next().is_some_and(|i| ch.get(i)),
        last: range.clone().last().is_some_and(|i| ch.get(i)),
        changes: 0,
        first_change: None,
        time_set: 0.0,
    };
    for i in range.clone() {
        if i > range.start && ch.get(i) != ch.get(i - 1) {
            s.changes += 1;
            s.first_change.get_or_insert(time[i]);
        }
        if ch.get(i) && i + 1 < range.end {
            s.time_set += time[i + 1] - time[i];
        }
    }
    s.first_change = s.first_change.map(round12);
    s.time_set = round12(s.time_set);
    s
}

fn clamp(range: Range<usize>, len: usize) -> Range<usize> {
    let end = range.end.min(len);
    range.start.min(end)..end
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comtrade::Comtrade;

    fn recording() -> Comtrade {
        let cfg = "S,D,1999\n2,1A,1D\n1,IA,A,,A,2,1,0,-100,100,10,1,P\n1,T,,,0\n50\n1\n1000,5\n\
01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.002000\nASCII\n1\n";
        // IA = 2 * raw + 1: 3, -5, missing, 9, 1; T: 0 1 1 0 0
        let dat = "1,0,1,0\n2,1000,-3,1\n3,2000,,1\n4,3000,4,0\n5,4000,0,0\n";
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    #[test]
    fn analog_statistics_skip_missing_values() {
        let c = recording();
        let s = analog(c.analog(0).unwrap(), c.time(), 0..5, Side::Primary);
        assert_eq!((s.samples, s.missing), (4, 1));
        assert_eq!((s.min, s.min_time), (Some(-5.0), Some(-0.001)));
        assert_eq!((s.max, s.max_time), (Some(9.0), Some(0.001)));
        assert_eq!((s.peak, s.peak_time), (Some(9.0), Some(0.001)));
        assert_eq!(s.mean, Some(2.0));
        assert_eq!(s.rms, Some(round12(29.0_f64.sqrt())));

        // Secondary values: ratio 10:1.
        let s = analog(c.analog(0).unwrap(), c.time(), 0..2, Side::Secondary);
        assert_eq!((s.peak, s.peak_time), (Some(-0.5), Some(-0.001)));
        let s = analog(c.analog(0).unwrap(), c.time(), 2..3, Side::Primary);
        assert_eq!((s.samples, s.mean, s.rms), (0, None, None));
    }

    #[test]
    fn binary_statistics_count_changes() {
        let c = recording();
        let s = binary(c.binary(0).unwrap(), c.time(), 0..99);
        assert_eq!((s.first, s.last, s.changes), (false, false, 2));
        assert_eq!(s.first_change, Some(-0.001));
        assert_eq!(s.time_set, 0.002);
        let s = binary(c.binary(0).unwrap(), c.time(), 1..3);
        assert_eq!((s.first, s.last, s.changes, s.first_change), (true, true, 0, None));
    }
}
