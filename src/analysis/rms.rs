//! Sliding RMS of a channel: for each sample the RMS over the window that
//! ends there, as a protection relay sees the signal at that moment.
//!
//! A window is found from the sample times, so recordings with several sample
//! rates work. It holds the same number of samples as a [`super::phasor`]
//! window of the same length.

use std::ops::Range;

use crate::comtrade::{AnalogChannel, Side};

/// What happens to the DC part of the signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dc {
    /// Included: the true RMS.
    Keep,
    /// Removed: the mean of the window is subtracted.
    Remove,
}

/// Fewest samples a window needs, as in [`super::phasor::Window`].
const MIN_SAMPLES: usize = 4;

/// Without DC, a variance below this part of the mean square is float noise
/// from the running totals, not signal. Below the resolution of a 16 bit
/// recorder, whose step is 3·10⁻⁵ of its full range.
const NOISE: f64 = 1e-10;

/// Sliding RMS of `ch` for each sample in `range`, over the window that ends
/// at the sample. `seconds` gives the window length at a sample. `None` where a
/// whole window doesn't fit, has fewer than 4 samples, or has a missing value.
/// `time` holds the sample times.
pub fn sliding(
    ch: &AnalogChannel,
    time: &[f64],
    range: Range<usize>,
    seconds: impl Fn(usize) -> f64,
    dc: Dc,
    side: Side,
) -> Vec<Option<f64>> {
    let range = range.start.min(time.len())..range.end.min(time.len());
    let starts: Vec<Option<usize>> = window_starts(time, range.clone(), seconds);
    let Some(first) = starts.iter().flatten().min().copied() else {
        return vec![None; range.len()];
    };

    // Running totals from the first window on make each window O(1), so the
    // cost is the same order as drawing the samples in view.
    let factor = ch.info.side_factor(side);
    let mut totals = Vec::with_capacity(range.end - first + 1);
    let (mut sum, mut squares, mut missing) = (0.0, 0.0, 0usize);
    totals.push((sum, squares, missing));
    for i in first..range.end {
        match ch.value(i) {
            Some(v) => {
                let v = v * factor;
                sum += v;
                squares += v * v;
            }
            None => missing += 1,
        }
        totals.push((sum, squares, missing));
    }

    range
        .zip(starts)
        .map(|(end, start)| {
            let start = start?;
            let (s0, q0, m0) = totals[start - first];
            let (s1, q1, m1) = totals[end + 1 - first];
            if m1 != m0 {
                return None;
            }
            let n = (end + 1 - start) as f64;
            let mean_square = ((q1 - q0) / n).max(0.0);
            Some(match dc {
                Dc::Keep => mean_square.sqrt(),
                Dc::Remove => {
                    let mean = (s1 - s0) / n;
                    let variance = mean_square - mean * mean;
                    if variance <= NOISE * mean_square {
                        0.0
                    } else {
                        variance.sqrt()
                    }
                }
            })
        })
        .collect()
}

/// First sample of the window ending at each sample in `range`, `None` where
/// no whole window fits. Windows start at `t_end - seconds + dt / 2`: half a
/// sample of tolerance, so a window holds `round(seconds / dt)` samples.
fn window_starts(time: &[f64], range: Range<usize>, seconds: impl Fn(usize) -> f64) -> Vec<Option<usize>> {
    let mut start = 0;
    range
        .map(|end| {
            let i = end.max(1);
            let dt = time[i.min(time.len() - 1)] - time[i - 1];
            let length = seconds(end);
            if time.len() < 2 || dt <= 0.0 || length <= 0.0 {
                return None;
            }
            let from = time[end] - length + dt / 2.0;
            // The slot of the sample before the window must be in the recording.
            if from <= time[0] - dt {
                return None;
            }
            // The next window starts close by, also when lengths vary.
            while start <= end && time[start] < from {
                start += 1;
            }
            while start > 0 && time[start - 1] >= from {
                start -= 1;
            }
            (end + 1 >= start + MIN_SAMPLES).then_some(start)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::analysis::frequency::Track;
    use crate::analysis::phasor::tests::close;
    use crate::comtrade::Comtrade;

    /// One channel with the values of `signal(n, t)` at `rate` Hz, with a line
    /// frequency of `line` Hz and a CT ratio of 100:1. Trigger at the start.
    fn recording(rate: f64, n: usize, line: f64, signal: impl Fn(usize, f64) -> Option<f64>) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n1,1A,0D\n1,IA,A,LINE,A,0.001,0,0,-1e9,1e9,100,1,P\n");
        cfg += &format!("{line}\n1\n{rate},{n}\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n");
        let mut dat = String::new();
        for k in 0..n {
            let t = k as f64 / rate;
            let value = signal(k, t).map_or(String::new(), |v| format!("{}", (v * 1000.0).round()));
            dat += &format!("{},{},{value}\n", k + 1, (t * 1e6).round());
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    fn sine(peak: f64, f: f64, t: f64) -> f64 {
        peak * (2.0 * PI * f * t).sin()
    }

    fn rms(c: &Comtrade, seconds: f64, dc: Dc) -> Vec<Option<f64>> {
        sliding(
            c.analog(0).unwrap(),
            c.time(),
            0..c.len(),
            |_| seconds,
            dc,
            Side::Primary,
        )
    }

    #[test]
    fn rms_of_a_sine_with_and_without_dc() {
        let c = recording(1000.0, 100, 50.0, |_, t| Some(sine(100.0, 50.0, t)));
        let v = rms(&c, 0.020, Dc::Keep);
        // No value until the first whole window of 20 samples.
        assert!(v[..19].iter().all(Option::is_none));
        for x in &v[19..] {
            assert!(close(x.unwrap(), 100.0 / 2f64.sqrt(), 0.01), "{x:?}");
        }

        let c = recording(1000.0, 100, 50.0, |_, t| Some(10.0 + sine(100.0, 50.0, t)));
        let keep = rms(&c, 0.020, Dc::Keep);
        let remove = rms(&c, 0.020, Dc::Remove);
        assert!(close(keep[50].unwrap(), (5000.0f64 + 100.0).sqrt(), 0.01));
        assert!(close(remove[50].unwrap(), 5000f64.sqrt(), 0.01));
    }

    #[test]
    fn pure_dc_is_zero_without_dc() {
        let c = recording(1000.0, 100, 50.0, |_, _| Some(5.0));
        assert!(close(rms(&c, 0.020, Dc::Keep)[50].unwrap(), 5.0, 1e-9));
        assert_eq!(rms(&c, 0.020, Dc::Remove)[50], Some(0.0));
    }

    #[test]
    fn a_step_reaches_the_new_value_one_cycle_later() {
        let c = recording(1000.0, 200, 50.0, |k, t| {
            Some(sine(if k >= 100 { 300.0 } else { 100.0 }, 50.0, t))
        });
        let v = rms(&c, 0.020, Dc::Keep);
        let new = 300.0 / 2f64.sqrt();
        assert!(close(v[99].unwrap(), 100.0 / 2f64.sqrt(), 0.01));
        assert!(!close(v[118].unwrap(), new, 0.01));
        assert!(close(v[119].unwrap(), new, 0.01));
    }

    #[test]
    fn windows_without_a_value() {
        // A missing sample leaves the 20 windows that hold it without a value.
        let c = recording(1000.0, 100, 50.0, |k, t| (k != 50).then(|| sine(100.0, 50.0, t)));
        let v = rms(&c, 0.020, Dc::Keep);
        assert!(v[49].is_some());
        assert!(v[50..70].iter().all(Option::is_none));
        assert!(v[70].is_some());

        // Fewer than 4 samples per window.
        let c = recording(100.0, 50, 50.0, |_, t| Some(sine(100.0, 50.0, t)));
        assert!(rms(&c, 0.020, Dc::Keep).iter().all(Option::is_none));
        assert!(sliding(c.analog(0).unwrap(), c.time(), 0..0, |_| 0.02, Dc::Keep, Side::Primary).is_empty());
    }

    #[test]
    fn same_samples_as_a_phasor_window() {
        // 60 Hz at 1 kHz: a cycle is 16.67 samples, a window holds 17.
        let c = recording(1000.0, 100, 60.0, |_, t| Some(sine(100.0, 60.0, t)));
        let ch = c.analog(0).unwrap();
        let v = rms(&c, 1.0 / 60.0, Dc::Keep);
        assert!(v[15].is_none());
        for end in [16, 40, 99] {
            let window = crate::analysis::Window::ending_at(&c, c.time()[end], 1.0, 60.0).unwrap();
            assert_eq!((window.end, window.samples), (end, 17));
            let square: f64 = (end + 1 - 17..=end).map(|i| ch.value(i).unwrap().powi(2)).sum();
            assert!(close(v[end].unwrap(), (square / 17.0).sqrt(), 1e-9));
        }
    }

    #[test]
    fn scaled_to_the_side() {
        let c = recording(1000.0, 100, 50.0, |_, t| Some(sine(100.0, 50.0, t)));
        let on = |side| sliding(c.analog(0).unwrap(), c.time(), 50..51, |_| 0.02, Dc::Keep, side)[0].unwrap();
        assert!(close(on(Side::Primary) / on(Side::Secondary), 100.0, 1e-9));
    }

    #[test]
    fn a_part_of_the_recording_gives_the_same_values() {
        let c = recording(1000.0, 100, 50.0, |k, t| Some(sine(100.0 + k as f64, 50.0, t)));
        let all = rms(&c, 0.020, Dc::Remove);
        let part = sliding(
            c.analog(0).unwrap(),
            c.time(),
            40..60,
            |_| 0.02,
            Dc::Remove,
            Side::Primary,
        );
        for (k, v) in part.iter().enumerate() {
            assert!(close(v.unwrap(), all[40 + k].unwrap(), 1e-9));
        }
        // Past the end.
        assert_eq!(
            sliding(
                c.analog(0).unwrap(),
                c.time(),
                99..500,
                |_| 0.02,
                Dc::Keep,
                Side::Primary
            )
            .len(),
            1
        );
    }

    #[test]
    fn several_sample_rates() {
        // 1 kHz, then 2 kHz: a cycle is 20 samples, then 40.
        let mut cfg = String::from("S,D,1999\n1,1A,0D\n1,IA,A,LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n");
        cfg += "50\n2\n1000,100\n2000,300\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        for k in 0..300 {
            let t = if k < 100 {
                k as f64 / 1000.0
            } else {
                0.1 + (k - 100) as f64 / 2000.0
            };
            let v = (sine(100.0, 50.0, t) * 1000.0).round();
            dat += &format!("{},{},{v}\n", k + 1, (t * 1e6).round());
        }
        let c = Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap();
        let v = rms(&c, 0.020, Dc::Keep);
        for end in [50, 299] {
            assert!(close(v[end].unwrap(), 100.0 / 2f64.sqrt(), 0.01), "{end}: {:?}", v[end]);
        }
    }

    #[test]
    fn a_window_fitted_to_the_signal_removes_the_ripple() {
        let c = recording(5000.0, 2000, 50.0, |_, t| Some(sine(100.0, 55.0, t)));
        let ch = c.analog(0).unwrap();
        let spread = |v: &[Option<f64>]| {
            let values: Vec<f64> = v[500..].iter().map(|x| x.unwrap()).collect();
            let (lo, hi) = values
                .iter()
                .fold((f64::INFINITY, 0.0f64), |(lo, hi), &x| (lo.min(x), hi.max(x)));
            (hi - lo) / (100.0 / 2f64.sqrt())
        };

        // A nominal window doesn't hold whole cycles of 55 Hz.
        assert!(spread(&rms(&c, 0.020, Dc::Keep)) > 0.05);

        let track = Track::new(&[ch], c.time(), 50.0).unwrap();
        let fitted = sliding(
            ch,
            c.time(),
            0..c.len(),
            |i| track.period_at(c.time()[i]),
            Dc::Keep,
            Side::Primary,
        );
        assert!(spread(&fitted) < 0.005, "{}", spread(&fitted));
    }
}
