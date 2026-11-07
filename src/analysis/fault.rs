//! Fault analysis of a recording: inception, faulted phases, interruption of
//! the fault current, peak currents, and the binary channels that give the
//! operate times (start, trip, breaker open).
//!
//! Inception is found from the superimposed current Δi(n) = i(n) − i(n − N),
//! N samples per cycle, as protection relays detect a fault: in steady state
//! it is near 0, a fault makes it jump. The faulted phases come from the
//! superimposed phasors ΔI = I(fault) − I(pre-fault), so load current does not
//! count as fault current.

use std::ops::Range;

use super::Complex;
use super::events;
use super::phasor::{self, Window};
use crate::comtrade::{AnalogChannel, BinaryChannel, Side};

/// Inception threshold: part of the largest pre-fault peak of the phases.
const PRE_FAULT_PART: f64 = 0.2;
/// Inception threshold at least: part of the largest current in the range
/// (unloaded line before the fault).
const MAX_PART: f64 = 0.02;
/// Samples in a row above the threshold that confirm an inception, so a
/// single spike is not a fault.
const CONFIRM: usize = 3;
/// A phase is faulted when its |ΔI| is at least this part of the largest.
const PHASE_PART: f64 = 0.5;
/// Ground is involved when 3·|ΔI0| is at least this part of the largest |ΔI|.
const GROUND_PART: f64 = 0.2;
/// The current is interrupted below this part of the phase's fault peak.
const INTERRUPTED_PART: f64 = 0.05;
/// Fewest samples per cycle, as in [`Window`].
const MIN_SAMPLES: usize = 4;

/// Phase names in order A, B, C.
pub const PHASES: [&str; 3] = ["A", "B", "C"];

/// Faulted phases and whether ground is involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultType {
    /// Phases A, B, C.
    pub phases: [bool; 3],
    /// Residual (ground) current flows.
    pub ground: bool,
}

impl FaultType {
    /// Name as protection engineers write it: `A-N`, `B-C`, `A-B-N`, `A-B-C`.
    pub fn name(self) -> String {
        let mut parts: Vec<&str> = PHASES
            .iter()
            .zip(self.phases)
            .filter_map(|(&p, on)| on.then_some(p))
            .collect();
        if self.ground {
            parts.push("N");
        }
        parts.join("-")
    }
}

/// The instantaneous peak of a current.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Peak {
    /// Sample (0-based).
    pub sample: usize,
    /// Absolute value.
    pub value: f64,
}

/// Result of [`analyse`]. Phasors are peak values in the channels' units.
#[derive(Debug, Clone, PartialEq)]
pub struct Fault {
    /// First sample of the fault (0-based).
    pub inception: usize,
    /// Samples per cycle used for Δi and the windows.
    pub per_cycle: usize,
    /// The cycle before inception.
    pub pre_window: Window,
    /// The cycle the fault phasors are measured over.
    pub fault_window: Window,
    /// Pre-fault phasors A, B, C; `None` with a missing value.
    pub pre: [Option<Complex>; 3],
    /// Fault phasors A, B, C.
    pub fault: [Option<Complex>; 3],
    /// Faulted phases.
    pub kind: FaultType,
    /// Peak of each phase from inception to the interruption (or the end).
    pub peaks: [Option<Peak>; 3],
    /// First sample with the current interrupted, `None` when the fault
    /// lasts to the end of the range.
    pub interruption: Option<usize>,
}

impl Fault {
    /// Superimposed phasor ΔI of each phase.
    pub fn delta(&self) -> [Option<Complex>; 3] {
        [0, 1, 2].map(|p| Some(self.fault[p]? - self.pre[p]?))
    }

    /// No current flowed before the inception: the line was energized, onto a
    /// fault or onto load, which the currents alone can't tell apart.
    pub fn energized(&self) -> bool {
        let largest = self.fault.iter().flatten().map(|p| p.abs()).fold(0.0, f64::max);
        self.pre.iter().all(|p| p.is_some_and(|p| p.abs() < MAX_PART * largest))
    }
}

/// Samples per cycle of `frequency` at sample `i`, from the sample spacing
/// there; `None` below 4 samples.
pub fn per_cycle(time: &[f64], i: usize, frequency: f64) -> Option<usize> {
    let i = i.max(1).min(time.len().checked_sub(1)?);
    let dt = time[i] - time[i - 1];
    if dt <= 0.0 || frequency <= 0.0 {
        return None;
    }
    let n = (1.0 / frequency / dt).round() as usize;
    (n >= MIN_SAMPLES).then_some(n)
}

/// Largest absolute value of `ch` in `range`.
fn max_abs(ch: &AnalogChannel, range: Range<usize>, side: Side) -> Option<Peak> {
    range
        .filter_map(|i| {
            ch.value_on(i, side).map(|v| Peak {
                sample: i,
                value: v.abs(),
            })
        })
        .fold(None, |best: Option<Peak>, p| match best {
            Some(b) if b.value >= p.value => Some(b),
            _ => Some(p),
        })
}

/// First sample of the fault in `range`: the first of [`CONFIRM`] samples in
/// a row where |Δi| of a phase is above the threshold. The first cycle of the
/// range is the pre-fault reference, so the search starts after it.
pub fn inception(currents: [&AnalogChannel; 3], range: Range<usize>, per_cycle: usize, side: Side) -> Option<usize> {
    let n = per_cycle;
    if range.len() <= n + CONFIRM {
        return None;
    }
    let peak = |r: Range<usize>| {
        currents
            .iter()
            .filter_map(|ch| max_abs(ch, r.clone(), side))
            .map(|p| p.value)
            .fold(0.0, f64::max)
    };
    let threshold = (PRE_FAULT_PART * peak(range.start..range.start + n)).max(MAX_PART * peak(range.clone()));
    if threshold <= 0.0 {
        return None;
    }
    let mut run = 0;
    for k in range.start + n..range.end {
        let above = currents.iter().any(|ch| {
            matches!((ch.value_on(k, side), ch.value_on(k - n, side)), (Some(a), Some(b)) if (a - b).abs() > threshold)
        });
        run = if above { run + 1 } else { 0 };
        if run == CONFIRM {
            return Some(k + 1 - CONFIRM);
        }
    }
    None
}

/// Faulted phases from the pre-fault and fault phasors; `None` when nothing
/// changed.
pub fn classify(pre: [Complex; 3], fault: [Complex; 3]) -> Option<FaultType> {
    let delta = [0, 1, 2].map(|p| fault[p] - pre[p]);
    let largest = delta.iter().map(|d| d.abs()).fold(0.0, f64::max);
    if largest <= 0.0 {
        return None;
    }
    let residual = (delta[0] + delta[1] + delta[2]).abs();
    Some(FaultType {
        phases: delta.map(|d| d.abs() >= PHASE_PART * largest),
        ground: residual >= GROUND_PART * largest,
    })
}

/// First sample in `range` after which the `faulted` currents stay below
/// [`INTERRUPTED_PART`] of their peak for a whole cycle, or until the end of
/// `range` when that is at least a quarter cycle.
pub fn interruption(
    currents: [&AnalogChannel; 3],
    faulted: [bool; 3],
    range: Range<usize>,
    per_cycle: usize,
    side: Side,
) -> Option<usize> {
    let chosen: Vec<(&AnalogChannel, f64)> = currents
        .iter()
        .zip(faulted)
        .filter(|(_, f)| *f)
        .filter_map(|(ch, _)| Some((*ch, max_abs(ch, range.clone(), side)?.value * INTERRUPTED_PART)))
        .collect();
    if chosen.is_empty() {
        return None;
    }
    let mut run = 0;
    for k in range.clone() {
        let below = chosen
            .iter()
            .all(|(ch, level)| ch.value_on(k, side).is_some_and(|v| v.abs() <= *level));
        run = if below { run + 1 } else { 0 };
        if run == per_cycle {
            return Some(k + 1 - per_cycle);
        }
    }
    // The recording may end soon after the breaker opened.
    (run > 0 && run >= per_cycle / 4).then(|| range.end - run)
}

/// Phasors of the three currents over `window`.
fn phasors(currents: [&AnalogChannel; 3], time: &[f64], window: &Window, side: Side) -> [Option<Complex>; 3] {
    currents.map(|ch| phasor::phasor(ch, time, window, side).ok())
}

/// The fault window: one cycle starting a quarter cycle after inception,
/// moved back to end at the last fault sample when the fault is shorter.
fn fault_window(inception: usize, n: usize, last: usize, frequency: f64) -> Window {
    let end = (inception + n / 4 + n - 1).min(last);
    let start = (end + 1).saturating_sub(n).max(inception);
    Window {
        end,
        samples: end + 1 - start,
        frequency,
    }
}

/// The [`fault_window`], moved on a cycle at a time while a current has a
/// missing value in it, as long as it ends by `last`. The first window when
/// none is complete.
fn complete_window(
    currents: [&AnalogChannel; 3],
    time: &[f64],
    (inception, n, last): (usize, usize, usize),
    frequency: f64,
    side: Side,
) -> Window {
    let first = fault_window(inception, n, last, frequency);
    let mut w = first;
    loop {
        if phasors(currents, time, &w, side).iter().all(Option::is_some) {
            return w;
        }
        if w.samples < n || w.end + n > last {
            return first;
        }
        w.end += n;
    }
}

/// Fault analysis of the `currents` (phases A, B, C) within `range`, for the
/// fundamental `frequency`; `None` when no fault is found. `time` holds the
/// sample times of the recording.
pub fn analyse(
    currents: [&AnalogChannel; 3],
    time: &[f64],
    range: Range<usize>,
    frequency: f64,
    side: Side,
) -> Option<Fault> {
    let range = range.start.min(time.len())..range.end.min(time.len());
    let n = per_cycle(time, range.start + 1, frequency)?;
    let inception = inception(currents, range.clone(), n, side)?;
    let pre_window = Window {
        end: inception - 1,
        samples: n,
        frequency,
    };
    let pre = phasors(currents, time, &pre_window, side);
    let classify_at = |w: &Window| {
        let fault = phasors(currents, time, w, side);
        let kind = classify(pre.map(Option::unwrap_or_default), fault.map(Option::unwrap_or_default))?;
        Some((fault, kind))
    };

    // The faulted phases decide where the current is interrupted, and a short
    // fault moves the window, so classify once more on the shorter window.
    let last = range.end - 1;
    let mut window = complete_window(currents, time, (inception, n, last), frequency, side);
    let (mut fault, mut kind) = classify_at(&window)?;
    let mut interrupted = interruption(currents, kind.phases, inception..range.end, n, side);
    if let Some(stop) = interrupted
        && stop <= window.end
    {
        let last = stop.saturating_sub(1).max(inception);
        window = complete_window(currents, time, (inception, n, last), frequency, side);
        (fault, kind) = classify_at(&window)?;
        interrupted = interruption(currents, kind.phases, inception..range.end, n, side);
    }
    let peak_end = interrupted.unwrap_or(range.end);
    Some(Fault {
        inception,
        per_cycle: n,
        pre_window,
        fault_window: window,
        pre,
        fault,
        kind,
        peaks: currents.map(|ch| max_abs(ch, inception..peak_end, side)),
        interruption: interrupted,
    })
}

/// A binary signal of the fault sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Protection start (pickup).
    Start,
    /// Trip command.
    Trip,
    /// Breaker open.
    BreakerOpen,
}

impl Role {
    /// All roles in the order they happen.
    pub const ALL: [Role; 3] = [Role::Start, Role::Trip, Role::BreakerOpen];

    /// Name as written in the output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Trip => "trip",
            Self::BreakerOpen => "breaker open",
        }
    }

    /// Whether a channel id looks like this role (case-insensitive).
    pub fn matches(self, id: &str) -> bool {
        let id = id.to_ascii_uppercase();
        match self {
            Self::Start => id.contains("START") || id.contains("PICKUP"),
            Self::Trip => id.contains("TRIP"),
            Self::BreakerOpen => id.contains("CB") || id.contains("BREAKER"),
        }
    }

    /// The state a channel changes to when the role happens: a breaker
    /// position named `…CLOSE…` goes to 0 when it opens, everything else to 1.
    pub fn active_state(self, id: &str) -> bool {
        !(self == Self::BreakerOpen && id.to_ascii_uppercase().contains("CLOSE"))
    }
}

/// When a role happened: the channel and the sample of its change.
#[derive(Debug, Clone, Copy)]
pub struct Operate<'a> {
    /// The role.
    pub role: Role,
    /// The channel that changed.
    pub channel: &'a BinaryChannel,
    /// First sample with the new state (0-based).
    pub sample: usize,
}

/// The first change of `channels` to the role's active state within
/// `range`, the earliest one when several channels are given.
pub fn operate<'a>(
    role: Role,
    channels: &[&'a BinaryChannel],
    time: &[f64],
    range: Range<usize>,
) -> Option<Operate<'a>> {
    channels
        .iter()
        .filter_map(|&ch| {
            let to = role.active_state(&ch.info.id);
            let e = events::events(&[ch], time, range.clone())
                .into_iter()
                .find(|e| e.to == to)?;
            Some(Operate {
                role,
                channel: ch,
                sample: e.sample - 1,
            })
        })
        .min_by_key(|o| o.sample)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::comtrade::Comtrade;

    /// A 1 kHz, 50 Hz recording from -100 ms to +200 ms, trigger at 0. Phases
    /// A, B, C carry 100 A load (peak, 0°, -120°, 120°); from `start` to
    /// `stop` (ms) phase p carries `fault[p]` A peak at -80° instead, and
    /// nothing after `stop`.
    fn recording(fault: [f64; 3], start: f64, stop: Option<f64>, load: f64) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n4,3A,1D\n");
        for (i, p) in PHASES.iter().enumerate() {
            cfg += &format!("{},I{p},{p},LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += "1,CB_CLOSED,,LINE,1\n";
        cfg += "50\n1\n1000,301\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.100000\nASCII\n1\n";
        let mut dat = String::new();
        for n in 0..301 {
            let ms = n as f64 - 100.0;
            let t = ms / 1e3;
            dat += &format!("{},{}", n + 1, n * 1000);
            for (p, f) in fault.iter().enumerate() {
                let shift = -(p as f64) * 2.0 * PI / 3.0;
                let (amp, angle) = match stop {
                    Some(s) if ms >= s => (0.0, 0.0),
                    _ if ms >= start && *f > 0.0 => (*f, shift - 80f64.to_radians()),
                    _ => (load, shift),
                };
                let v = amp * (2.0 * PI * 50.0 * t + angle).cos();
                dat += &format!(",{}", (v * 1000.0).round());
            }
            let closed = stop.is_none_or(|s| ms < s + 5.0);
            dat += &format!(",{}\n", u8::from(closed));
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    fn run(c: &Comtrade) -> Option<Fault> {
        let ch = |i| c.analog(i).unwrap();
        analyse([ch(0), ch(1), ch(2)], c.time(), 0..c.len(), 50.0, Side::Primary)
    }

    #[test]
    fn single_phase_to_ground() {
        let c = recording([2000.0, 0.0, 0.0], 0.0, Some(60.0), 100.0);
        let f = run(&c).unwrap();
        // The step at 0 ms is sample 100; Δi needs a moment to rise above 20 A.
        assert!((100..=103).contains(&f.inception), "{}", f.inception);
        assert_eq!(f.kind.name(), "A-N");
        assert_eq!(f.per_cycle, 20);
        assert_eq!(f.interruption, Some(160));
        assert!(!f.energized());
        let peak = f.peaks[0].unwrap();
        // No sample falls on the crest at 1 kHz: 2000 A · cos(8°).
        assert!(peak.value > 1950.0 && peak.value <= 2000.0, "{peak:?}");
        assert!(f.peaks[1].unwrap().value <= 100.0);
        // The fault phasor is the fault current, the pre-fault one the load.
        assert!((f.fault[0].unwrap().abs() - 2000.0).abs() < 1.0);
        assert!((f.pre[0].unwrap().abs() - 100.0).abs() < 1.0);
    }

    #[test]
    fn fault_types() {
        let kind = |fault: [f64; 3]| run(&recording(fault, 0.0, None, 100.0)).unwrap().kind.name();
        // Equal and opposite currents in B and C: no residual current.
        let bc = {
            let p = |m: f64, deg: f64| Complex::from_polar(m, deg.to_radians());
            let pre = [p(1.0, 0.0), p(1.0, -120.0), p(1.0, 120.0)];
            let fault = [pre[0], pre[1] + p(1000.0, -90.0), pre[2] + p(1000.0, 90.0)];
            classify(pre, fault).unwrap().name()
        };
        assert_eq!(bc, "B-C");
        assert_eq!(kind([0.0, 1500.0, 1500.0]), "B-C-N");
        assert_eq!(kind([2000.0, 2000.0, 2000.0]), "A-B-C");
        assert_eq!(kind([0.0, 0.0, 3000.0]), "C-N");
    }

    #[test]
    fn unloaded_line_and_no_fault() {
        // No load before the fault: the threshold comes from the fault current.
        let f = run(&recording([1000.0, 0.0, 0.0], 20.0, None, 0.0)).unwrap();
        assert!((120..=122).contains(&f.inception), "{}", f.inception);
        assert_eq!(f.kind.name(), "A-N");
        assert_eq!(f.interruption, None);
        // Currents alone can't tell an unloaded line from an energized one.
        assert!(f.energized());
        // Steady load and an empty recording have no fault.
        assert_eq!(run(&recording([0.0; 3], 0.0, None, 100.0)), None);
        assert_eq!(run(&recording([0.0; 3], 0.0, None, 0.0)), None);
    }

    #[test]
    fn short_fault_moves_the_window() {
        // Half a cycle of fault: the window ends at the last fault sample.
        let f = run(&recording([2000.0, 0.0, 0.0], 0.0, Some(10.0), 100.0)).unwrap();
        assert_eq!(f.interruption, Some(110));
        assert!(f.fault_window.end < 110, "{:?}", f.fault_window);
        assert_eq!(f.kind.phases, [true, false, false]);
    }

    #[test]
    fn breaker_role_follows_the_name() {
        let c = recording([2000.0, 0.0, 0.0], 0.0, Some(60.0), 100.0);
        let cb = c.binary(0).unwrap();
        assert!(Role::BreakerOpen.matches(&cb.info.id));
        assert!(!Role::Trip.matches(&cb.info.id));
        assert!(!Role::BreakerOpen.active_state("CB_CLOSED"));
        assert!(Role::BreakerOpen.active_state("CB_OPEN"));
        let o = operate(Role::BreakerOpen, &[cb], c.time(), 0..c.len()).unwrap();
        assert_eq!(o.sample, 165);
        assert!(operate(Role::Trip, &[], c.time(), 0..c.len()).is_none());
    }
}
