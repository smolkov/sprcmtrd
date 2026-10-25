//! Apparent impedance of the six fault loops, as a distance relay measures
//! it: phase to ground with residual compensation, and phase to phase.

use super::Complex;
use super::sequence::{self, Group};
use crate::comtrade::AnalogInfo;

/// A fault loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loop {
    /// Phase A to ground.
    AN,
    /// Phase B to ground.
    BN,
    /// Phase C to ground.
    CN,
    /// Phase A to phase B.
    AB,
    /// Phase B to phase C.
    BC,
    /// Phase C to phase A.
    CA,
}

impl Loop {
    /// All loops in the usual order.
    pub const ALL: [Loop; 6] = [Loop::AN, Loop::BN, Loop::CN, Loop::AB, Loop::BC, Loop::CA];

    /// Name as written in tables (`A-N`).
    pub fn name(self) -> &'static str {
        match self {
            Self::AN => "A-N",
            Self::BN => "B-N",
            Self::CN => "C-N",
            Self::AB => "A-B",
            Self::BC => "B-C",
            Self::CA => "C-A",
        }
    }

    /// Parses `AN`, `A-N`, `an`, `AB`, … (`NA` and `BA` style too).
    pub fn parse(s: &str) -> Option<Self> {
        let letters: String = s
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_ascii_uppercase();
        let mut sorted: Vec<char> = letters.chars().collect();
        sorted.sort_unstable();
        match sorted.iter().collect::<String>().as_str() {
            "AN" => Some(Self::AN),
            "BN" => Some(Self::BN),
            "CN" => Some(Self::CN),
            "AB" => Some(Self::AB),
            "BC" => Some(Self::BC),
            "AC" => Some(Self::CA),
            _ => None,
        }
    }
}

/// Voltage and current of a loop, from the phase phasors (order A, B, C) and
/// the residual compensation factor `k0` = (Z0 − Z1) / (3·Z1):
/// phase to ground V / (I + k0·3I0), phase to phase (V₁ − V₂) / (I₁ − I₂).
pub fn loop_quantities(l: Loop, v: [Complex; 3], i: [Complex; 3], k0: Complex) -> (Complex, Complex) {
    let residual = i[0] + i[1] + i[2];
    let ground = |p: usize| (v[p], i[p] + k0 * residual);
    let phases = |p: usize, q: usize| (v[p] - v[q], i[p] - i[q]);
    match l {
        Loop::AN => ground(0),
        Loop::BN => ground(1),
        Loop::CN => ground(2),
        Loop::AB => phases(0, 1),
        Loop::BC => phases(1, 2),
        Loop::CA => phases(2, 0),
    }
}

/// Apparent impedance V / I, `None` without loop current.
pub fn impedance(v: Complex, i: Complex) -> Option<Complex> {
    (i.abs() > 0.0).then(|| v / i)
}

/// Factor from a unit to volts or amperes (`kV` → 1000), `None` for other units.
pub fn unit_factor(unit: &str) -> Option<(Quantity, f64)> {
    let (q, factor) = match unit.trim() {
        "V" => (Quantity::Voltage, 1.0),
        "kV" => (Quantity::Voltage, 1e3),
        "mV" => (Quantity::Voltage, 1e-3),
        "MV" => (Quantity::Voltage, 1e6),
        "A" => (Quantity::Current, 1.0),
        "kA" => (Quantity::Current, 1e3),
        "mA" => (Quantity::Current, 1e-3),
        _ => return None,
    };
    Some((q, factor))
}

/// Whether a unit is a voltage or a current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quantity {
    /// Volts.
    Voltage,
    /// Amperes.
    Current,
}

/// A voltage and a current three-phase group to measure the loops with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The voltage channels A, B, C (positions in the list given to [`find_pair`]).
    pub voltage: Group,
    /// The current channels A, B, C.
    pub current: Group,
}

/// The first voltage and current groups among `channels`, preferring a
/// current group of the same circuit as the voltage group.
pub fn find_pair(channels: &[&AnalogInfo]) -> Option<Pair> {
    let groups = sequence::groups(channels);
    let of = |q: Quantity| {
        groups
            .iter()
            .filter(move |g| unit_factor(&g.unit).is_some_and(|(k, _)| k == q))
    };
    let voltage = of(Quantity::Voltage).next()?.clone();
    let current = of(Quantity::Current)
        .find(|g| g.circuit == voltage.circuit)
        .or_else(|| of(Quantity::Current).next())?
        .clone();
    Some(Pair { voltage, current })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::phasor::tests::close;

    fn p(m: f64, deg: f64) -> Complex {
        Complex::from_polar(m, deg.to_radians())
    }

    #[test]
    fn loops_of_a_balanced_load_and_a_ground_fault() {
        // 63.5 kV and 318 A per phase, current 30° behind the voltage.
        let v = [p(63.5e3, 0.0), p(63.5e3, -120.0), p(63.5e3, 120.0)];
        let i = [p(318.0, -30.0), p(318.0, -150.0), p(318.0, 90.0)];
        for l in Loop::ALL {
            let (lv, li) = loop_quantities(l, v, i, Complex::default());
            let z = impedance(lv, li).unwrap();
            // Every loop sees the load impedance 63.5 kV / 318 A ∠ 30°.
            assert!(close(z.abs(), 63.5e3 / 318.0, 1e-6), "{l:?} {}", z.abs());
            assert!(close(z.arg().to_degrees(), 30.0, 1e-9), "{l:?}");
        }

        // B to ground with residual compensation: V_B / (I_B + k0·3I0).
        let i = [p(0.0, 0.0), p(1000.0, -80.0), p(0.0, 0.0)];
        let v = [p(63.5e3, 0.0), p(10e3, -120.0), p(63.5e3, 120.0)];
        let k0 = p(0.5, 0.0);
        let (lv, li) = loop_quantities(Loop::BN, v, i, k0);
        assert!(close(li.abs(), 1500.0, 1e-9));
        let z = impedance(lv, li).unwrap();
        assert!(close(z.abs(), 10e3 / 1500.0, 1e-9));
        assert!(close(z.arg().to_degrees(), -40.0, 1e-9));
        // No current: no impedance.
        assert_eq!(impedance(lv, Complex::default()), None);
    }

    #[test]
    fn loop_names_and_units() {
        assert_eq!(Loop::parse("an"), Some(Loop::AN));
        assert_eq!(Loop::parse("C-A"), Some(Loop::CA));
        assert_eq!(Loop::parse("AC"), Some(Loop::CA));
        assert_eq!(Loop::parse("AA"), None);
        assert_eq!(unit_factor("kV"), Some((Quantity::Voltage, 1e3)));
        assert_eq!(unit_factor("A"), Some((Quantity::Current, 1.0)));
        assert_eq!(unit_factor("Hz"), None);
    }
}
