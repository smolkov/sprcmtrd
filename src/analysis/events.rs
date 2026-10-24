//! State changes of binary channels as a timeline.

use std::ops::Range;

use serde::Serialize;

use crate::comtrade::{BinaryChannel, round12};

/// A state change of a binary channel.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Event {
    /// 1-based number of the first sample with the new state.
    pub sample: usize,
    /// Time of that sample in s relative to the trigger.
    pub time: f64,
    /// Channel index `Dn`.
    pub index: usize,
    /// Channel identifier.
    pub id: String,
    /// State before the change.
    pub from: bool,
    /// State after the change.
    pub to: bool,
}

/// State changes of `channels` within the samples in `range`, ordered by
/// time and then by the order of `channels`. A change is reported at the
/// first sample with the new state; the first sample of the range has no
/// previous state and is never a change. `time` holds the sample times of
/// the recording.
pub fn events(channels: &[&BinaryChannel], time: &[f64], range: Range<usize>) -> Vec<Event> {
    let end = range.end.min(time.len());
    let range = range.start.min(end)..end;
    let mut out = Vec::new();
    for (i, &t) in time.iter().enumerate().take(range.end).skip(range.start + 1) {
        for ch in channels {
            let (from, to) = (ch.get(i - 1), ch.get(i));
            if from != to {
                out.push(Event {
                    sample: i + 1,
                    time: round12(t),
                    index: ch.info.index,
                    id: ch.info.id.clone(),
                    from,
                    to,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comtrade::Comtrade;

    #[test]
    fn changes_in_time_and_channel_order() {
        let cfg = "S,D,1999\n2,0A,2D\n1,A,,,0\n2,B,,,0\n50\n1\n1000,5\n\
01/01/2020,00:00:00.000000\n01/01/2020,00:00:00.002000\nASCII\n1\n";
        // A: 0 1 1 0 0   B: 1 1 0 0 1
        let dat = "1,0,0,1\n2,1000,1,1\n3,2000,1,0\n4,3000,0,0\n5,4000,0,1\n";
        let c = Comtrade::from_readers(cfg.as_bytes(), dat.as_bytes()).unwrap();
        let (a, b) = (c.binary(0).unwrap(), c.binary(1).unwrap());

        let ev = events(&[b, a], c.time(), 0..5);
        let short: Vec<_> = ev.iter().map(|e| (e.sample, e.id.as_str(), e.to)).collect();
        assert_eq!(
            short,
            [(2, "A", true), (3, "B", false), (4, "A", false), (5, "B", true)]
        );
        assert_eq!(ev[0].time, -0.001);
        assert!(ev[0].from == !ev[0].to);

        // The first sample of the range is the starting state, not a change.
        let ev = events(&[a], c.time(), 1..4);
        assert_eq!(ev.iter().map(|e| e.sample).collect::<Vec<_>>(), [4]);
        assert!(events(&[a], c.time(), 4..99).is_empty());
    }
}
