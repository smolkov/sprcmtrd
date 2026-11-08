//! Comparison of two recordings of the same event: time alignment, channel
//! pairs, phasors of B in A's time frame, and the differential current of two
//! line ends.
//!
//! Times: the alignment gives the offset `d` with t_A = t_B + d, so B's time
//! of A's moment t is t − d. Each recording's phasor angles refer to its own
//! time axis (see [`super::phasor::dft`]), so B's phasor in A's frame is
//! P_B · e^(−jω·d).

use std::f64::consts::TAU;

use super::Complex;
use super::fault;
use crate::comtrade::{AnalogChannel, Comtrade, Side};

/// How the time axes of A and B are aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// By the absolute trigger times (synchronized clocks).
    Time,
    /// Both triggers at 0.
    Trigger,
    /// The fault inception of A on that of B.
    Fault,
}

impl Align {
    /// Name for the output: `time`, `trigger`, `fault`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Trigger => "trigger",
            Self::Fault => "fault",
        }
    }
}

/// Why the recordings can't be aligned.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AlignError {
    /// The time spans have no common part.
    #[error(
        "the recordings don't overlap in time (B is {0:+.3} s from A); use --align trigger or --align fault, or --shift"
    )]
    NoOverlap(f64),
    /// No fault found in a recording (`A` or `B`).
    #[error("no fault found in {0}; use --align time or --align trigger")]
    NoFault(&'static str),
}

/// Seconds from A's trigger to B's trigger in absolute time.
pub fn trigger_difference(a: &Comtrade, b: &Comtrade) -> f64 {
    let d = b.config.trigger_time - a.config.trigger_time;
    d.num_microseconds()
        .map_or(d.num_seconds() as f64, |us| us as f64 * 1e-6)
}

/// Offset `d` (t_A = t_B + d) from the fault inceptions of the currents of A
/// and B (phases A, B, C).
pub fn fault_offset(
    a: (&Comtrade, [&AnalogChannel; 3]),
    b: (&Comtrade, [&AnalogChannel; 3]),
    frequency: f64,
    side: Side,
) -> Result<f64, AlignError> {
    let inception = |c: &Comtrade, currents, name| {
        fault::analyse(currents, c.time(), 0..c.len(), frequency, side)
            .map(|f| c.time()[f.inception])
            .ok_or(AlignError::NoFault(name))
    };
    Ok(inception(a.0, a.1, "A")? - inception(b.0, b.1, "B")?)
}

/// The offset `d` (t_A = t_B + d) for `align`, plus `shift` (s; moves B
/// later). An error when the aligned time spans don't overlap.
pub fn offset(
    a: &Comtrade,
    b: &Comtrade,
    align: Align,
    fault_offset: Option<f64>,
    shift: f64,
) -> Result<f64, AlignError> {
    let d = match align {
        Align::Time => trigger_difference(a, b),
        Align::Trigger => 0.0,
        Align::Fault => fault_offset.unwrap_or_default(),
    } + shift;
    let (a0, a1) = a.time_span();
    let (b0, b1) = b.time_span();
    if a0.max(b0 + d) > a1.min(b1 + d) {
        return Err(AlignError::NoOverlap(d));
    }
    Ok(d)
}

/// B's phasor `p` in A's time frame, for the offset `d` (s).
pub fn rotate(p: Complex, d: f64, frequency: f64) -> Complex {
    p * Complex::from_polar(1.0, -TAU * frequency * d)
}

/// Why the channels can't be paired.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairError {
    /// No analog channel id is in both.
    #[error("A and B have no analog channel with the same id; pair them with --pair A_ID=B_ID,...")]
    NoCommon,
    /// A channel of `--pair` doesn't exist.
    #[error("{which} has no analog channel '{id}'")]
    Unknown {
        /// `A` or `B`.
        which: &'static str,
        /// The id.
        id: String,
    },
}

/// Analog channel pairs (positions in A and in B): the `explicit` pairs of
/// ids, else every id of A that B has too (case-insensitive), in A's order.
pub fn pairs(a: &[&str], b: &[&str], explicit: &[(String, String)]) -> Result<Vec<(usize, usize)>, PairError> {
    let find = |ids: &[&str], id: &str| ids.iter().position(|x| x.trim().eq_ignore_ascii_case(id.trim()));
    if !explicit.is_empty() {
        return explicit
            .iter()
            .map(|(ia, ib)| {
                let pa = find(a, ia).ok_or_else(|| PairError::Unknown {
                    which: "A",
                    id: ia.clone(),
                })?;
                let pb = find(b, ib).ok_or_else(|| PairError::Unknown {
                    which: "B",
                    id: ib.clone(),
                })?;
                Ok((pa, pb))
            })
            .collect();
    }
    let found: Vec<(usize, usize)> = a
        .iter()
        .enumerate()
        .filter_map(|(i, id)| Some((i, find(b, id)?)))
        .collect();
    if found.is_empty() {
        return Err(PairError::NoCommon);
    }
    Ok(found)
}

/// Differential and restraint current of one phase (peak phasors, both
/// currents measured into the line).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Differential {
    /// |I_A + I_B|.
    pub diff: f64,
    /// (|I_A| + |I_B|) / 2.
    pub restraint: f64,
}

impl Differential {
    /// From the currents of both ends.
    pub fn new(a: Complex, b: Complex) -> Self {
        Self {
            diff: (a + b).abs(),
            restraint: (a.abs() + b.abs()) / 2.0,
        }
    }

    /// I_diff / I_restraint; `None` without current.
    pub fn ratio(&self) -> Option<f64> {
        (self.restraint > 0.0).then(|| self.diff / self.restraint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 50 Hz recording: `rate` samples per s from -0.1 s to +0.2 s around
    /// the trigger at `trigger` (`HH:MM:SS.ffffff` on 01/01/2020). Channel IA
    /// carries `amp`·cos(ωt_abs + `deg`), with t_abs the time since 00:00:00,
    /// from `fault_at` (absolute s) on 10 times as much.
    fn recording(rate: usize, trigger: f64, amp: f64, deg: f64, fault_at: f64) -> Comtrade {
        let n = rate * 3 / 10 + 1;
        let start = trigger - 0.1;
        let stamp = |t: f64| format!("01/01/2020,00:00:{:09.6}", t);
        let mut cfg = String::from("S,D,1999\n3,3A,0D\n");
        for (i, p) in ["A", "B", "C"].iter().enumerate() {
            cfg += &format!("{},I{p},{p},LINE,A,0.01,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += &format!("50\n1\n{rate},{n}\n{}\n{}\nASCII\n1\n", stamp(start), stamp(trigger));
        let mut dat = String::new();
        for k in 0..n {
            let t = start + k as f64 / rate as f64;
            dat += &format!("{},{}", k + 1, k);
            for p in 0..3 {
                let m = if t >= fault_at && p == 0 { amp * 10.0 } else { amp };
                let phase = deg.to_radians() - p as f64 * TAU / 3.0;
                let v = m * (TAU * 50.0 * t + phase).cos();
                dat += &format!(",{}", (v * 100.0).round());
            }
            dat += "\n";
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    fn phasor_at(c: &Comtrade, t: f64) -> Complex {
        let w = super::super::phasor::Window::ending_at(c, t, 1.0, 50.0).unwrap();
        super::super::phasor::phasor(c.analog(0).unwrap(), c.time(), &w, Side::Primary).unwrap()
    }

    fn degrees(z: Complex) -> f64 {
        z.arg().to_degrees()
    }

    #[test]
    fn time_alignment_rotates_the_phasor() {
        // The same signal, B triggered 12 ms later and sampled at 1 kHz
        // instead of 2 kHz.
        let a = recording(2000, 1.0, 100.0, 30.0, 9.0);
        let b = recording(1000, 1.012, 100.0, 30.0, 9.0);
        assert!((trigger_difference(&a, &b) - 0.012).abs() < 1e-9);
        let d = offset(&a, &b, Align::Time, None, 0.0).unwrap();
        let pa = phasor_at(&a, 0.04);
        let pb = rotate(phasor_at(&b, 0.04 - d), d, 50.0);
        assert!(
            (degrees(pa) - degrees(pb)).abs() < 0.5,
            "{} {}",
            degrees(pa),
            degrees(pb)
        );
        assert!((pa.abs() - pb.abs()).abs() < 0.5);
        // Trigger alignment: 12 ms are 216° = -144° at 50 Hz.
        let d = offset(&a, &b, Align::Trigger, None, 0.0).unwrap();
        let pb = rotate(phasor_at(&b, 0.04 - d), d, 50.0);
        let diff = (degrees(pb) - degrees(pa) + 540.0) % 360.0 - 180.0;
        assert!((diff + 144.0).abs() < 0.5, "{diff}");
        // --shift adds to the offset.
        let d = offset(&a, &b, Align::Trigger, None, 0.012).unwrap();
        assert!((d - 0.012).abs() < 1e-12);
    }

    #[test]
    fn fault_alignment_and_no_overlap() {
        // Unsynchronized clocks: B's clock runs 5 s behind, the fault is at
        // the same instant, 1.020 s on A's clock.
        let a = recording(2000, 1.0, 100.0, 0.0, 1.020);
        let b = recording(2000, 6.0, 100.0, 0.0, 6.020);
        fn ch(c: &Comtrade) -> [&AnalogChannel; 3] {
            [0, 1, 2].map(|i| c.analog(i).unwrap())
        }
        let f = fault_offset((&a, ch(&a)), (&b, ch(&b)), 50.0, Side::Primary).unwrap();
        assert!(f.abs() < 1e-3, "{f}");
        assert!(matches!(
            offset(&a, &b, Align::Time, None, 0.0),
            Err(AlignError::NoOverlap(_))
        ));
        assert!(offset(&a, &b, Align::Fault, Some(f), 0.0).is_ok());
        let quiet = recording(2000, 1.0, 100.0, 0.0, 99.0);
        assert_eq!(
            fault_offset((&a, ch(&a)), (&quiet, ch(&quiet)), 50.0, Side::Primary),
            Err(AlignError::NoFault("B"))
        );
    }

    #[test]
    fn channel_pairs() {
        let a = ["IA", "IB", "VA", "TEMP"];
        let b = ["va", "ib", "IN"];
        assert_eq!(pairs(&a, &b, &[]).unwrap(), [(1, 1), (2, 0)]);
        assert_eq!(pairs(&a, &["X"], &[]), Err(PairError::NoCommon));
        let explicit = [("IA".to_owned(), "IN".to_owned())];
        assert_eq!(pairs(&a, &b, &explicit).unwrap(), [(0, 2)]);
        let bad = [("IA".to_owned(), "IL1".to_owned())];
        assert!(matches!(
            pairs(&a, &b, &bad),
            Err(PairError::Unknown { which: "B", .. })
        ));
    }

    #[test]
    fn differential_current() {
        let p = |m: f64, deg: f64| Complex::from_polar(m, deg.to_radians());
        // Through current: into the line at A, out at B.
        let through = Differential::new(p(1000.0, -80.0), p(1000.0, 100.0));
        assert!(through.diff < 1e-9);
        assert!((through.restraint - 1000.0).abs() < 1e-9);
        // Internal fault, fed from both ends.
        let internal = Differential::new(p(3000.0, -80.0), p(1000.0, -70.0));
        assert!(internal.diff > 3900.0);
        assert!(internal.ratio().unwrap() > 1.9);
        assert_eq!(Differential::new(Complex::default(), Complex::default()).ratio(), None);
    }
}
