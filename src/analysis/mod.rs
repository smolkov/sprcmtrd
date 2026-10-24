//! Signal analysis of loaded recordings: statistics, binary events, phasors
//! of the fundamental and symmetrical components.
//!
//! Builds on [`crate::comtrade`] (channels, sample times) and is used by the
//! commands; it does not depend on the command line or the TUI. Results are
//! rounded to 12 significant digits where float noise would show.

pub mod complex;
pub mod events;
pub mod phasor;
pub mod sequence;
pub mod stats;

pub use complex::Complex;
pub use events::Event;
pub use phasor::{Polar, Window};
pub use sequence::Sequence;
pub use stats::{AnalogStats, BinaryStats};
