//! The loaded recording as the TUI shows it: visible channels, time window
//! (zoom, pan) and cursor.

use std::sync::Arc;

use crate::comtrade::{Comtrade, RecordingPath};

/// A loaded recording and its view state.
#[derive(Debug)]
pub struct Recording {
    /// Recording data.
    pub data: Arc<Comtrade>,
    /// File name shown in the header.
    pub name: String,
    /// Size of the data file in bytes.
    pub size: Option<u64>,
    /// Visibility of each analog channel.
    pub visible_analog: Vec<bool>,
    /// Visibility of each binary channel.
    pub visible_binary: Vec<bool>,
    /// Visible time window (seconds relative to trigger).
    pub view: (f64, f64),
    /// Cursor time.
    pub cursor: f64,
}

/// Default number of analog channels shown after loading.
const DEFAULT_ANALOG: usize = 6;
/// Default number of binary channels shown after loading.
const DEFAULT_BINARY: usize = 8;

impl Recording {
    pub(super) fn new(data: Comtrade) -> Self {
        let name = data.path.as_ref().map(RecordingPath::name).unwrap_or_default();
        let size = data.path.as_ref().and_then(RecordingPath::data_size);
        let visible_analog = (0..data.analog_channels().len()).map(|i| i < DEFAULT_ANALOG).collect();
        // Binary channels that never change are rarely interesting.
        let mut shown = 0;
        let visible_binary = data
            .binary_channels()
            .map(|b| {
                let show = shown < DEFAULT_BINARY && b.changes();
                shown += usize::from(show);
                show
            })
            .collect();
        let view = data.time_span();
        // Cursor on the sample closest to the trigger.
        let cursor = data.index_at(0.0).map_or(0.0, |i| data.time()[i]);
        Self {
            data: Arc::new(data),
            name,
            size,
            visible_analog,
            visible_binary,
            view,
            cursor,
        }
    }

    pub(super) fn n_channels(&self) -> usize {
        self.visible_analog.len() + self.visible_binary.len()
    }

    pub(super) fn visible_mut(&mut self, i: usize) -> Option<&mut bool> {
        let na = self.visible_analog.len();
        if i < na {
            self.visible_analog.get_mut(i)
        } else {
            self.visible_binary.get_mut(i - na)
        }
    }

    /// Shortest allowed view span.
    pub(super) fn min_span(&self) -> f64 {
        let t = self.data.time();
        let (a, b) = self.data.time_span();
        if t.len() > 1 {
            ((b - a) / (t.len() - 1) as f64 * 8.0).max(1e-9)
        } else {
            1e-3
        }
    }

    /// Clamps the view to the recording and to the minimum span.
    pub(super) fn clamp_view(&mut self) {
        let (first, last) = self.data.time_span();
        let full = last - first;
        let mut span = (self.view.1 - self.view.0).max(self.min_span().min(full));
        if span >= full {
            self.view = (first, last);
            return;
        }
        span = span.max(f64::EPSILON);
        let mut start = self.view.0.clamp(first, last - span);
        if start + span > last {
            start = last - span;
        }
        self.view = (start, start + span);
    }

    pub(super) fn zoom(&mut self, factor: f64) {
        let (a, b) = self.view;
        let span = b - a;
        let new_span = span * factor;
        let anchor = if (a..=b).contains(&self.cursor) {
            self.cursor
        } else {
            (a + b) / 2.0
        };
        let start = anchor - (anchor - a) * factor;
        self.view = (start, start + new_span);
        self.clamp_view();
    }

    pub(super) fn pan(&mut self, fraction: f64) {
        let shift = (self.view.1 - self.view.0) * fraction;
        self.view = (self.view.0 + shift, self.view.1 + shift);
        self.clamp_view();
        if !(self.view.0..=self.view.1).contains(&self.cursor) {
            // Keep the cursor on a sample inside the view.
            let range = self.data.index_range(self.view.0, self.view.1);
            let t = self.data.time();
            if !range.is_empty() {
                let i = if self.cursor < self.view.0 {
                    range.start
                } else {
                    range.end - 1
                };
                self.cursor = t[i];
            }
        }
    }

    pub(super) fn move_cursor(&mut self, direction: f64) {
        let step = (self.view.1 - self.view.0) / 100.0;
        let t = self.data.time();
        let mut new = self.cursor + direction * step;
        // Always move at least one sample.
        if let (Some(i), Some(j)) = (self.data.index_at(self.cursor), self.data.index_at(new))
            && i == j
        {
            let k = if direction > 0.0 {
                (i + 1).min(t.len() - 1)
            } else {
                i.saturating_sub(1)
            };
            new = t[k];
        }
        self.set_cursor(new);
    }

    /// Moves the cursor to the sample closest to `t`, so the shown time is
    /// exactly the time of the sample whose values are shown.
    pub(super) fn set_cursor(&mut self, t: f64) {
        let (first, last) = self.data.time_span();
        let t = t.clamp(first, last);
        self.cursor = self.data.index_at(t).map_or(t, |i| self.data.time()[i]);
        let span = self.view.1 - self.view.0;
        if self.cursor < self.view.0 {
            self.view = (self.cursor, self.cursor + span);
        } else if self.cursor > self.view.1 {
            self.view = (self.cursor - span, self.cursor);
        }
        self.clamp_view();
    }
}
