//! Decaying DC offset of a fault current.
//!
//! After inception the current of an R-L circuit is a sine plus a decaying DC
//! term, i(t) = a·sin(ωt) + b·cos(ωt) + D·e^(−t/τ), t from inception. For a
//! fixed τ the model is linear in a, b and D, so τ is found by a search in one
//! dimension (a grid, then golden section refinement), solving the 3×3 least
//! squares problem for each τ. τ = L/R, so X/R = ω·τ.

use std::f64::consts::TAU as TWO_PI;
use std::ops::Range;

use super::fault::Fault;
use crate::comtrade::{AnalogChannel, Side};

/// τ is given only for a DC part of at least this part of the AC peak: the τ
/// of a tiny DC term is noise.
pub const SIGNIFICANT_PART: f64 = 0.05;
/// Shortest τ searched, in s.
pub const TAU_MIN: f64 = 1e-3;
/// Longest τ searched, in s.
pub const TAU_MAX: f64 = 1.0;
/// Points of the τ grid, spaced logarithmically.
const GRID: usize = 64;
/// Golden section steps after the grid.
const REFINE: usize = 60;
/// Fewest samples a fit needs.
const MIN_SAMPLES: usize = 6;

/// The decay time constant of a fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TimeConstant {
    /// The DC part is below [`SIGNIFICANT_PART`] of the AC peak.
    Insignificant,
    /// τ in s.
    Fitted(f64),
    /// At the [`TAU_MAX`] limit: the DC part hardly decays within the window.
    Longer,
}

impl TimeConstant {
    /// τ in s, when it was fitted.
    pub fn seconds(self) -> Option<f64> {
        match self {
            Self::Fitted(t) => Some(t),
            _ => None,
        }
    }
}

/// Result of [`fit`], in the units of the samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decay {
    /// DC part at inception (t = 0), with its sign.
    pub dc: f64,
    /// Decay time constant.
    pub tau: TimeConstant,
    /// Peak of the AC part.
    pub ac_peak: f64,
    /// RMS of the fit residual as a part of the AC peak.
    pub fit_error: f64,
}

impl Decay {
    /// DC part as a part of the AC peak.
    pub fn dc_part(&self) -> f64 {
        self.dc / self.ac_peak
    }

    /// X/R = ω·τ, when τ was fitted.
    pub fn x_over_r(&self, frequency: f64) -> Option<f64> {
        self.tau.seconds().map(|t| TWO_PI * frequency * t)
    }
}

/// Solves `m·x = r` by Gaussian elimination with partial pivoting.
fn solve(mut m: [[f64; 3]; 3], mut r: [f64; 3]) -> Option<[f64; 3]> {
    let scale = m.iter().flatten().fold(0.0f64, |a, v| a.max(v.abs()));
    for col in 0..3 {
        let pivot = (col..3).max_by(|&a, &b| m[a][col].abs().total_cmp(&m[b][col].abs()))?;
        if m[pivot][col].abs() <= 1e-12 * scale {
            return None;
        }
        m.swap(col, pivot);
        r.swap(col, pivot);
        let pivot_row = m[col];
        for row in col + 1..3 {
            let k = m[row][col] / pivot_row[col];
            for (x, p) in m[row].iter_mut().zip(pivot_row).skip(col) {
                *x -= k * p;
            }
            r[row] -= k * r[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let sum: f64 = (row + 1..3).map(|c| m[row][c] * x[c]).sum();
        x[row] = (r[row] - sum) / m[row][row];
    }
    Some(x)
}

/// Least squares a, b, D for a fixed `tau` and the sum of squared residuals.
fn least_squares(samples: &[(f64, f64)], omega: f64, tau: f64) -> Option<([f64; 3], f64)> {
    let row = |t: f64| [(omega * t).sin(), (omega * t).cos(), (-t / tau).exp()];
    let mut m = [[0.0; 3]; 3];
    let mut r = [0.0; 3];
    for &(t, y) in samples {
        let x = row(t);
        for i in 0..3 {
            for j in 0..3 {
                m[i][j] += x[i] * x[j];
            }
            r[i] += x[i] * y;
        }
    }
    let c = solve(m, r)?;
    let error: f64 = samples
        .iter()
        .map(|&(t, y)| {
            let x = row(t);
            let e = c[0] * x[0] + c[1] * x[1] + c[2] * x[2] - y;
            e * e
        })
        .sum();
    Some((c, error))
}

/// Fits the model to `samples` (time in s from inception, value) at the
/// fundamental `frequency`. `None` with fewer than 6 samples, no AC part or a
/// singular problem.
pub fn fit(samples: &[(f64, f64)], frequency: f64) -> Option<Decay> {
    if samples.len() < MIN_SAMPLES || frequency <= 0.0 {
        return None;
    }
    let omega = TWO_PI * frequency;
    let error_at = |log_tau: f64| least_squares(samples, omega, log_tau.exp()).map_or(f64::INFINITY, |(_, e)| e);
    let (lo, hi) = (TAU_MIN.ln(), TAU_MAX.ln());
    let step = (hi - lo) / (GRID - 1) as f64;
    let best = (0..GRID)
        .map(|k| (k, error_at(lo + step * k as f64)))
        .min_by(|a, b| a.1.total_cmp(&b.1))?
        .0;
    // Golden section between the grid neighbours of the best point.
    let (mut a, mut b) = (
        lo + step * best.saturating_sub(1) as f64,
        lo + step * (best + 1).min(GRID - 1) as f64,
    );
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let (mut c, mut d) = (b - g * (b - a), a + g * (b - a));
    let (mut ec, mut ed) = (error_at(c), error_at(d));
    for _ in 0..REFINE {
        if ec < ed {
            (b, d, ed) = (d, c, ec);
            c = b - g * (b - a);
            ec = error_at(c);
        } else {
            (a, c, ec) = (c, d, ed);
            d = a + g * (b - a);
            ed = error_at(d);
        }
    }
    let log_tau = (a + b) / 2.0;
    // The ends of the range are candidates too: golden section stays inside.
    let log_tau = [lo, log_tau, hi]
        .into_iter()
        .min_by(|x, y| error_at(*x).total_cmp(&error_at(*y)))?;
    let tau = log_tau.exp();
    let (coef, error) = least_squares(samples, omega, tau)?;
    let ac_peak = coef[0].hypot(coef[1]);
    if ac_peak <= 0.0 || !ac_peak.is_finite() {
        return None;
    }
    let dc = coef[2];
    let tau = if dc.abs() < SIGNIFICANT_PART * ac_peak {
        TimeConstant::Insignificant
    } else if tau >= TAU_MAX * 0.99 {
        TimeConstant::Longer
    } else {
        TimeConstant::Fitted(tau)
    };
    Some(Decay {
        dc,
        tau,
        ac_peak,
        fit_error: (error / samples.len() as f64).sqrt() / ac_peak,
    })
}

/// Result of [`analyse`].
#[derive(Debug, Clone, PartialEq)]
pub struct DcOffset {
    /// Samples of the fit window (0-based, end exclusive).
    pub window: Range<usize>,
    /// Fit of phases A, B, C; `None` when the window is shorter than a cycle
    /// or the fit fails.
    pub phases: [Option<Decay>; 3],
}

/// Fits the `currents` (phases A, B, C) from the inception of `fault` over
/// `cycles` cycles, cut at the interruption or at `end` (exclusive, the end of
/// the search range). Missing values are left out.
pub fn analyse(
    currents: [&AnalogChannel; 3],
    time: &[f64],
    fault: &Fault,
    end: usize,
    cycles: f64,
    frequency: f64,
    side: Side,
) -> DcOffset {
    let start = fault.inception;
    let length = (cycles * fault.per_cycle as f64).round() as usize;
    let stop = (start + length)
        .min(fault.interruption.unwrap_or(end))
        .min(end)
        .min(time.len())
        .max(start);
    let window = start..stop;
    let long_enough = window.len() >= fault.per_cycle;
    let t0 = time[start];
    let phases = currents.map(|ch| {
        if !long_enough {
            return None;
        }
        let samples: Vec<(f64, f64)> = window
            .clone()
            .filter_map(|i| ch.value_on(i, side).map(|v| (time[i] - t0, v)))
            .collect();
        fit(&samples, frequency)
    });
    DcOffset { window, phases }
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: f64 = 50.0;

    /// `cycles` cycles at `rate` samples per cycle of
    /// ac·sin(ωt + phase) + dc·e^(−t/tau).
    fn signal(ac: f64, phase: f64, dc: f64, tau: f64, cycles: usize, rate: usize) -> Vec<(f64, f64)> {
        let dt = 1.0 / F / rate as f64;
        (0..cycles * rate)
            .map(|n| {
                let t = n as f64 * dt;
                (t, ac * (TWO_PI * F * t + phase).sin() + dc * (-t / tau).exp())
            })
            .collect()
    }

    fn close(a: f64, b: f64, part: f64) -> bool {
        (a - b).abs() <= part * b.abs()
    }

    #[test]
    fn known_decay_is_found() {
        for (phase, dc, tau) in [(0.0, 3000.0, 0.040), (1.2, -2500.0, 0.030), (-2.0, 800.0, 0.005)] {
            let d = fit(&signal(5000.0, phase, dc, tau, 3, 20), F).unwrap();
            assert!(close(d.dc, dc, 0.01), "{d:?}");
            let fitted = d.tau.seconds().unwrap();
            assert!(close(fitted, tau, 0.01), "{d:?}");
            assert!(close(d.ac_peak, 5000.0, 0.01), "{d:?}");
            assert!(d.fit_error < 1e-6, "{d:?}");
            assert!(close(d.x_over_r(F).unwrap(), TWO_PI * F * tau, 0.01));
        }
    }

    #[test]
    fn noise_and_missing_samples() {
        let mut s = signal(4200.0, 0.4, 2500.0, 0.030, 3, 40);
        // Deterministic noise of ±20 A and a missing sample.
        for (n, (_, y)) in s.iter_mut().enumerate() {
            *y += 20.0 * ((n * 7919) % 13) as f64 / 6.0 - 20.0;
        }
        s.remove(17);
        let d = fit(&s, F).unwrap();
        assert!(close(d.dc, 2500.0, 0.02), "{d:?}");
        assert!(close(d.tau.seconds().unwrap(), 0.030, 0.03), "{d:?}");
        assert!(d.fit_error > 0.0 && d.fit_error < 0.01, "{d:?}");
    }

    #[test]
    fn no_or_slow_decay() {
        let pure = fit(&signal(1000.0, 0.3, 0.0, 0.03, 3, 20), F).unwrap();
        assert_eq!(pure.tau, TimeConstant::Insignificant);
        assert!(pure.dc.abs() < 1e-6);
        // A DC part that stays: τ at the limit.
        let constant = fit(&signal(1000.0, 0.0, 400.0, 1e6, 3, 20), F).unwrap();
        assert_eq!(constant.tau, TimeConstant::Longer);
        // τ stops at 1 s, which decays 6 % over the window: D comes out higher.
        assert!(close(constant.dc, 400.0, 0.05), "{constant:?}");
        assert!(constant.x_over_r(F).is_none());
    }

    #[test]
    fn too_few_samples_or_no_current() {
        assert!(fit(&signal(1000.0, 0.0, 500.0, 0.03, 1, 20)[..5], F).is_none());
        assert!(fit(&signal(0.0, 0.0, 0.0, 0.03, 3, 20), F).is_none());
        assert!(fit(&signal(1000.0, 0.0, 500.0, 0.03, 3, 20), 0.0).is_none());
    }

    #[test]
    fn solves_a_small_system() {
        let x = solve([[0.0, 2.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 4.0]], [4.0, 3.0, 8.0]).unwrap();
        assert_eq!(x, [3.0, 2.0, 2.0]);
        assert!(solve([[1.0, 2.0, 0.0], [2.0, 4.0, 0.0], [0.0, 0.0, 1.0]], [1.0, 2.0, 3.0]).is_none());
    }
}
