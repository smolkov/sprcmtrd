//! Signal analysis of loaded recordings: statistics, binary events, phasors
//! of the fundamental, symmetrical components, the system frequency, the
//! sliding RMS, the fault summary, the DC offset of the fault current and the
//! fault location.
//!
//! Builds on [`crate::comtrade`] (channels, sample times) and is used by the
//! commands; it does not depend on the command line or the TUI. Results are
//! rounded to 12 significant digits where float noise would show.

pub mod compare;
pub mod complex;
pub mod dc_offset;
pub mod events;
pub mod fault;
pub mod frequency;
pub mod harmonics;
pub mod impedance;
pub mod locate;
pub mod phasor;
pub mod power;
pub mod rms;
pub mod sequence;
pub mod stats;
pub mod trend;

pub use complex::Complex;
pub use events::Event;
pub use phasor::{Polar, Window};
pub use sequence::Sequence;
pub use stats::{AnalogStats, BinaryStats};
