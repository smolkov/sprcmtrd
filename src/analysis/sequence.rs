//! Symmetrical components of three-phase sets.

use std::f64::consts::PI;

use super::Complex;
use crate::comtrade::AnalogInfo;

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

impl Sequence {
    /// Negative / positive sequence in %, `None` without positive sequence.
    pub fn unbalance(&self) -> Option<f64> {
        let positive = self.positive.abs();
        (positive > 0.0).then(|| self.negative.abs() / positive * 100.0)
    }
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

/// Symmetrical components of every three-phase group among `channels`,
/// from their `phasors` (same order; `None` where a phasor could not be
/// computed, which leaves its group out).
pub fn sequences(channels: &[&AnalogInfo], phasors: &[Option<Complex>]) -> Vec<(Group, Sequence)> {
    groups(channels)
        .into_iter()
        .filter_map(|g| {
            let [a, b, c] = g.phases.map(|i| phasors.get(i).copied().flatten());
            let s = symmetrical(a?, b?, c?);
            Some((g, s))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::phasor::tests::{close, recording};
    use crate::analysis::phasor::{Window, phasor};
    use crate::comtrade::Side;

    #[test]
    fn symmetrical_components() {
        let p = |deg: f64| Complex::from_polar(1.0, deg.to_radians());
        // A balanced A-B-C set is all positive sequence.
        let s = symmetrical(p(0.0), p(-120.0), p(120.0));
        assert!(close(s.positive.abs(), 1.0, 1e-12) && s.negative.abs() < 1e-12 && s.zero.abs() < 1e-12);
        assert!(s.unbalance().unwrap() < 1e-9);
        // A-C-B is negative sequence, equal phasors zero sequence.
        let s = symmetrical(p(0.0), p(120.0), p(-120.0));
        assert!(close(s.negative.abs(), 1.0, 1e-12) && s.positive.abs() < 1e-12);
        let s = symmetrical(p(10.0), p(10.0), p(10.0));
        assert!(close(s.zero.abs(), 1.0, 1e-12) && s.positive.abs() < 1e-12);
    }

    fn info(phase: &str, circuit: &str, unit: &str) -> AnalogInfo {
        AnalogInfo {
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
        }
    }

    #[test]
    fn groups_by_circuit_and_unit() {
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

    #[test]
    fn sequences_of_a_recording() {
        // Balanced 100 A peak, A-B-C order.
        let c = recording([(100.0, 0.0), (100.0, -120.0), (100.0, 120.0)]);
        let w = Window::ending_at(&c, 0.020, 1.0, 50.0).unwrap();
        let infos: Vec<_> = c.analog_channels().map(|ch| &ch.info).collect();
        let phasors: Vec<_> = c
            .analog_channels()
            .map(|ch| phasor(ch, c.time(), &w, Side::Primary).ok())
            .collect();
        let s = sequences(&infos, &phasors);
        assert_eq!(s.len(), 1);
        assert!(close(s[0].1.positive.abs(), 100.0, 0.1));
        assert!(s[0].1.unbalance().unwrap() < 0.1);
        // A missing phasor leaves the group out.
        assert!(sequences(&infos, &[phasors[0], None, phasors[2]]).is_empty());
    }
}
