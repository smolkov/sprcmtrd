//! Event stream for the TUI: key presses, resizes, ticks and background task results.

use std::path::PathBuf;
use std::time::Duration;

use crossterm::event::{Event as TermEvent, EventStream, KeyEvent, KeyEventKind};
use futures::StreamExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{Interval, MissedTickBehavior};

use super::app::PickerEntry;
use crate::comtrade::Comtrade;

/// Result of a background task.
#[derive(Debug)]
pub enum Message {
    /// A recording finished loading.
    Loaded {
        /// Requested path.
        path: PathBuf,
        /// The recording, or the error text.
        result: Result<Box<Comtrade>, String>,
    },
    /// The directory scan found an entry.
    ScanEntry {
        /// Scan the entry belongs to.
        generation: u64,
        /// The entry.
        entry: PickerEntry,
    },
    /// The directory scan finished.
    ScanDone {
        /// Scan that finished.
        generation: u64,
        /// Error that stopped the scan.
        error: Option<String>,
    },
}

/// Something the event loop has to react to.
#[derive(Debug)]
pub enum Event {
    /// A key was pressed.
    Key(KeyEvent),
    /// The terminal was resized.
    Resize,
    /// Periodic tick for animations and status timeouts.
    Tick,
    /// A background task finished.
    Message(Message),
}

/// Interval between ticks.
const TICK: Duration = Duration::from_millis(120);

/// Merges terminal events, ticks and background messages.
pub struct Events {
    terminal: EventStream,
    tick: Interval,
    rx: UnboundedReceiver<Message>,
}

impl Events {
    /// Creates the stream; `rx` receives background task results.
    pub fn new(rx: UnboundedReceiver<Message>) -> Self {
        let mut tick = tokio::time::interval(TICK);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        Self {
            terminal: EventStream::new(),
            tick,
            rx,
        }
    }

    /// Waits for the next event. `None` when the terminal input closed.
    pub async fn next(&mut self) -> std::io::Result<Option<Event>> {
        loop {
            tokio::select! {
                ev = self.terminal.next() => match ev {
                    // Windows also reports key releases.
                    Some(Ok(TermEvent::Key(k))) if k.kind != KeyEventKind::Release => {
                        return Ok(Some(Event::Key(k)));
                    }
                    Some(Ok(TermEvent::Resize(..))) => return Ok(Some(Event::Resize)),
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => return Err(e),
                    None => return Ok(None),
                },
                _ = self.tick.tick() => return Ok(Some(Event::Tick)),
                Some(msg) = self.rx.recv() => return Ok(Some(Event::Message(msg))),
            }
        }
    }
}
