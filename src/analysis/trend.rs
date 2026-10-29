//! Values over time, step by step: phasors, true RMS, symmetrical
//! components and the system frequency for windows that end at regular
//! intervals.
//!
//! Each step uses the same kind of window as [`super::phasor`]: whole cycles
//! ending at a sample, as a protection relay sees the signals at that moment.

use std::ops::Range;

use super::Complex;
use super::frequency::from_turn;
use super::phasor::{self, Window};
use super::sequence::{self, Group, Sequence};
use crate::comtrade::{AnalogChannel, AnalogInfo, Side};

/// The values of one step.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// Last sample of the window (0-based).
    pub end: usize,
    /// Time of that sample in s relative to the trigger.
    pub time: f64,
    /// System frequency in Hz from the reference channel, `None` for the
    /// first step or without a usable reference phasor.
    pub frequency: Option<f64>,
    /// Fundamental phasor (peak value) per channel; `None` where a sample in
    /// the window has no value.
    pub phasors: Vec<Option<Complex>>,
    /// True RMS of all samples in the window per channel (fundamental,
    /// harmonics and DC).
    pub rms: Vec<Option<f64>>,
    /// Symmetrical components per group of [`Trend::groups`].
    pub sequences: Vec<Option<Sequence>>,
}

/// Steps of a trend and the three-phase groups their sequences belong to.
#[derive(Debug, Clone, PartialEq)]
pub struct Trend {
    /// Three-phase groups among the channels.
    pub groups: Vec<Group>,
    /// One entry per step, in time order.
    pub steps: Vec<Step>,
}

/// Trend of `channels` over the windows of `samples` samples that end at
/// every `step`-th sample within `range` (the first one where a whole window
/// fits), for the fundamental `frequency`. `reference` is the position in
/// `channels` the frequency is estimated from. `time` holds the sample times.
#[allow(clippy::too_many_arguments)]
pub fn trend(
    channels: &[&AnalogChannel],
    time: &[f64],
    range: Range<usize>,
    samples: usize,
    step: usize,
    frequency: f64,
    reference: usize,
    side: Side,
) -> Trend {
    let infos: Vec<&AnalogInfo> = channels.iter().map(|c| &c.info).collect();
    let groups = sequence::groups(&infos);
    let end = range.end.min(time.len());
    let first = range.start.max(samples.saturating_sub(1));
    let mut steps = Vec::new();
    let mut previous: Option<(f64, Complex)> = None;
    if samples == 0 || step == 0 {
        return Trend { groups, steps };
    }
    for last in (first..end).step_by(step) {
        let window = Window {
            end: last,
            samples,
            frequency,
        };
        let phasors: Vec<Option<Complex>> = channels
            .iter()
            .map(|ch| phasor::phasor(ch, time, &window, side).ok())
            .collect();
        let rms = channels
            .iter()
            .map(|ch| true_rms(ch, last + 1 - samples..last + 1, side))
            .collect();
        let sequences = groups
            .iter()
            .map(|g| {
                let [a, b, c] = g.phases.map(|i| phasors[i]);
                Some(sequence::symmetrical(a?, b?, c?))
            })
            .collect();

        let t = time[last];
        let current = phasors.get(reference).copied().flatten().filter(|p| p.abs() > 0.0);
        let estimated = match (previous, current) {
            (Some(p0), Some(p)) => from_turn(frequency, p0, (t, p)),
            _ => None,
        };
        previous = current.map(|p| (t, p));
        steps.push(Step {
            end: last,
            time: t,
            frequency: estimated,
            phasors,
            rms,
            sequences,
        });
    }
    Trend { groups, steps }
}

/// RMS of the values of `ch` in `range`, `None` if one is missing.
fn true_rms(ch: &AnalogChannel, range: Range<usize>, side: Side) -> Option<f64> {
    let n = range.len();
    let mut sum = 0.0;
    for i in range {
        let v = ch.value_on(i, side)?;
        sum += v * v;
    }
    (n > 0).then(|| (sum / n as f64).sqrt())
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::analysis::phasor::tests::close;
    use crate::comtrade::Comtrade;

    /// 1 kHz, 200 ms, trigger at 0: phases A, B, C at `f` Hz with 100 A peak;
    /// from 100 ms phase B has 300 A.
    fn recording(f: f64) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n3,3A,0D\n");
        for (i, p) in ["A", "B", "C"].iter().enumerate() {
            cfg += &format!("{},I{p},{p},LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += "50\n1\n1000,200\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        for n in 0..200 {
            let t = n as f64 / 1000.0;
            dat += &format!("{},{}", n + 1, n * 1000);
            for (k, deg) in [0.0f64, -120.0, 120.0].iter().enumerate() {
                let peak = if k == 1 && n >= 100 { 300.0 } else { 100.0 };
                let v = peak * (2.0 * PI * f * t + deg.to_radians()).cos();
                dat += &format!(",{}", (v * 1000.0).round());
            }
            dat += "\n";
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    #[test]
    fn steps_follow_the_signal() {
        let c = recording(50.0);
        let ch: Vec<_> = c.analog_channels().collect();
        let t = trend(&ch, c.time(), 0..200, 20, 20, 50.0, 0, Side::Primary);
        // Windows end at 19, 39, …, 199.
        assert_eq!(
            t.steps.iter().map(|s| s.end).collect::<Vec<_>>(),
            (19..200).step_by(20).collect::<Vec<_>>()
        );
        assert_eq!(t.groups.len(), 1);
        let first = &t.steps[0];
        assert_eq!(first.frequency, None);
        assert!(close(first.phasors[0].unwrap().abs(), 100.0, 0.1));
        assert!(close(first.rms[0].unwrap(), 100.0 / 2f64.sqrt(), 0.1));
        // Balanced before 100 ms, unbalanced after.
        assert!(first.sequences[0].unwrap().unbalance().unwrap() < 0.1);
        let last = t.steps.last().unwrap();
        assert!(close(last.phasors[1].unwrap().abs(), 300.0, 0.3));
        assert!(last.sequences[0].unwrap().unbalance().unwrap() > 20.0);
        assert!(close(last.frequency.unwrap(), 50.0, 0.01));
    }

    #[test]
    fn frequency_from_the_phasor_turn() {
        // 50.5 Hz analysed as 50 Hz: the phasor turns by 2π·0.5·Δt per step.
        let c = recording(50.5);
        let ch: Vec<_> = c.analog_channels().collect();
        let t = trend(&ch, c.time(), 0..100, 20, 10, 50.0, 0, Side::Primary);
        for s in &t.steps[1..] {
            assert!(close(s.frequency.unwrap(), 50.5, 0.05), "{:?}", s.frequency);
        }
    }

    #[test]
    fn no_steps_without_room_for_a_window() {
        let c = recording(50.0);
        let ch: Vec<_> = c.analog_channels().collect();
        assert!(
            trend(&ch, c.time(), 0..10, 20, 20, 50.0, 0, Side::Primary)
                .steps
                .is_empty()
        );
        assert!(
            trend(&ch, c.time(), 0..200, 20, 0, 50.0, 0, Side::Primary)
                .steps
                .is_empty()
        );
    }
}
