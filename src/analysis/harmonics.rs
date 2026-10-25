//! Harmonic spectrum and total harmonic distortion (THD) of a channel over a
//! [`Window`] of whole cycles.

use std::f64::consts::FRAC_1_SQRT_2;

use super::Complex;
use super::phasor::{self, PhasorError, Window};
use crate::comtrade::{AnalogChannel, Side};

/// Spectrum of one channel over a window: DC, fundamental and harmonics.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    /// Mean value over the window.
    pub dc: f64,
    /// Fundamental phasor (peak value).
    pub fundamental: Complex,
    /// Phasors (peak values) of the orders 2, 3, … in that order.
    pub harmonics: Vec<Complex>,
}

impl Spectrum {
    /// Highest order in the spectrum.
    pub fn max_order(&self) -> usize {
        self.harmonics.len() + 1
    }

    /// Phasor of `order` (1 = fundamental), `None` outside the spectrum.
    pub fn phasor(&self, order: usize) -> Option<Complex> {
        match order {
            1 => Some(self.fundamental),
            h => self.harmonics.get(h.checked_sub(2)?).copied(),
        }
    }

    /// RMS of the fundamental.
    pub fn fundamental_rms(&self) -> f64 {
        self.fundamental.abs() * FRAC_1_SQRT_2
    }

    /// Magnitude of `order` in % of the fundamental, `None` without a
    /// fundamental or outside the spectrum.
    pub fn percent(&self, order: usize) -> Option<f64> {
        let f = self.fundamental.abs();
        (f > 0.0)
            .then(|| self.phasor(order).map(|p| p.abs() / f * 100.0))
            .flatten()
    }

    /// DC in % of the fundamental (peak), `None` without a fundamental.
    pub fn dc_percent(&self) -> Option<f64> {
        let f = self.fundamental.abs();
        (f > 0.0).then(|| self.dc.abs() / f * 100.0)
    }

    /// Angle of `order` in degrees relative to the fundamental, shifted by
    /// the order (φₕ − h·φ₁, in (-180, 180]), so the angles describe the
    /// waveform's shape wherever the window starts.
    pub fn angle(&self, order: usize) -> Option<f64> {
        let p = self.phasor(order)?;
        let a = (p.arg() - order as f64 * self.fundamental.arg()).to_degrees();
        let mut a = (a + 180.0).rem_euclid(360.0) - 180.0;
        if a <= -180.0 {
            a += 360.0;
        }
        Some(if a.abs() < 1e-9 { 0.0 } else { a })
    }

    /// Total harmonic distortion in %: RMS of all harmonics relative to the
    /// fundamental, `None` without a fundamental.
    pub fn thd(&self) -> Option<f64> {
        let f = self.fundamental.abs();
        let sum: f64 = self.harmonics.iter().map(|h| h.abs() * h.abs()).sum();
        (f > 0.0).then(|| sum.sqrt() / f * 100.0)
    }
}

/// Highest harmonic order below half the sample rate around the end of
/// `window` (the sample rate can't show higher ones), at least 1.
pub fn max_order(time: &[f64], window: &Window) -> usize {
    let i = window.end.max(1).min(time.len().saturating_sub(1));
    let dt = if i > 0 { time[i] - time[i - 1] } else { 0.0 };
    if dt <= 0.0 || window.frequency <= 0.0 {
        return 1;
    }
    // An order exactly at half the sample rate is not resolved either.
    let limit = 1.0 / dt / 2.0 / window.frequency;
    ((limit - 1e-9).ceil() as usize).saturating_sub(1).max(1)
}

/// Spectrum of `ch` over `window` up to `max_order`, with values on `side`.
pub fn spectrum(
    ch: &AnalogChannel,
    time: &[f64],
    window: &Window,
    max_order: usize,
    side: Side,
) -> Result<Spectrum, PhasorError> {
    let fundamental = phasor::phasor(ch, time, window, side)?;
    let harmonics = (2..=max_order)
        .map(|h| phasor::dft(ch, time, window, h, side))
        .collect::<Result<Vec<_>, _>>()?;
    let start = window.end + 1 - window.samples;
    let sum: f64 = (start..=window.end).filter_map(|i| ch.value_on(i, side)).sum();
    Ok(Spectrum {
        dc: sum / window.samples as f64,
        fundamental,
        harmonics,
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use super::*;
    use crate::analysis::phasor::tests::close;
    use crate::comtrade::Comtrade;

    /// 2 kHz, 100 ms of one channel: 10 + 100·cos(ωt) + 20·cos(3ωt + 30°)
    /// + 5·cos(5ωt) at 50 Hz.
    fn recording() -> Comtrade {
        let cfg = "S,D,1999\n1,1A,0D\n1,IA,A,LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n50\n1\n2000,200\n\
01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        for n in 0..200 {
            let t = n as f64 / 2000.0;
            let w = TAU * 50.0 * t;
            let v = 10.0 + 100.0 * w.cos() + 20.0 * (3.0 * w + 30f64.to_radians()).cos() + 5.0 * (5.0 * w).cos();
            dat += &format!("{},{},{}\n", n + 1, n * 500, (v * 1000.0).round());
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    #[test]
    fn spectrum_of_a_known_signal() {
        let c = recording();
        let w = Window::ending_at(&c, 0.060, 1.0, 50.0).unwrap();
        assert_eq!(w.samples, 40);
        // 40 samples per cycle: up to the 19th (20th is half the sample rate).
        assert_eq!(max_order(c.time(), &w), 19);
        let s = spectrum(c.analog(0).unwrap(), c.time(), &w, 19, Side::Primary).unwrap();
        assert_eq!(s.max_order(), 19);
        assert!(close(s.dc, 10.0, 0.01));
        assert!(close(s.fundamental_rms(), 100.0 / 2f64.sqrt(), 0.01));
        assert!(close(s.percent(3).unwrap(), 20.0, 0.01));
        assert!(close(s.percent(5).unwrap(), 5.0, 0.01));
        assert!(s.percent(2).unwrap() < 0.01 && s.percent(7).unwrap() < 0.01);
        assert!(close(s.angle(3).unwrap(), 30.0, 0.1));
        assert!(close(s.thd().unwrap(), (20f64.powi(2) + 5f64.powi(2)).sqrt(), 0.01));
        assert!(close(s.dc_percent().unwrap(), 10.0, 0.01));
        assert_eq!(s.percent(20), None);
    }

    #[test]
    fn angles_do_not_depend_on_the_window_start() {
        let c = recording();
        let a = Window::ending_at(&c, 0.060, 1.0, 50.0).unwrap();
        let b = Window::ending_at(&c, 0.0675, 1.0, 50.0).unwrap();
        let ch = c.analog(0).unwrap();
        let sa = spectrum(ch, c.time(), &a, 5, Side::Primary).unwrap();
        let sb = spectrum(ch, c.time(), &b, 5, Side::Primary).unwrap();
        assert!(close(sa.angle(3).unwrap(), sb.angle(3).unwrap(), 0.1));
    }
}
