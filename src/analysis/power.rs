//! Active, reactive and apparent power of a three-phase circuit from the
//! fundamental phasors of its phase-to-neutral voltages and currents.
//!
//! Sign convention: positive active power flows in the direction the
//! currents are measured (usually from the busbar into the line), positive
//! reactive power is inductive (current lagging the voltage).

use super::Complex;

/// Complex power S = P + jQ in VA of one phase from the peak phasors of its
/// voltage (V) and current (A): S = V·I* with RMS values, so half the
/// product of the peaks.
pub fn power(v: Complex, i: Complex) -> Complex {
    clean((v * i.conj()).scale(0.5))
}

/// Power of the three phases and of the circuit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreePhase {
    /// Complex power of phases A, B and C in VA.
    pub phases: [Complex; 3],
    /// Sum of the phases.
    pub total: Complex,
}

/// Power per phase and in total from the peak phasors of the voltages and
/// currents of phases A, B, C (in V and A).
pub fn three_phase(v: [Complex; 3], i: [Complex; 3]) -> ThreePhase {
    let phases = [0, 1, 2].map(|k| power(v[k], i[k]));
    let total = clean(phases[0] + phases[1] + phases[2]);
    ThreePhase { phases, total }
}

/// Power factor P / |S|, negative for reverse active power; `None` without
/// apparent power (no current or no voltage).
pub fn power_factor(s: Complex) -> Option<f64> {
    let a = s.abs();
    (a > 0.0).then(|| s.re / a)
}

/// `s` with a part below 1e-6 of |S| set to 0: residue of integer samples,
/// which would show as a reactive power of a few µvar next to megawatts.
fn clean(s: Complex) -> Complex {
    let small = |v: f64| if v.abs() <= 1e-6 * s.abs() { 0.0 } else { v };
    Complex {
        re: small(s.re),
        im: small(s.im),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Equal to 1e-9 of `b` (powers reach megawatts).
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * b.abs().max(1.0)
    }

    /// Peak phasor of an RMS value and an angle in degrees.
    fn p(rms: f64, deg: f64) -> Complex {
        Complex::from_polar(rms * 2f64.sqrt(), deg.to_radians())
    }

    fn balanced(v: f64, i: f64, lag: f64) -> ([Complex; 3], [Complex; 3]) {
        (
            [0.0, -120.0, 120.0].map(|a| p(v, a)),
            [0.0, -120.0, 120.0].map(|a| p(i, a - lag)),
        )
    }

    #[test]
    fn balanced_resistive_load() {
        let (v, i) = balanced(63.5e3, 318.0, 0.0);
        let s = three_phase(v, i);
        for ph in s.phases {
            assert!(close(ph.re, 63.5e3 * 318.0));
            assert_eq!(ph.im, 0.0);
        }
        assert!(close(s.total.re, 3.0 * 63.5e3 * 318.0));
        assert_eq!(s.total.im, 0.0);
        assert!(close(power_factor(s.total).unwrap(), 1.0));
    }

    #[test]
    fn lagging_load_and_reverse_flow() {
        // Current 30° behind the voltage: inductive, Q > 0.
        let (v, i) = balanced(100.0, 10.0, 30.0);
        let s = three_phase(v, i).total;
        assert!(close(s.re, 3000.0 * 30f64.to_radians().cos()));
        assert!(close(s.im, 3000.0 * 0.5));
        assert!(close(power_factor(s).unwrap(), 30f64.to_radians().cos()));

        // Reversed currents: power flows the other way.
        let (v, i) = balanced(100.0, 10.0, 180.0);
        let s = three_phase(v, i).total;
        assert!(close(s.re, -3000.0));
        assert_eq!(s.im, 0.0);
        assert!(close(power_factor(s).unwrap(), -1.0));
    }

    #[test]
    fn no_current_has_no_power_factor() {
        let (v, _) = balanced(100.0, 0.0, 0.0);
        let s = three_phase(v, [Complex::default(); 3]).total;
        assert_eq!(s, Complex::default());
        assert_eq!(power_factor(s), None);
    }
}
