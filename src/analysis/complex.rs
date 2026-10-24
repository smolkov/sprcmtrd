//! Complex numbers, as far as phasors need them.

use std::ops::{Add, Mul, Sub};

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

    /// Multiplied by the real number `k`.
    pub fn scale(self, k: f64) -> Self {
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
