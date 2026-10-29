//! System frequency over time and its rate of change (ROCOF), from the zero
//! crossings of a channel or from how far its phasor turns.
//!
//! Both methods measure over `N` cycles that end at a sample, like the
//! windows of [`super::phasor`]: the last `N` periods between rising zero
//! crossings, or the turn of the phasor between the window ending there and
//! the one ending `N` cycles earlier. The ROCOF compares the frequency there
//! with the one `N` cycles earlier.

use std::f64::consts::PI;
use std::ops::Range;

use serde::Serialize;

use super::Complex;
use super::phasor::{self, Window};
use crate::comtrade::{AnalogChannel, Side};

/// A signal below this part of the channel's largest value has no
/// frequency: no zero crossings are counted and its phasor is not used.
pub const MIN_LEVEL: f64 = 0.05;

/// How the frequency is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Time between rising zero crossings.
    Crossings,
    /// Turn of the fundamental phasor.
    Phasor,
}

/// Frequency of a phasor that turned from `p0` at `t0` to `p1` at `t1` (s),
/// computed for the fundamental `frequency`: a phasor of f₀ + Δf turns by
/// 2π·Δf·Δt, so f = f₀ + Δφ / (2π·Δt). Valid while |Δf|·Δt < ½; `None`
/// when `t1` is not after `t0`.
pub fn from_turn(frequency: f64, (t0, p0): (f64, Complex), (t1, p1): (f64, Complex)) -> Option<f64> {
    (t1 > t0).then(|| {
        let turn = (p1.arg() - p0.arg() + PI).rem_euclid(2.0 * PI) - PI;
        frequency + turn / (2.0 * PI * (t1 - t0))
    })
}

/// A rising zero crossing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    /// Time in s relative to the trigger, interpolated between the samples.
    pub time: f64,
    /// Run of samples without a missing value it belongs to; periods are only
    /// measured within one run.
    pub run: usize,
}

/// Rising zero crossings of `ch`. With hysteresis: after a crossing the
/// signal must fall below `-level` before the next one counts, so noise
/// around zero does not add crossings.
pub fn crossings(ch: &AnalogChannel, time: &[f64], level: f64, side: Side) -> Vec<Crossing> {
    let mut out = Vec::new();
    let (mut run, mut armed, mut previous) = (0, false, None::<f64>);
    for (i, &t) in time.iter().enumerate() {
        let Some(v) = ch.value_on(i, side) else {
            run += 1;
            armed = false;
            previous = None;
            continue;
        };
        if v < -level {
            armed = true;
        }
        if let Some(p) = previous
            && armed
            && p < 0.0
            && v >= 0.0
        {
            // Linear interpolation between the samples around zero.
            let t0 = time[i - 1];
            out.push(Crossing {
                time: t0 + (t - t0) * (-p / (v - p)),
                run,
            });
            armed = false;
        }
        previous = Some(v);
    }
    out
}

/// A measured frequency and the signal it was measured over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// Frequency in Hz.
    pub frequency: f64,
    /// Start of the measurement in s relative to the trigger.
    pub start: f64,
    /// End of the measurement in s relative to the trigger.
    pub end: f64,
}

impl Measured {
    /// The time the frequency stands for: the middle of the measurement.
    pub fn center(&self) -> f64 {
        (self.start + self.end) / 2.0
    }
}

/// Frequency from the last `periods` periods between the `crossings` up to
/// time `t`, for the nominal `frequency`. `None` without enough crossings,
/// across a missing value, or when the signal stopped: the last crossing
/// must be within 1.5 nominal periods before `t` (so down to ⅔ of nominal).
pub fn from_crossings(crossings: &[Crossing], t: f64, periods: usize, frequency: f64) -> Option<Measured> {
    let n = crossings.partition_point(|c| c.time <= t);
    let used = crossings.get(n.checked_sub(periods + 1)?..n)?;
    let (first, last) = (used.first()?, used.last()?);
    let span = last.time - first.time;
    (periods > 0 && first.run == last.run && span > 0.0 && t - last.time <= 1.5 / frequency).then(|| Measured {
        frequency: periods as f64 / span,
        start: first.time,
        end: last.time,
    })
}

/// The frequency at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Last sample used (0-based).
    pub end: usize,
    /// Time of that sample in s relative to the trigger.
    pub time: f64,
    /// Frequency in Hz, `None` where it can't be measured.
    pub frequency: Option<f64>,
    /// Rate of change in Hz/s against the measurement of the `N` cycles
    /// before, over the time between their middles.
    pub rocof: Option<f64>,
}

/// Frequency of `ch` at every `step`-th sample within `range`, measured over
/// `cycles` cycles of the nominal `frequency` (`samples` samples) with
/// `method`; [`Method::Crossings`] counts whole periods. The first point is
/// the first one where a measurement fits. `time` holds the sample times.
#[allow(clippy::too_many_arguments)]
pub fn series(
    ch: &AnalogChannel,
    time: &[f64],
    range: Range<usize>,
    cycles: f64,
    samples: usize,
    step: usize,
    frequency: f64,
    method: Method,
    side: Side,
) -> Vec<Point> {
    if samples == 0 || step == 0 {
        return Vec::new();
    }
    let peak = ch.values().flatten().map(|v| v.abs()).fold(0.0, f64::max);
    let level = MIN_LEVEL * peak;
    let crossings = match method {
        Method::Crossings => crossings(ch, time, level, side),
        Method::Phasor => Vec::new(),
    };
    let periods = (cycles.round() as usize).max(1);
    // Off the nominal frequency, the DFT of one phase has an image that
    // makes the angle ripple. Measured again with windows of whole cycles of
    // the frequency found, the image cancels (frequency tracking).
    let turn = |end: usize| {
        let mut m = turn_at(ch, time, end, samples, frequency, level, side)?;
        for _ in 0..TRACKING {
            if !(frequency / 2.0..frequency * 2.0).contains(&m.frequency) {
                break;
            }
            let tracked = (samples as f64 * frequency / m.frequency).round() as usize;
            m = turn_at(ch, time, end, tracked, m.frequency, level, side)?;
        }
        Some(m)
    };
    // The measurement at the sample `end` and the one of the cycles before it.
    let measure = |end: usize| match method {
        Method::Crossings => from_crossings(&crossings, time[end], periods, frequency),
        Method::Phasor => turn(end),
    };
    let before = |end: usize, m: &Measured| match method {
        Method::Crossings => from_crossings(&crossings, m.start, periods, frequency),
        Method::Phasor => turn(end.checked_sub(samples)?),
    };
    let end = range.end.min(time.len());
    let needed = match method {
        Method::Crossings => samples,
        Method::Phasor => 2 * samples,
    };
    let first = range.start.max(needed - 1);
    (first..end)
        .step_by(step)
        .map(|last| {
            let now = measure(last);
            let rocof = now.and_then(|m| {
                let m0 = before(last, &m)?;
                (m.center() > m0.center()).then(|| (m.frequency - m0.frequency) / (m.center() - m0.center()))
            });
            Point {
                end: last,
                time: time[last],
                frequency: now.map(|m| m.frequency),
                rocof,
            }
        })
        .collect()
}

/// Times the phasor method measures again at the frequency it found.
const TRACKING: usize = 2;

/// Frequency from the turn of the phasor of `ch` (DFT at `frequency`)
/// between the window of `samples` samples ending at `end` and the one
/// before it. A phasor below `level` (peak) is not used.
fn turn_at(
    ch: &AnalogChannel,
    time: &[f64],
    end: usize,
    samples: usize,
    frequency: f64,
    level: f64,
    side: Side,
) -> Option<Measured> {
    if samples < 4 {
        return None;
    }
    let at = |end: usize| {
        let window = Window {
            end,
            samples,
            frequency,
        };
        let p = phasor::phasor(ch, time, &window, side).ok()?;
        (p.abs() > 0.0 && p.abs() >= level).then_some((time[end], p))
    };
    // The turn covers both windows, from the start of the earlier one.
    let earlier = end.checked_sub(samples)?;
    Some(Measured {
        frequency: from_turn(frequency, at(earlier)?, at(end)?)?,
        start: time[(earlier + 1).checked_sub(samples)?],
        end: time[end],
    })
}

/// A value and the time it occurs at.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AtTime {
    /// The value.
    pub value: f64,
    /// Time in s relative to the trigger.
    pub time_s: f64,
}

/// Extremes and mean of a frequency series.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Summary {
    /// Lowest frequency in Hz.
    pub min: AtTime,
    /// Highest frequency in Hz.
    pub max: AtTime,
    /// Mean of the measured frequencies in Hz.
    pub mean: f64,
    /// ROCOF with the largest magnitude in Hz/s, `None` without one.
    pub rocof: Option<AtTime>,
}

/// Summary of `points`, `None` if none has a frequency.
pub fn summary(points: &[Point]) -> Option<Summary> {
    let measured: Vec<(f64, f64)> = points.iter().filter_map(|p| Some((p.time, p.frequency?))).collect();
    let at = |&(time_s, value): &(f64, f64)| AtTime { value, time_s };
    let min = measured.iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(at)?;
    let max = measured.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(at)?;
    let mean = measured.iter().map(|m| m.1).sum::<f64>() / measured.len() as f64;
    let rocof = points
        .iter()
        .filter_map(|p| Some((p.time, p.rocof?)))
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))
        .map(|m| at(&m));
    Some(Summary { min, max, mean, rocof })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::phasor::tests::close;
    use crate::comtrade::Comtrade;

    /// 1 kHz, 2 s, trigger at 0: one voltage channel of 90 kV peak (99999
    /// would be a missing value) whose frequency is `f(t)` Hz; zero from
    /// `off` s on.
    fn recording(f: impl Fn(f64) -> f64, off: f64) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n1,1A,0D\n");
        cfg += "1,VA,A,LINE,kV,0.001,0,0,-1e9,1e9,1,1,P\n";
        cfg += "50\n1\n1000,2000\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        let mut angle = 0.0f64;
        for n in 0..2000 {
            let t = n as f64 / 1000.0;
            let v = if t < off { 90.0 * angle.cos() } else { 0.0 };
            angle += 2.0 * PI * f(t) / 1000.0;
            dat += &format!("{},{},{}\n", n + 1, n * 1000, (v * 1000.0).round());
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    fn run(c: &Comtrade, cycles: usize, method: Method) -> Vec<Point> {
        let ch = c.analog(0).unwrap();
        series(
            ch,
            c.time(),
            0..c.len(),
            cycles as f64,
            20 * cycles,
            20,
            50.0,
            method,
            Side::Primary,
        )
    }

    #[test]
    fn constant_off_nominal_frequency() {
        let c = recording(|_| 49.5, 9.0);
        for method in [Method::Crossings, Method::Phasor] {
            let points = run(&c, 1, method);
            assert!(points.len() > 90);
            for p in points.iter().filter(|p| p.time >= 0.1) {
                assert!(close(p.frequency.unwrap(), 49.5, 0.02), "{method:?} {p:?}");
                // Over one cycle at 20 samples, the linear interpolation of the
                // crossings is good to a few mHz, which is 0.3 Hz/s.
                assert!(p.rocof.unwrap().abs() < 0.5, "{method:?} {p:?}");
            }
        }
    }

    #[test]
    fn falling_frequency_gives_rocof() {
        // 50 Hz until 0.5 s, then falling by 2 Hz/s.
        let c = recording(|t| if t < 0.5 { 50.0 } else { 50.0 - 2.0 * (t - 0.5) }, 9.0);
        // The mean over the last 5 cycles lags by 2.5 cycles, the turn between
        // two windows of 5 cycles by 5 cycles.
        for (method, lag, tolerance) in [
            (Method::Crossings, 0.05, (0.06, 0.1)),
            (Method::Phasor, 0.1, (0.02, 0.1)),
        ] {
            let points = run(&c, 5, method);
            let late: Vec<&Point> = points.iter().filter(|p| p.time >= 1.0).collect();
            for p in &late {
                let expected = 50.0 - 2.0 * (p.time - lag - 0.5);
                assert!(close(p.frequency.unwrap(), expected, tolerance.0), "{method:?} {p:?}");
                assert!(close(p.rocof.unwrap(), -2.0, tolerance.1), "{method:?} {p:?}");
            }
            let mean = late.iter().map(|p| p.rocof.unwrap()).sum::<f64>() / late.len() as f64;
            assert!(close(mean, -2.0, 0.1), "{method:?} {mean}");
            let s = summary(&points).unwrap();
            assert!(close(s.max.value, 50.0, 0.01) && s.min.value < 47.3, "{s:?}");
            assert!(s.rocof.unwrap().value < -1.8, "{s:?}");
        }
    }

    #[test]
    fn no_frequency_without_a_signal() {
        let c = recording(|_| 50.0, 1.0);
        for method in [Method::Crossings, Method::Phasor] {
            let points = run(&c, 1, method);
            assert!(
                points
                    .iter()
                    .filter(|p| p.time < 0.9)
                    .skip(2)
                    .all(|p| p.frequency.is_some())
            );
            assert!(
                points.iter().filter(|p| p.time > 1.1).all(|p| p.frequency.is_none()),
                "{method:?}"
            );
        }
        assert_eq!(summary(&[]), None);
    }

    #[test]
    fn crossings_with_hysteresis() {
        let c = recording(|_| 50.0, 9.0);
        let x = crossings(c.analog(0).unwrap(), c.time(), 5.0, Side::Primary);
        // The cosine starts at its peak: first rising crossing at 15 ms.
        assert_eq!(x.len(), 100);
        assert!(close(x[0].time, 0.015, 1e-4), "{:?}", x[0]);
        let m = from_crossings(&x, 1.0, 10, 50.0).unwrap();
        assert!(
            close(m.frequency, 50.0, 1e-6) && close(m.center(), 0.895, 1e-4),
            "{m:?}"
        );
        assert_eq!(from_crossings(&x, 0.02, 2, 50.0), None);
        // The signal stopped: the last crossing is 2 periods old.
        assert!(from_crossings(&x, 1.995 + 0.02, 1, 50.0).is_some());
        assert_eq!(from_crossings(&x, 1.995 + 0.04, 1, 50.0), None);
    }

    #[test]
    fn frequency_from_a_turn() {
        let p = |deg: f64| Complex::from_polar(1.0, deg.to_radians());
        // 18° in 20 ms: 0.05 turns / 20 ms = 2.5 Hz above 50 Hz.
        assert!(close(
            from_turn(50.0, (0.0, p(0.0)), (0.02, p(18.0))).unwrap(),
            52.5,
            1e-9
        ));
        assert!(close(
            from_turn(50.0, (0.0, p(170.0)), (0.02, p(-170.0))).unwrap(),
            52.777_777_777_8,
            1e-6
        ));
        assert_eq!(from_turn(50.0, (0.02, p(0.0)), (0.02, p(0.0))), None);
    }
}
