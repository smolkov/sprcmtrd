//! Phasors of the fundamental frequency and symmetrical components.
//!
//! A phasor is computed with a DFT over a window of whole cycles that ends at
//! a sample, as a protection relay sees the signal at that moment. The DFT
//! uses the sample times, so the angle is right even when a cycle is not a
//! whole number of samples. Magnitudes are RMS values.

use std::f64::consts::{FRAC_1_SQRT_2, PI};
use std::ops::{Add, Mul, Sub};

use serde::Serialize;

use super::{AnalogChannel, AnalogInfo, Side};

/// A complex number: the phasor of a signal (peak value).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Complex {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl Complex {
    /// From magnitude and angle in radians.
    pub fn from_polar(magnitude: f64, angle: f64) -> Self {
        Self {
            re: magnitude * angle.cos(),
            im: magnitude * angle.sin(),
        }
    }

    /// Magnitude.
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Angle in radians, in (-π, π].
    pub fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    fn scale(self, k: f64) -> Self {
        Self {
            re: self.re * k,
            im: self.im * k,
        }
    }
}

impl Add for Complex {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            re: self.re + o.re,
            im: self.im + o.im,
        }
    }
}

impl Sub for Complex {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self {
            re: self.re - o.re,
            im: self.im - o.im,
        }
    }
}

impl Mul for Complex {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
}

/// A phasor for display: RMS magnitude and angle in degrees relative to a
/// reference, in (-180, 180].
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Polar {
    /// RMS value.
    pub magnitude: f64,
    /// Angle in degrees.
    pub angle: f64,
}

impl Polar {
    /// `phasor` (peak value) as RMS and angle relative to `reference` (radians).
    pub fn new(phasor: Complex, reference: f64) -> Self {
        let mut angle = (phasor.arg() - reference).to_degrees();
        // Into (-180, 180], and no -0.
        angle = (angle + 180.0).rem_euclid(360.0) - 180.0;
        if angle <= -180.0 {
            angle += 360.0;
        }
        // Float residue around 0 would show as -0.0.
        if angle.abs() < 1e-9 {
            angle = 0.0;
        }
        Self {
            magnitude: phasor.abs() * FRAC_1_SQRT_2,
            angle,
        }
    }
}

/// Why a phasor could not be computed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PhasorError {
    /// The window would start before the first sample.
    #[error("the window starts before the recording")]
    BeforeStart,
    /// A sample in the window has no value.
    #[error("missing value at sample {0}")]
    Missing(usize),
}

/// Fundamental phasor (peak value) of `ch` over the `samples` samples ending
/// at sample `end` (inclusive), for `frequency` in Hz. `time` holds the
/// sample times of the recording.
pub fn phasor(
    ch: &AnalogChannel,
    time: &[f64],
    end: usize,
    samples: usize,
    frequency: f64,
    side: Side,
) -> Result<Complex, PhasorError> {
    let start = (end + 1).checked_sub(samples).ok_or(PhasorError::BeforeStart)?;
    let omega = 2.0 * PI * frequency;
    let mut sum = Complex::default();
    for (i, &t) in time.iter().enumerate().take(end + 1).skip(start) {
        let v = ch.value_on(i, side).ok_or(PhasorError::Missing(i + 1))?;
        // x(t) = A·cos(ωt + φ) gives A·e^{jφ}.
        sum = sum + Complex::from_polar(v, -omega * t);
    }
    Ok(sum.scale(2.0 / samples as f64))
}

/// Number of samples in `cycles` cycles of `frequency` at the sample spacing
/// around sample `end`, `None` without a usable spacing.
pub fn window(time: &[f64], end: usize, cycles: f64, frequency: f64) -> Option<usize> {
    let i = end.max(1).min(time.len().checked_sub(1)?);
    let dt = time[i] - time[i - 1];
    (dt > 0.0 && frequency > 0.0).then(|| (cycles / frequency / dt).round() as usize)
}

/// Symmetrical components of a three-phase set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sequence {
    /// Zero sequence: (A + B + C) / 3.
    pub zero: Complex,
    /// Positive sequence: (A + a·B + a²·C) / 3.
    pub positive: Complex,
    /// Negative sequence: (A + a²·B + a·C) / 3.
    pub negative: Complex,
}

/// Symmetrical components of the phasors `a`, `b`, `c` (phase order A, B, C).
pub fn symmetrical(a: Complex, b: Complex, c: Complex) -> Sequence {
    let op = Complex::from_polar(1.0, 2.0 * PI / 3.0);
    let op2 = op * op;
    Sequence {
        zero: (a + b + c).scale(1.0 / 3.0),
        positive: (a + op * b + op2 * c).scale(1.0 / 3.0),
        negative: (a + op2 * b + op * c).scale(1.0 / 3.0),
    }
}

/// Three channels of one circuit and unit with phases A, B and C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Circuit (`ccbm`) of the channels.
    pub circuit: String,
    /// Unit of the channels.
    pub unit: String,
    /// Positions of the A, B and C channels in the list given to [`groups`].
    pub phases: [usize; 3],
}

/// Phase letter of a channel: `A`/`B`/`C`, also `L1`/`L2`/`L3` and
/// `R`/`S`/`T`, as index 0..3.
fn phase_index(phase: &str) -> Option<usize> {
    match phase.trim().to_ascii_uppercase().as_str() {
        "A" | "L1" | "R" => Some(0),
        "B" | "L2" | "S" => Some(1),
        "C" | "L3" | "T" => Some(2),
        _ => None,
    }
}

/// Three-phase groups among `channels`: one channel each for phases A, B
/// and C with the same circuit and unit, in the order of their A channel.
pub fn groups(channels: &[&AnalogInfo]) -> Vec<Group> {
    let mut out: Vec<(Group, [Option<usize>; 3])> = Vec::new();
    for (pos, info) in channels.iter().enumerate() {
        let Some(p) = phase_index(&info.phase) else {
            continue;
        };
        let key = (info.circuit.as_str(), info.unit.as_str());
        let found = out
            .iter_mut()
            .find(|(g, slots)| (g.circuit.as_str(), g.unit.as_str()) == key && slots[p].is_none());
        match found {
            Some((_, slots)) => slots[p] = Some(pos),
            None => {
                let mut slots = [None; 3];
                slots[p] = Some(pos);
                let group = Group {
                    circuit: info.circuit.clone(),
                    unit: info.unit.clone(),
                    phases: [0; 3],
                };
                out.push((group, slots));
            }
        }
    }
    out.into_iter()
        .filter_map(|(mut g, slots)| {
            g.phases = [slots[0]?, slots[1]?, slots[2]?];
            Some(g)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comtrade::Comtrade;

    /// A 1 kHz recording of 50 Hz cosines with the given peak values and
    /// angles (degrees) on phases A, B, C.
    fn recording(signals: [(f64, f64); 3]) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n3,3A,0D\n");
        for (i, p) in ["A", "B", "C"].iter().enumerate() {
            cfg += &format!("{},I{p},{p},LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n", i + 1);
        }
        cfg += "50\n1\n1000,60\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.020000\nASCII\n1\n";
        let mut dat = String::new();
        for n in 0..60 {
            let t = n as f64 / 1000.0;
            dat += &format!("{},{}", n + 1, n * 1000);
            for (peak, deg) in signals {
                let v = peak * (2.0 * PI * 50.0 * t + deg.to_radians()).cos();
                dat += &format!(",{}", (v * 1000.0).round());
            }
            dat += "\n";
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn phasor_of_a_cosine() {
        let c = recording([(100.0, 30.0), (100.0, -90.0), (100.0, 150.0)]);
        let n = window(c.time(), 40, 1.0, 50.0).unwrap();
        assert_eq!(n, 20);
        let a = phasor(c.analog(0).unwrap(), c.time(), 40, n, 50.0, Side::Primary).unwrap();
        // Times are relative to the trigger (20 ms = one cycle), so the angle
        // is the one of the generated cosine.
        let p = Polar::new(a, 0.0);
        assert!(close(p.magnitude, 100.0 / 2f64.sqrt(), 0.01), "{p:?}");
        assert!(close(p.angle, 30.0, 0.05), "{p:?}");
        // Relative to phase A.
        let b = phasor(c.analog(1).unwrap(), c.time(), 40, n, 50.0, Side::Primary).unwrap();
        assert!(close(Polar::new(b, a.arg()).angle, -120.0, 0.05));

        assert_eq!(
            phasor(c.analog(0).unwrap(), c.time(), 10, n, 50.0, Side::Primary),
            Err(PhasorError::BeforeStart)
        );
    }

    #[test]
    fn symmetrical_components() {
        let p = |deg: f64| Complex::from_polar(1.0, deg.to_radians());
        // A balanced A-B-C set is all positive sequence.
        let s = symmetrical(p(0.0), p(-120.0), p(120.0));
        assert!(close(s.positive.abs(), 1.0, 1e-12) && s.negative.abs() < 1e-12 && s.zero.abs() < 1e-12);
        // A-C-B is negative sequence, equal phasors zero sequence.
        let s = symmetrical(p(0.0), p(120.0), p(-120.0));
        assert!(close(s.negative.abs(), 1.0, 1e-12) && s.positive.abs() < 1e-12);
        let s = symmetrical(p(10.0), p(10.0), p(10.0));
        assert!(close(s.zero.abs(), 1.0, 1e-12) && s.positive.abs() < 1e-12);

        assert_eq!(Polar::new(p(-179.0), 2f64.to_radians()).angle, 179.0);
        assert_eq!(Polar::new(p(180.0), 0.0).angle, 180.0);
    }

    #[test]
    fn groups_by_circuit_and_unit() {
        let info = |phase: &str, circuit: &str, unit: &str| AnalogInfo {
            index: 0,
            id: String::new(),
            phase: phase.into(),
            circuit: circuit.into(),
            unit: unit.into(),
            a: 1.0,
            b: 0.0,
            skew: 0.0,
            min: f64::NAN,
            max: f64::NAN,
            primary: 1.0,
            secondary: 1.0,
            side: Side::Primary,
        };
        let ch = [
            info("A", "L1", "A"),
            info("B", "L1", "A"),
            info("C", "L1", "A"),
            info("N", "L1", "A"),
            info("L1", "L1", "kV"),
            info("L2", "L1", "kV"),
            info("A", "L2", "A"),
            info("L3", "L1", "kV"),
        ];
        let refs: Vec<&AnalogInfo> = ch.iter().collect();
        let g = groups(&refs);
        assert_eq!(g.len(), 2, "{g:?}");
        assert_eq!((g[0].unit.as_str(), g[0].phases), ("A", [0, 1, 2]));
        assert_eq!((g[1].unit.as_str(), g[1].phases), ("kV", [4, 5, 7]));
    }
}
