//! Apparent impedance of the six fault loops, as a distance relay measures
//! it: phase to ground with residual compensation, and phase to phase.

use super::Complex;
use super::phasor::{self, Window};
use super::sequence::{self, Group};
use crate::comtrade::{AnalogChannel, AnalogInfo, Comtrade, Side};

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

/// `z` with a part below 1e-6 of |Z| set to 0: residue of integer samples,
/// which would show as `-31.65µ` next to 199.6 Ω (and as an angle of -0.0°).
pub fn clean(z: Complex) -> Complex {
    let small = |v: f64| if v.abs() <= 1e-6 * z.abs() { 0.0 } else { v };
    Complex {
        re: small(z.re),
        im: small(z.im),
    }
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
    let voltage = find_group(channels, Quantity::Voltage, None)?;
    let current = find_group(channels, Quantity::Current, Some(&voltage.circuit))?;
    Some(Pair { voltage, current })
}

/// The first current group among `channels`.
pub fn find_current(channels: &[&AnalogInfo]) -> Option<Group> {
    find_group(channels, Quantity::Current, None)
}

/// The first voltage group among `channels`, preferring one of `circuit`.
pub fn find_voltage(channels: &[&AnalogInfo], circuit: &str) -> Option<Group> {
    find_group(channels, Quantity::Voltage, Some(circuit))
}

/// The six channels of a [`Pair`] (voltages A, B, C, then currents A, B, C)
/// and the factors from their units to V and A.
#[derive(Debug, Clone, Copy)]
pub struct Loops<'a> {
    /// Voltages A, B, C, then currents A, B, C.
    pub channels: [&'a AnalogChannel; 6],
    /// Factor from the voltage unit to V.
    pub voltage_factor: f64,
    /// Factor from the current unit to A.
    pub current_factor: f64,
}

impl<'a> Loops<'a> {
    /// The channels of `pair` in `c`, `None` when the voltage group's unit is
    /// not a voltage or the current group's not a current.
    pub fn new(c: &'a Comtrade, pair: &Pair) -> Option<Self> {
        let factor = |g: &Group, q: Quantity| unit_factor(&g.unit).filter(|&(k, _)| k == q).map(|(_, f)| f);
        let ch = |i: usize| c.analog(i);
        let [va, vb, vc] = pair.voltage.phases.map(ch);
        let [ia, ib, ic] = pair.current.phases.map(ch);
        Some(Self {
            channels: [va?, vb?, vc?, ia?, ib?, ic?],
            voltage_factor: factor(&pair.voltage, Quantity::Voltage)?,
            current_factor: factor(&pair.current, Quantity::Current)?,
        })
    }

    /// Channel ids of the voltages and of the currents.
    pub fn ids(&self) -> ([String; 3], [String; 3]) {
        let id = |k: usize| self.channels[k].info.id.clone();
        ([0, 1, 2].map(id), [3, 4, 5].map(id))
    }

    /// The phasors of the six channels (in their order) in V and A; `None`
    /// if one is missing.
    pub fn scaled(&self, phasors: &[Option<Complex>]) -> Option<([Complex; 3], [Complex; 3])> {
        let get = |k: usize, f: f64| phasors.get(k).copied().flatten().map(|p| p.scale(f));
        let v = [
            get(0, self.voltage_factor)?,
            get(1, self.voltage_factor)?,
            get(2, self.voltage_factor)?,
        ];
        let i = [
            get(3, self.current_factor)?,
            get(4, self.current_factor)?,
            get(5, self.current_factor)?,
        ];
        Some((v, i))
    }
}

/// Impedance (Ω) and loop current (A) of one loop at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopPoint {
    /// Apparent impedance V / I.
    pub z: Complex,
    /// Loop current (peak phasor).
    pub current: Complex,
}

/// The six loops ([`Loop::ALL`] order) at each of the samples `points`,
/// from the phasors over the window of `samples` samples ending there at
/// `frequency`, as `impedance --trend` measures them. `None` where the window
/// doesn't fit, a value is missing or the loop has no current.
pub fn trajectory(
    loops: &Loops,
    time: &[f64],
    points: &[usize],
    samples: usize,
    frequency: f64,
    k0: Complex,
    side: Side,
) -> Vec<[Option<LoopPoint>; 6]> {
    points
        .iter()
        .map(|&end| {
            let window = Window {
                end,
                samples,
                frequency,
            };
            let phasors: Vec<Option<Complex>> = loops
                .channels
                .iter()
                .map(|ch| phasor::phasor(ch, time, &window, side).ok())
                .collect();
            let scaled = (end + 1 >= samples && end < time.len())
                .then(|| loops.scaled(&phasors))
                .flatten();
            Loop::ALL.map(|l| {
                let (v, i) = scaled?;
                let (lv, li) = loop_quantities(l, v, i, k0);
                Some(LoopPoint {
                    z: clean(impedance(lv, li)?),
                    current: li,
                })
            })
        })
        .collect()
}

/// The first group of quantity `q`, preferring one of `circuit` when given.
fn find_group(channels: &[&AnalogInfo], q: Quantity, circuit: Option<&str>) -> Option<Group> {
    let groups = sequence::groups(channels);
    let mut of = groups
        .into_iter()
        .filter(|g| unit_factor(&g.unit).is_some_and(|(k, _)| k == q));
    let first = of.next()?;
    match circuit {
        Some(circuit) if first.circuit != circuit => Some(of.find(|g| g.circuit == circuit).unwrap_or(first)),
        _ => Some(first),
    }
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

    /// 1 kHz, 100 ms at 50 Hz: currents IA, IB, IC of 100 A, from 60 ms on
    /// 0 (breaker open); voltages V = Z · I with Z = 3 + j4 Ω in each phase.
    fn loop_recording() -> crate::comtrade::Comtrade {
        let mut cfg = String::from("S,D,1999\n6,6A,0D\n");
        for (i, (id, unit)) in ["VA", "VB", "VC", "IA", "IB", "IC"]
            .iter()
            .zip(["V", "V", "V", "A", "A", "A"])
            .enumerate()
        {
            cfg += &format!("{},{id},{},LINE,{unit},0.001,0,0,-1e9,1e9,1,1,P\n", i + 1, &id[1..]);
        }
        cfg += "50\n1\n1000,100\n01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.000000\nASCII\n1\n";
        let z = Complex { re: 3.0, im: 4.0 };
        let mut dat = String::new();
        for k in 0..100 {
            let t = k as f64 / 1000.0;
            let on = if k < 60 { 1.0 } else { 0.0 };
            dat += &format!("{},{}", k + 1, k * 1000);
            let phase = |p: usize| 2.0 * std::f64::consts::PI * (50.0 * t - p as f64 / 3.0);
            for p in 0..3 {
                let v = (Complex::from_polar(100.0 * on, phase(p)) * z).re;
                dat += &format!(",{}", (v * 1000.0).round());
            }
            for p in 0..3 {
                dat += &format!(",{}", (100.0 * on * phase(p).cos() * 1000.0).round());
            }
            dat += "\n";
        }
        crate::comtrade::Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap()
    }

    #[test]
    fn trajectory_of_known_loops() {
        let c = loop_recording();
        let infos: Vec<&AnalogInfo> = c.analog_channels().map(|ch| &ch.info).collect();
        let pair = find_pair(&infos).unwrap();
        let loops = Loops::new(&c, &pair).unwrap();
        assert_eq!(loops.ids().0, ["VA", "VB", "VC"]);
        let t = trajectory(
            &loops,
            c.time(),
            &[10, 19, 40, 99],
            20,
            50.0,
            Complex::default(),
            Side::Primary,
        );
        // Before a whole window: nothing.
        assert!(t[0].iter().all(Option::is_none));
        // Every loop sees 3 + j4 Ω (balanced: phase to phase too).
        for k in [1, 2] {
            for p in t[k] {
                let z = p.unwrap().z;
                assert!(close(z.re, 3.0, 1e-3) && close(z.im, 4.0, 1e-3), "{z:?}");
            }
        }
        assert!(close(t[2][0].unwrap().current.abs(), 100.0, 1e-3));
        // Breaker open: no current, no impedance.
        assert!(t[3].iter().all(Option::is_none));
    }
}
