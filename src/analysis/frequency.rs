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
use super::impedance::{Quantity, unit_factor};
use super::phasor::{self, Window};
use super::sequence;
use crate::comtrade::{AnalogChannel, AnalogInfo, Side};

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
/// must be within 1.5 nominal periods before `t`, and no period may be
/// longer (so down to ⅔ of nominal), which also leaves out a pause.
pub fn from_crossings(crossings: &[Crossing], t: f64, periods: usize, frequency: f64) -> Option<Measured> {
    let n = crossings.partition_point(|c| c.time <= t);
    let used = crossings.get(n.checked_sub(periods + 1)?..n)?;
    let (first, last) = (used.first()?, used.last()?);
    let span = last.time - first.time;
    let longest = 1.5 / frequency;
    let paused = used.windows(2).any(|w| w[1].time - w[0].time > longest);
    (periods > 0 && first.run == last.run && span > 0.0 && t - last.time <= longest && !paused).then(|| Measured {
        frequency: periods as f64 / span,
        start: first.time,
        end: last.time,
    })
}

/// The frequency of a recording over time, from the zero crossings of one
/// reference channel: the strongest voltage with a signal, otherwise the
/// strongest current, because voltages are the less disturbed signals.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// Position of the reference in the channels given to [`Track::new`].
    pub reference: usize,
    crossings: Vec<Crossing>,
    nominal: f64,
}

impl Track {
    /// Track for the nominal `frequency`, `None` when no voltage or current
    /// channel has a signal to measure. `time` holds the sample times.
    pub fn new(channels: &[&AnalogChannel], time: &[f64], frequency: f64) -> Option<Self> {
        let strongest = |quantity: Quantity| {
            let mut found: Vec<(usize, f64, f64)> = channels
                .iter()
                .enumerate()
                .filter_map(|(i, ch)| {
                    let (q, factor) = unit_factor(&ch.info.unit)?;
                    let peak = (0..time.len())
                        .filter_map(|k| ch.value_on(k, Side::Primary))
                        .fold(0.0, |peak: f64, v| peak.max(v.abs()));
                    (q == quantity && peak > 0.0).then_some((i, peak * factor, peak))
                })
                .collect();
            found.sort_by(|a, b| b.1.total_cmp(&a.1));
            found
        };
        let candidates = [Quantity::Voltage, Quantity::Current].into_iter().flat_map(strongest);
        candidates.into_iter().find_map(|(reference, _, peak)| {
            let crossings = crossings(channels[reference], time, MIN_LEVEL * peak, Side::Primary);
            (crossings.len() >= 2).then_some(Self {
                reference,
                crossings,
                nominal: frequency,
            })
        })
    }

    /// Length in s of the cycle that ends at time `t`: the last period between
    /// zero crossings, or the nominal one where no period can be measured or it
    /// is far off the nominal frequency.
    pub fn period_at(&self, t: f64) -> f64 {
        let highest = self.nominal * 1.5;
        from_crossings(&self.crossings, t, 1, self.nominal)
            .map(|m| m.frequency)
            .filter(|&f| f <= highest)
            .map_or(1.0 / self.nominal, |f| 1.0 / f)
    }
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
    let phasor = |w: &Window| phasor::phasor(ch, time, w, side).ok();
    let turn = |end: usize| tracked(&phasor, time, end, samples, frequency, level);
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

/// What a frequency is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The positive sequence of a three-phase group (positions of phases A,
    /// B, C in the channels): the image of the single phases cancels.
    Group([usize; 3]),
    /// One channel (position in the channels).
    Channel(usize),
}

/// A [`Source`] and the largest sample value of its channels; a phasor below
/// [`MIN_LEVEL`] of it is no signal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// The source.
    pub source: Source,
    /// Largest |sample| of its channels on the side measured.
    pub peak: f64,
}

/// The sources to measure the frequency from, best first: the positive
/// sequence of the voltage groups, of the current groups, then the single
/// voltages and the single currents; each kind by its largest value
/// (converted to V or A). Voltages first, as they are the less disturbed
/// signals. Channels without any value are left out.
pub fn sources(channels: &[&AnalogChannel], side: Side) -> Vec<Candidate> {
    let peaks: Vec<f64> = channels.iter().map(|ch| peak(ch, side)).collect();
    let quantity = |i: usize| unit_factor(&channels[i].info.unit);
    let infos: Vec<&AnalogInfo> = channels.iter().map(|ch| &ch.info).collect();
    let groups = sequence::groups(&infos);
    let mut out = Vec::new();
    for q in [Quantity::Voltage, Quantity::Current] {
        let mut found: Vec<(Candidate, f64)> = groups
            .iter()
            .filter_map(|g| {
                let (gq, factor) = quantity(g.phases[0])?;
                let peak = g.phases.map(|i| peaks[i]).into_iter().fold(0.0, f64::max);
                (gq == q && peak > 0.0).then_some((
                    Candidate {
                        source: Source::Group(g.phases),
                        peak,
                    },
                    peak * factor,
                ))
            })
            .collect();
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        out.extend(found.into_iter().map(|(c, _)| c));
    }
    for q in [Quantity::Voltage, Quantity::Current] {
        let mut found: Vec<(Candidate, f64)> = (0..channels.len())
            .filter_map(|i| {
                let (cq, factor) = quantity(i)?;
                (cq == q && peaks[i] > 0.0).then_some((
                    Candidate {
                        source: Source::Channel(i),
                        peak: peaks[i],
                    },
                    peaks[i] * factor,
                ))
            })
            .collect();
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        out.extend(found.into_iter().map(|(c, _)| c));
    }
    out
}

/// Largest |sample| of `ch` on `side`, 0 without values.
pub fn peak(ch: &AnalogChannel, side: Side) -> f64 {
    ch.values().flatten().map(|v| v.abs()).fold(0.0, f64::max) * ch.info.side_factor(side)
}

/// The frequency at the sample `end`, from the first of `candidates` that
/// has a signal in both windows (at least [`MIN_LEVEL`] of its peak): the
/// turn of its phasor between the window of `samples` samples ending at
/// `end` and the one before, measured again at the frequency found
/// (tracking). Returns the frequency and the position of the candidate used.
pub fn at(
    channels: &[&AnalogChannel],
    time: &[f64],
    end: usize,
    samples: usize,
    frequency: f64,
    candidates: &[Candidate],
    side: Side,
) -> Option<(f64, usize)> {
    candidates.iter().enumerate().find_map(|(n, c)| {
        let phasor = |w: &Window| -> Option<Complex> {
            let p = |i: usize| phasor::phasor(channels[i], time, w, side).ok();
            match c.source {
                Source::Channel(i) => p(i),
                Source::Group([a, b, cc]) => Some(sequence::symmetrical(p(a)?, p(b)?, p(cc)?).positive),
            }
        };
        let level = MIN_LEVEL * c.peak;
        tracked(&phasor, time, end, samples, frequency, level).map(|m| (m.frequency, n))
    })
}

/// [`turn_with`] at the nominal `frequency`, then measured again with
/// windows of whole cycles of the frequency found.
fn tracked(
    phasor: &dyn Fn(&Window) -> Option<Complex>,
    time: &[f64],
    end: usize,
    samples: usize,
    frequency: f64,
    level: f64,
) -> Option<Measured> {
    let mut m = turn_with(phasor, time, end, samples, frequency, level)?;
    for _ in 0..TRACKING {
        if !(frequency / 2.0..frequency * 2.0).contains(&m.frequency) {
            break;
        }
        let tracked = (samples as f64 * frequency / m.frequency).round() as usize;
        m = turn_with(phasor, time, end, tracked, m.frequency, level)?;
    }
    Some(m)
}

/// Frequency from the turn of `phasor` (DFT at `frequency`) between the
/// window of `samples` samples ending at `end` and the one before it. A
/// phasor below `level` (peak) is not used.
fn turn_with(
    phasor: &dyn Fn(&Window) -> Option<Complex>,
    time: &[f64],
    end: usize,
    samples: usize,
    frequency: f64,
    level: f64,
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
        let p = phasor(&window)?;
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
    fn no_frequency_across_a_pause() {
        // The signal stops for 0.5 s: no measurement spans the pause.
        let mut x: Vec<Crossing> = (0..10)
            .map(|k| Crossing {
                time: 0.02 * k as f64,
                run: 0,
            })
            .collect();
        x.extend((0..10).map(|k| Crossing {
            time: 0.7 + 0.02 * k as f64,
            run: 0,
        }));
        assert!(from_crossings(&x, 0.19, 5, 50.0).is_some());
        assert_eq!(from_crossings(&x, 0.75, 5, 50.0), None);
        let m = from_crossings(&x, 0.81, 5, 50.0).unwrap();
        assert!(close(m.frequency, 50.0, 1e-9), "{m:?}");
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

    /// Recording at 5 kHz, line frequency 50 Hz, with `(unit, signal(t))` channels.
    fn track_recording(signals: &[(&str, &dyn Fn(f64) -> f64)]) -> crate::comtrade::Comtrade {
        let mut cfg = format!("S,D,1999\n{0},{0}A,0D\n", signals.len());
        for (i, (unit, _)) in signals.iter().enumerate() {
            cfg += &format!("{},C{i},,,{unit},0.001,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += "50\n1\n5000,2000\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        for k in 0..2000 {
            let t = k as f64 / 5000.0;
            dat += &format!("{},{}", k + 1, (t * 1e6).round());
            for (_, signal) in signals {
                dat += &format!(",{}", (signal(t) * 1000.0).round());
            }
            dat += "\n";
        }
        crate::comtrade::Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    fn sine(peak: f64, f: f64) -> impl Fn(f64) -> f64 {
        move |t| peak * (2.0 * PI * f * t).sin()
    }

    fn track(c: &crate::comtrade::Comtrade) -> Option<Track> {
        let channels: Vec<_> = c.analog_channels().collect();
        Track::new(&channels, c.time(), 50.0)
    }

    #[test]
    fn track_prefers_the_strongest_voltage() {
        // A large current, and 1 kV is stronger than 500 V.
        let c = track_recording(&[
            ("A", &sine(5000.0, 50.0)),
            ("V", &sine(500.0, 50.0)),
            ("kV", &sine(1.0, 50.0)),
        ]);
        assert_eq!(track(&c).unwrap().reference, 2);
    }

    #[test]
    fn track_falls_back_to_the_strongest_current() {
        let c = track_recording(&[("V", &|_| 0.0), ("A", &sine(100.0, 50.0)), ("kA", &sine(1.0, 50.0))]);
        assert_eq!(track(&c).unwrap().reference, 2);
        // A voltage with no zero crossings doesn't count either.
        let c = track_recording(&[("V", &|_| 5.0), ("A", &sine(100.0, 50.0))]);
        assert_eq!(track(&c).unwrap().reference, 1);
    }

    #[test]
    fn track_needs_a_voltage_or_current_with_a_signal() {
        assert_eq!(track(&track_recording(&[("V", &|_| 0.0)])), None);
        assert_eq!(track(&track_recording(&[("Hz", &sine(100.0, 50.0))])), None);
    }

    #[test]
    fn period_follows_the_signal_or_stays_nominal() {
        let c = track_recording(&[("V", &sine(100.0, 55.0))]);
        let t = track(&c).unwrap();
        assert!((t.period_at(0.2) - 1.0 / 55.0).abs() < 1e-5, "{}", t.period_at(0.2));
        // Before two crossings.
        assert_eq!(t.period_at(0.001), 0.02);

        // During a pause: nominal again, once the last crossing is too old.
        let paused = |t: f64| {
            if (0.1..0.2).contains(&t) {
                0.0
            } else {
                sine(100.0, 55.0)(t)
            }
        };
        let t = track(&track_recording(&[("V", &paused)])).unwrap();
        assert_eq!(t.period_at(0.19), 0.02);

        // Far above the nominal frequency.
        let t = track(&track_recording(&[("V", &sine(100.0, 400.0))])).unwrap();
        assert_eq!(t.period_at(0.2), 0.02);
    }

    /// Recording at 4 kHz, 400 ms, line frequency 50 Hz, with
    /// `(id, phase, unit, signal(t))` channels.
    /// A channel: id, phase, unit and the signal over time.
    type Signal<'a> = (&'a str, &'a str, &'a str, &'a dyn Fn(f64) -> f64);

    fn phases_recording(signals: &[Signal]) -> Comtrade {
        let mut cfg = format!("S,D,1999\n{0},{0}A,0D\n", signals.len());
        for (i, (id, phase, unit, _)) in signals.iter().enumerate() {
            cfg += &format!("{},{id},{phase},LINE,{unit},0.001,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += "50\n1\n4000,1600\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        for k in 0..1600 {
            let t = k as f64 / 4000.0;
            dat += &format!("{},{}", k + 1, k * 250);
            for (.., signal) in signals {
                dat += &format!(",{}", (signal(t) * 1000.0).round());
            }
            dat += "\n";
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    /// Phase `k` (0, 1, 2) of a three-phase set at `f` Hz.
    fn phase(peak: f64, f: f64, k: usize) -> impl Fn(f64) -> f64 {
        move |t| peak * (2.0 * PI * f * t - k as f64 * 2.0 * PI / 3.0).cos()
    }

    fn frequencies(c: &Comtrade) -> Vec<Option<(f64, usize)>> {
        let ch: Vec<_> = c.analog_channels().collect();
        let candidates = sources(&ch, Side::Primary);
        (159..c.len())
            .step_by(80)
            .map(|end| at(&ch, c.time(), end, 80, 50.0, &candidates, Side::Primary))
            .collect()
    }

    #[test]
    fn sources_voltages_first_groups_before_channels() {
        let c = phases_recording(&[
            ("IA", "A", "A", &phase(80.0, 50.0, 0)),
            ("IB", "B", "A", &phase(80.0, 50.0, 1)),
            ("IC", "C", "A", &phase(80.0, 50.0, 2)),
            ("VX", "", "kV", &phase(1.0, 50.0, 0)),
            ("VA", "A", "V", &phase(90.0, 50.0, 0)),
            ("VB", "B", "V", &phase(90.0, 50.0, 1)),
            ("VC", "C", "V", &phase(90.0, 50.0, 2)),
        ]);
        let ch: Vec<_> = c.analog_channels().collect();
        let order: Vec<Source> = sources(&ch, Side::Primary).iter().map(|c| c.source).collect();
        assert_eq!(
            order,
            [
                Source::Group([4, 5, 6]),
                Source::Group([0, 1, 2]),
                // 1 kV is stronger than 90 V.
                Source::Channel(3),
                Source::Channel(4),
                Source::Channel(5),
                Source::Channel(6),
                Source::Channel(0),
                Source::Channel(1),
                Source::Channel(2),
            ]
        );
    }

    #[test]
    fn positive_sequence_has_no_image_off_nominal() {
        // 55 Hz measured with 50 Hz windows, 4 % unbalance (below 99.999,
        // which is a missing value).
        let c = phases_recording(&[
            ("VA", "A", "V", &phase(90.0, 55.0, 0)),
            ("VB", "B", "V", &phase(86.4, 55.0, 1)),
            ("VC", "C", "V", &phase(90.0, 55.0, 2)),
        ]);
        let found = frequencies(&c);
        assert_eq!(found.len(), 19);
        for f in &found {
            let (f, n) = f.unwrap();
            assert_eq!(n, 0, "{found:?}");
            assert!(close(f, 55.0, 0.005), "{f}");
        }
        // One phase alone: tracking keeps it close, too.
        let c = phases_recording(&[("VA", "A", "V", &phase(90.0, 55.0, 0))]);
        for f in frequencies(&c) {
            assert!(close(f.unwrap().0, 55.0, 0.01), "{f:?}");
        }
    }

    #[test]
    fn falls_back_to_the_currents_without_voltage() {
        // The voltages come on at 200 ms; before, the currents are measured.
        let on = |k: usize| move |t: f64| if t >= 0.2 { phase(90.0, 50.0, k)(t) } else { 0.0 };
        let c = phases_recording(&[
            ("VA", "A", "V", &on(0)),
            ("VB", "B", "V", &on(1)),
            ("VC", "C", "V", &on(2)),
            ("IA", "A", "A", &phase(5.0, 50.0, 0)),
            ("IB", "B", "A", &phase(5.0, 50.0, 1)),
            ("IC", "C", "A", &phase(5.0, 50.0, 2)),
        ]);
        let found = frequencies(&c);
        let used: Vec<usize> = found.iter().map(|f| f.unwrap().1).collect();
        // Windows end at 39.75 ms, 59.75 ms, …: both windows from 200 ms on
        // from the 11th (ending at 239.75 ms).
        assert_eq!(used, [[1; 10].as_slice(), &[0; 9]].concat());
        for f in found {
            assert!(close(f.unwrap().0, 50.0, 0.01), "{f:?}");
        }
        // Without any signal, nothing.
        let quiet = phases_recording(&[("VA", "A", "V", &|_| 0.0), ("IA", "A", "A", &|_| 0.0)]);
        assert!(frequencies(&quiet).iter().all(Option::is_none));
    }
}
