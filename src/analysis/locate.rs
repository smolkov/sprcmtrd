//! Single-ended fault location from the voltages and currents of one line
//! end: the reactance method and Takagi.
//!
//! The faulted loop sees V = d·z1·I + Rf·If, with the distance d, the line's
//! positive sequence impedance per km z1, the loop current I (ground loops
//! compensated with k0) and the fault resistance Rf carrying the fault
//! current If.
//!
//! - **Reactance:** d = Im(V / I) / Im(z1). Exact for a bolted fault; with Rf
//!   the term Rf·If/I adds reactance when If and I differ in phase (load flow,
//!   infeed from the other end).
//! - **Takagi:** multiplies with the conjugate of the superimposed current
//!   ΔI = I(fault) − I(pre-fault), which is nearly in phase with If, so the
//!   Rf term is (nearly) real and drops out of the imaginary part:
//!   d = Im(V·ΔI*) / Im(z1·I·ΔI*). Exact when the source and line impedance
//!   angles are equal (homogeneous system).

use super::Complex;
use super::fault::{Fault, FaultType};
use super::impedance::{Loop, loop_quantities};
use super::phasor::Window;

/// Residual compensation factor k0 = (Z0 − Z1) / (3·Z1).
pub fn k0(z1: Complex, z0: Complex) -> Complex {
    (z0 - z1) / z1.scale(3.0)
}

/// The loop that measures a fault: the phase to ground loop of a single
/// phase fault, else the loop between the first two faulted phases (any phase
/// to phase loop sees Z1 in a three-phase fault). `None` without a faulted
/// phase.
pub fn fault_loop(kind: FaultType) -> Option<Loop> {
    match kind.phases {
        [true, false, false] => Some(Loop::AN),
        [false, true, false] => Some(Loop::BN),
        [false, false, true] => Some(Loop::CN),
        [true, true, _] => Some(Loop::AB),
        [false, true, true] => Some(Loop::BC),
        [true, false, true] => Some(Loop::CA),
        [false, false, false] => None,
    }
}

/// Whether a loop is phase to ground (and needs k0).
pub fn is_ground(l: Loop) -> bool {
    matches!(l, Loop::AN | Loop::BN | Loop::CN)
}

/// Loop quantities for the location: voltage, (compensated) current and
/// the superimposed current of the loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopValues {
    /// Loop voltage.
    pub v: Complex,
    /// Loop current, ground loops with I + k0·3I0.
    pub i: Complex,
    /// Superimposed loop current: ΔI of the phase for ground loops, the
    /// difference of the two phases' ΔI for phase loops.
    pub delta: Complex,
}

/// The values of loop `l` from the fault phasors `v`, `i` and the pre-fault
/// currents `pre` (phases A, B, C).
pub fn loop_values(l: Loop, v: [Complex; 3], i: [Complex; 3], pre: [Complex; 3], k0: Complex) -> LoopValues {
    let (lv, li) = loop_quantities(l, v, i, k0);
    let d = [0, 1, 2].map(|p| i[p] - pre[p]);
    let delta = match l {
        Loop::AN => d[0],
        Loop::BN => d[1],
        Loop::CN => d[2],
        Loop::AB => d[0] - d[1],
        Loop::BC => d[1] - d[2],
        Loop::CA => d[2] - d[0],
    };
    LoopValues { v: lv, i: li, delta }
}

/// Distance in km with the reactance method; `None` without loop current.
pub fn reactance(m: &LoopValues, z1: Complex) -> Option<f64> {
    (m.i.abs() > 0.0 && z1.im > 0.0).then(|| (m.v / m.i).im / z1.im)
}

/// Distance in km with Takagi; `None` without superimposed current.
pub fn takagi(m: &LoopValues, z1: Complex) -> Option<f64> {
    let d = m.delta.conj();
    let den = (z1 * m.i * d).im;
    (m.delta.abs() > 0.0 && den != 0.0).then(|| (m.v * d).im / den)
}

/// The measurement window of `samples` samples: ending in the middle of the
/// fault, but starting at least one cycle after inception, when the DC offset
/// and the transition have decayed. For a fault too short for that, the
/// fault window of [`Fault`]. `range_end` is the end of the search range.
pub fn window(f: &Fault, range_end: usize, samples: usize) -> Window {
    let last = f.interruption.unwrap_or(range_end).saturating_sub(1);
    let earliest = f.inception + f.per_cycle + samples - 1;
    if samples < 4 || earliest > last {
        return f.fault_window;
    }
    Window {
        end: ((f.inception + last) / 2).max(earliest),
        samples,
        frequency: f.fault_window.frequency,
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::analysis::fault::analyse;
    use crate::analysis::phasor::{self, tests::close};
    use crate::comtrade::{Comtrade, Side};

    fn c(re: f64, im: f64) -> Complex {
        Complex { re, im }
    }

    fn polar(m: f64, deg: f64) -> Complex {
        Complex::from_polar(m, deg.to_radians())
    }

    /// Line data of the model: Ω/km and km.
    const Z1: Complex = Complex { re: 0.03, im: 0.4 };
    const Z0: Complex = Complex { re: 0.1, im: 1.2 };
    const LENGTH: f64 = 50.0;

    /// Kinds of faults in the model.
    #[derive(Clone, Copy)]
    enum Kind {
        /// A to ground.
        Ground,
        /// B to C.
        PhasePhase,
        /// A, B and C.
        ThreePhase,
    }

    /// A line between two sources S (the relay end) and R, with source
    /// impedances `ks`·z, `kr`·z (in km of line, so the system is homogeneous
    /// when both sequences use the same factors). S leads R by `load` degrees.
    /// Returns the phasors at S before and during a fault at `d` km with
    /// resistance `rf`: (pre V, pre I, fault V, fault I), phases A, B, C.
    fn model(kind: Kind, d: f64, rf: f64, load: f64, (ks, kr): (f64, f64)) -> [[Complex; 3]; 4] {
        let e = 90e3;
        let (es, er) = (polar(e, load), polar(e, 0.0));
        let zs = [Z0.scale(ks), Z1.scale(ks)];
        let zr = [Z0.scale(kr), Z1.scale(kr)];
        let zl = [Z0.scale(LENGTH), Z1.scale(LENGTH)];
        let m = d / LENGTH;
        // Index 0: zero sequence, 1: positive and negative sequence.
        let i_pre = (es - er) / (zs[1] + zl[1] + zr[1]);
        let v_pre = es - zs[1] * i_pre;
        let vf = es - (zs[1] + zl[1].scale(m)) * i_pre;
        let near = |k: usize| zs[k] + zl[k].scale(m);
        let far = |k: usize| zr[k] + zl[k].scale(1.0 - m);
        let zf = |k: usize| near(k) * far(k) / (near(k) + far(k));
        let share = |k: usize| far(k) / (near(k) + far(k));
        let rf_c = c(rf, 0.0);
        // Fault current in sequences 0, 1, 2.
        let f = match kind {
            Kind::Ground => {
                let i = vf / (zf(0) + zf(1) + zf(1) + rf_c.scale(3.0));
                [i, i, i]
            }
            Kind::PhasePhase => {
                let i = vf / (zf(1) + zf(1) + rf_c);
                [Complex::default(), i, Complex::default() - i]
            }
            Kind::ThreePhase => [Complex::default(), vf / (zf(1) + rf_c), Complex::default()],
        };
        let di = [f[0] * share(0), f[1] * share(1), f[2] * share(1)];
        let i_seq = [di[0], i_pre + di[1], di[2]];
        let v_seq = [
            Complex::default() - zs[0] * di[0],
            v_pre - zs[1] * di[1],
            Complex::default() - zs[1] * di[2],
        ];
        let a = polar(1.0, 120.0);
        let a2 = a * a;
        let phases = |s: [Complex; 3]| {
            [
                s[0] + s[1] + s[2],
                s[0] + a2 * s[1] + a * s[2],
                s[0] + a * s[1] + a2 * s[2],
            ]
        };
        let balanced = |p: Complex| [p, a2 * p, a * p];
        [balanced(v_pre), balanced(i_pre), phases(v_seq), phases(i_seq)]
    }

    /// The model as a 1 kHz, 50 Hz recording from -100 ms to +200 ms with the
    /// fault from 0 on (VA VB VC in V, IA IB IC in A).
    fn recording(p: [[Complex; 3]; 4]) -> Comtrade {
        let mut cfg = String::from("S,D,1999\n6,6A,0D\n");
        for (i, ph) in ["A", "B", "C"].iter().enumerate() {
            cfg += &format!("{},V{ph},{ph},LINE,V,0.001,0,0,-1e12,1e12,1,1,P\n", i + 1);
        }
        for (i, ph) in ["A", "B", "C"].iter().enumerate() {
            cfg += &format!("{},I{ph},{ph},LINE,A,0.001,0,0,-1e12,1e12,1,1,P\n", i + 4);
        }
        cfg += "50\n1\n1000,301\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.100000\nASCII\n1\n";
        let mut dat = String::new();
        for n in 0..301 {
            let t = (n as f64 - 100.0) / 1e3;
            let [v, i] = if t < 0.0 { [p[0], p[1]] } else { [p[2], p[3]] };
            dat += &format!("{},{}", n + 1, n * 1000);
            for x in v.iter().chain(&i) {
                let value = x.abs() * (2.0 * PI * 50.0 * t + x.arg()).cos();
                dat += &format!(",{}", (value * 1000.0).round());
            }
            dat += "\n";
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    /// Locates the fault in the recording of the model: (loop, reactance km, Takagi km).
    fn locate(p: [[Complex; 3]; 4]) -> (Loop, f64, f64) {
        let rec = recording(p);
        let ch = |k: usize| rec.analog(k).unwrap();
        let f = analyse([ch(3), ch(4), ch(5)], rec.time(), 0..rec.len(), 50.0, Side::Primary).unwrap();
        let l = fault_loop(f.kind).unwrap();
        let w = window(&f, rec.len(), f.per_cycle);
        let at = |k: usize, w: &Window| phasor::phasor(ch(k), rec.time(), w, Side::Primary).unwrap();
        let v = [0, 1, 2].map(|k| at(k, &w));
        let i = [3, 4, 5].map(|k| at(k, &w));
        let pre = [3, 4, 5].map(|k| at(k, &f.pre_window));
        let m = loop_values(l, v, i, pre, k0(Z1, Z0));
        (l, reactance(&m, Z1).unwrap(), takagi(&m, Z1).unwrap())
    }

    #[test]
    fn bolted_faults_on_both_methods() {
        for (kind, name) in [
            (Kind::Ground, Loop::AN),
            (Kind::PhasePhase, Loop::BC),
            (Kind::ThreePhase, Loop::AB),
        ] {
            let (l, x, t) = locate(model(kind, 20.0, 0.0, 10.0, (20.0, 30.0)));
            assert_eq!(l, name);
            // 0.5 % of the line.
            assert!(close(x, 20.0, 0.25), "{name:?} reactance {x}");
            assert!(close(t, 20.0, 0.25), "{name:?} Takagi {t}");
        }
    }

    #[test]
    fn takagi_removes_the_fault_resistance() {
        // 10 Ω to ground with S exporting 20° ahead of R.
        let (_, x, t) = locate(model(Kind::Ground, 30.0, 10.0, 20.0, (20.0, 30.0)));
        assert!(close(t, 30.0, 0.5), "Takagi {t}");
        assert!((x - 30.0).abs() > 2.0, "reactance {x} should be off");
        // Phase to phase through 5 Ω.
        let (_, _, t) = locate(model(Kind::PhasePhase, 12.0, 5.0, 20.0, (20.0, 30.0)));
        assert!(close(t, 12.0, 0.5), "Takagi {t}");
    }

    #[test]
    fn beyond_the_line_and_without_change() {
        // A fault behind the remote source looks like more than the line.
        let (_, x, _) = locate(model(Kind::ThreePhase, 70.0, 0.0, 0.0, (20.0, 30.0)));
        assert!(x > LENGTH, "{x}");
        // No superimposed current: no Takagi result.
        let m = LoopValues {
            v: c(1.0, 1.0),
            i: c(1.0, 0.0),
            delta: Complex::default(),
        };
        assert_eq!(takagi(&m, Z1), None);
        assert!(close(reactance(&m, Z1).unwrap(), 1.0 / 0.4, 1e-12));
    }

    #[test]
    fn loops_and_k0() {
        let kind = |phases, ground| FaultType { phases, ground };
        assert_eq!(fault_loop(kind([false, true, false], true)), Some(Loop::BN));
        assert_eq!(fault_loop(kind([true, false, true], true)), Some(Loop::CA));
        assert_eq!(fault_loop(kind([true, true, true], false)), Some(Loop::AB));
        assert_eq!(fault_loop(kind([false; 3], false)), None);
        assert!(is_ground(Loop::CN) && !is_ground(Loop::BC));
        let k = k0(c(1.0, 1.0), c(4.0, 4.0));
        assert!(close(k.re, 1.0, 1e-12) && close(k.im, 0.0, 1e-12));
    }
}
