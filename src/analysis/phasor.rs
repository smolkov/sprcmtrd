//! Phasors of the fundamental frequency.
//!
//! A phasor is computed with a DFT over a [`Window`] of whole cycles that
//! ends at a sample, as a protection relay sees the signal at that moment.
//! The DFT uses the sample times, so the angle is right even when a cycle is
//! not a whole number of samples. [`Polar`] shows it as RMS and angle.

use std::f64::consts::{FRAC_1_SQRT_2, PI};

use serde::Serialize;

use super::Complex;
use crate::comtrade::{AnalogChannel, Comtrade, Side, round12};

/// Samples a phasor is computed over: `samples` samples ending at sample
/// `end` (inclusive), for the fundamental `frequency` in Hz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// Last sample (0-based).
    pub end: usize,
    /// Number of samples.
    pub samples: usize,
    /// Fundamental frequency in Hz.
    pub frequency: f64,
}

/// Why no [`Window`] fits.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum WindowError {
    /// The recording has no samples.
    #[error("the recording has no samples")]
    NoSamples,
    /// The frequency or the number of cycles is not above 0.
    #[error("frequency and cycles must be above 0")]
    NotPositive,
    /// Fewer than 4 samples per window: the sample rate is too low.
    #[error("the window has only {0} samples")]
    TooShort(usize),
    /// The window would start before the first sample; `earliest` is the
    /// first time (s, relative to the trigger) a window can end at.
    #[error("the window of {samples} samples starts before the recording")]
    BeforeStart {
        /// Samples in the window.
        samples: usize,
        /// Earliest window end in s relative to the trigger.
        earliest: f64,
    },
}

impl Window {
    /// The window of `cycles` cycles of `frequency` ending at the sample
    /// nearest `t` (s relative to the trigger). Its length in samples comes
    /// from the sample spacing there.
    pub fn ending_at(c: &Comtrade, t: f64, cycles: f64, frequency: f64) -> Result<Self, WindowError> {
        if frequency <= 0.0 || cycles <= 0.0 {
            return Err(WindowError::NotPositive);
        }
        let time = c.time();
        let end = c.index_at(t).ok_or(WindowError::NoSamples)?;
        let i = end.max(1).min(time.len() - 1);
        let dt = if i > 0 { time[i] - time[i - 1] } else { 0.0 };
        let samples = if dt > 0.0 {
            (cycles / frequency / dt).round() as usize
        } else {
            0
        };
        if samples < 4 {
            return Err(WindowError::TooShort(samples));
        }
        if end + 1 < samples {
            let earliest = time.get(samples - 1).copied().map_or(f64::NAN, round12);
            return Err(WindowError::BeforeStart { samples, earliest });
        }
        Ok(Self {
            end,
            samples,
            frequency,
        })
    }

    /// First sample of the window.
    pub fn start(&self) -> usize {
        self.end + 1 - self.samples
    }
}

/// Why a channel's phasor could not be computed.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PhasorError {
    /// The window would start before the first sample.
    #[error("the window starts before the recording")]
    BeforeStart,
    /// A sample in the window has no value.
    #[error("missing value at sample {0}")]
    Missing(usize),
}

/// Fundamental phasor (peak value) of `ch` over `window`, with values on
/// `side`. `time` holds the sample times of the recording.
pub fn phasor(ch: &AnalogChannel, time: &[f64], window: &Window, side: Side) -> Result<Complex, PhasorError> {
    dft(ch, time, window, 1, side)
}

/// Phasor (peak value) of the `order`-th harmonic of `ch` over `window`:
/// the DFT at `order` times the window's frequency. Order 1 is
/// [`phasor`]; harmonics are exact over whole cycles.
pub fn dft(
    ch: &AnalogChannel,
    time: &[f64],
    window: &Window,
    order: usize,
    side: Side,
) -> Result<Complex, PhasorError> {
    let start = (window.end + 1)
        .checked_sub(window.samples)
        .ok_or(PhasorError::BeforeStart)?;
    let omega = 2.0 * PI * window.frequency * order as f64;
    let mut sum = Complex::default();
    for (i, &t) in time.iter().enumerate().take(window.end + 1).skip(start) {
        let v = ch.value_on(i, side).ok_or(PhasorError::Missing(i + 1))?;
        // x(t) = A·cos(ωt + φ) gives A·e^{jφ}.
        sum = sum + Complex::from_polar(v, -omega * t);
    }
    Ok(sum.scale(2.0 / window.samples as f64))
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
        // Into (-180, 180].
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

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A 1 kHz recording (trigger at 20 ms) of 50 Hz cosines with the given
    /// peak values and angles (degrees) on phases A, B, C of circuit LINE.
    pub fn recording(signals: [(f64, f64); 3]) -> Comtrade {
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

    pub fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn phasor_of_a_cosine() {
        let c = recording([(100.0, 30.0), (100.0, -90.0), (100.0, 150.0)]);
        // 40 ms after the start = 20 ms after the trigger.
        let w = Window::ending_at(&c, 0.020, 1.0, 50.0).unwrap();
        assert_eq!((w.end, w.samples, w.start()), (40, 20, 21));
        let a = phasor(c.analog(0).unwrap(), c.time(), &w, Side::Primary).unwrap();
        // Times are relative to the trigger (20 ms = one cycle), so the angle
        // is the one of the generated cosine.
        let p = Polar::new(a, 0.0);
        assert!(close(p.magnitude, 100.0 / 2f64.sqrt(), 0.01), "{p:?}");
        assert!(close(p.angle, 30.0, 0.05), "{p:?}");
        // Relative to phase A.
        let b = phasor(c.analog(1).unwrap(), c.time(), &w, Side::Primary).unwrap();
        assert!(close(Polar::new(b, a.arg()).angle, -120.0, 0.05));
    }

    #[test]
    fn windows_that_do_not_fit() {
        let c = recording([(1.0, 0.0); 3]);
        assert_eq!(
            Window::ending_at(&c, -0.010, 1.0, 50.0),
            Err(WindowError::BeforeStart {
                samples: 20,
                earliest: -0.001
            })
        );
        assert_eq!(Window::ending_at(&c, 0.0, 0.1, 50.0), Err(WindowError::TooShort(2)));
        assert_eq!(Window::ending_at(&c, 0.0, 1.0, 0.0), Err(WindowError::NotPositive));
        // A hand-made window that starts too early.
        let w = Window {
            end: 5,
            samples: 20,
            frequency: 50.0,
        };
        assert_eq!(
            phasor(c.analog(0).unwrap(), c.time(), &w, Side::Primary),
            Err(PhasorError::BeforeStart)
        );
    }

    #[test]
    fn polar_angles() {
        let p = |deg: f64| Complex::from_polar(1.0, deg.to_radians());
        assert_eq!(Polar::new(p(-179.0), 2f64.to_radians()).angle, 179.0);
        assert_eq!(Polar::new(p(180.0), 0.0).angle, 180.0);
    }
}
