//! Progress line on stderr.

use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

use super::fit;

const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// How often the spinner moves on.
const FRAME_TIME: Duration = Duration::from_millis(80);

/// Progress on one stderr line, redrawn in place: an animated frame and a
/// text. Does nothing unless stderr is a terminal.
///
/// Clear it before printing to stdout and draw it again afterwards, so the
/// line doesn't end up between output rows.
pub struct Spinner {
    enabled: bool,
    frame: usize,
    last: Instant,
    text: String,
}

impl Spinner {
    /// A spinner for stderr, active when stderr is a terminal.
    pub fn stderr() -> Self {
        Self {
            enabled: std::io::stderr().is_terminal(),
            frame: 0,
            last: Instant::now(),
            text: String::new(),
        }
    }

    /// Sets the text shown after the frame; drawn with the next [`tick`] or
    /// [`draw`].
    ///
    /// [`tick`]: Self::tick
    /// [`draw`]: Self::draw
    pub fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    /// Advances the animation and redraws, at most every 80 ms however often
    /// it is called (call it on every unit of work and on idle timeouts).
    pub fn tick(&mut self) {
        if self.last.elapsed() >= FRAME_TIME {
            self.last = Instant::now();
            self.frame += 1;
            self.draw();
        }
    }

    /// Draws the current frame and text.
    pub fn draw(&self) {
        if !self.enabled {
            return;
        }
        let text = format!("{} {}", FRAMES[self.frame % FRAMES.len()], self.text);
        // A wrapped line could not be cleared with \r.
        let width = crossterm::terminal::size().map_or(80, |(w, _)| w as usize);
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r\x1b[2K{}", fit(&text, width.saturating_sub(1), false));
        let _ = err.flush();
    }

    /// Removes the line.
    pub fn clear(&self) {
        if self.enabled {
            let mut err = std::io::stderr().lock();
            let _ = write!(err, "\r\x1b[2K");
            let _ = err.flush();
        }
    }
}
