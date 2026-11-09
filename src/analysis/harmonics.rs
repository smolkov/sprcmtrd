//! Harmonic spectrum and total harmonic distortion (THD) of a channel over a
//! [`Window`] of whole cycles.

use std::f64::consts::{FRAC_1_SQRT_2, TAU};

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

/// RMS of the `orders` (1 = fundamental) of `ch` at each of the samples
/// `points`, over the window of `seconds(i)` ending there, from a DFT at
/// `order / seconds(i)`: with `seconds` one cycle of the measured frequency,
/// the harmonics stay at whole multiples off nominal. Per point one value
/// per order; `None` where no whole window fits, it has fewer than 4
/// samples, or a value is missing. `time` holds the sample times.
///
/// Off nominal a cycle isn't a whole number of samples (55 Hz at 4 kHz:
/// 72.7). The window then covers exactly one cycle: the samples inside count
/// fully, the oldest one by the part of its slot that is inside.
pub fn sliding(
    ch: &AnalogChannel,
    time: &[f64],
    points: &[usize],
    seconds: impl Fn(usize) -> f64,
    orders: &[usize],
    side: Side,
) -> Vec<Vec<Option<f64>>> {
    let factor = ch.info.side_factor(side);
    points
        .iter()
        .map(|&end| {
            let none = vec![None; orders.len()];
            if end == 0 || end >= time.len() {
                return none;
            }
            let length = seconds(end);
            let dt = time[end] - time[end - 1];
            if dt <= 0.0 || length <= 0.0 {
                return none;
            }
            // The window holds `n` sample slots: whole ones and the part
            // `fraction` of the oldest.
            let n = length / dt;
            let whole = n.floor();
            let fraction = n - whole;
            let count = whole as usize + usize::from(fraction > 1e-9);
            if n < 4.0 || count > end + 1 {
                return none;
            }
            let start = end + 1 - count;
            let values: Option<Vec<f64>> = (start..=end).map(|k| ch.value(k).map(|v| v * factor)).collect();
            let Some(values) = values else {
                return none;
            };
            let weight = |k: usize| if k == 0 && fraction > 1e-9 { fraction } else { 1.0 };
            let frequency = 1.0 / length;
            orders
                .iter()
                .map(|&h| {
                    let omega = TAU * frequency * h as f64;
                    let mut sum = Complex::default();
                    for (k, (v, &t)) in values.iter().zip(&time[start..=end]).enumerate() {
                        sum = sum + Complex::from_polar(v * weight(k), -omega * t);
                    }
                    Some(sum.scale(2.0 / n).abs() * FRAC_1_SQRT_2)
                })
                .collect()
        })
        .collect()
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

    /// 4 kHz, 200 ms of one channel at `f` Hz: 100·cos(ωt) + 20·cos(3ωt)
    /// + 5·cos(5ωt + 40°); sample 300 has no value.
    fn off_nominal(f: f64) -> Comtrade {
        let cfg = "S,D,1999\n1,1A,0D\n1,IA,A,LINE,A,0.001,0,0,-1e9,1e9,1,1,P\n50\n1\n4000,800\n\
01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let mut dat = String::new();
        for n in 0..800 {
            let t = n as f64 / 4000.0;
            let w = TAU * f * t;
            let v = 100.0 * w.cos() + 20.0 * (3.0 * w).cos() + 5.0 * (5.0 * w + 40f64.to_radians()).cos();
            let v = if n == 300 {
                String::new()
            } else {
                format!("{}", (v * 1000.0).round())
            };
            dat += &format!("{},{},{v}\n", n + 1, n * 250);
        }
        Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    #[test]
    fn sliding_harmonics_follow_the_frequency() {
        let rms = |peak: f64| peak / 2f64.sqrt();
        for f in [50.0, 55.0] {
            let c = off_nominal(f);
            let ch = c.analog(0).unwrap();
            let values = sliding(ch, c.time(), &[200, 700], |_| 1.0 / f, &[1, 2, 3, 5], Side::Primary);
            for row in &values {
                let v: Vec<f64> = row.iter().map(|x| x.unwrap()).collect();
                // Off nominal a cycle isn't a whole number of samples: the
                // 5th (14.5 samples a cycle at 55 Hz) is about 1 % low.
                let near = |x: f64, want: f64| close(x, want, want * 0.015);
                assert!(near(v[0], rms(100.0)), "{f} {v:?}");
                assert!(v[1] < 0.05, "{f} {v:?}");
                assert!(near(v[2], rms(20.0)), "{f} {v:?}");
                assert!(near(v[3], rms(5.0)), "{f} {v:?}");
            }
        }
        // 55 Hz with 50 Hz windows: the fundamental leaks into the 2nd.
        let c = off_nominal(55.0);
        let v = sliding(c.analog(0).unwrap(), c.time(), &[200], |_| 0.02, &[2], Side::Primary);
        assert!(v[0][0].unwrap() > 5.0, "{v:?}");
    }

    #[test]
    fn sliding_harmonics_need_a_whole_window_with_values() {
        let c = off_nominal(50.0);
        let ch = c.analog(0).unwrap();
        // 80 samples a cycle: the first whole window ends at sample 79; the
        // windows ending at 300 … 379 hold the missing sample.
        let v = sliding(ch, c.time(), &[10, 79, 320, 380], |_| 0.02, &[1], Side::Primary);
        assert_eq!(v[0], [None]);
        assert!(v[1][0].is_some());
        assert_eq!(v[2], [None]);
        assert!(v[3][0].is_some());
    }
}
