//! Terminal output shared by the commands: widgets (a spinner on stderr for
//! long-running work, a table whose rows are printed as they arrive), colors,
//! and small helpers for fitting text to the terminal.

mod spinner;
mod style;
mod table;

use std::io::IsTerminal;

pub use spinner::Spinner;
pub use style::{Palette, Role};
pub use table::LiveTable;

/// Width of the terminal stdout is connected to, `None` when it is not a
/// terminal.
pub fn terminal_width() -> Option<usize> {
    if !std::io::stdout().is_terminal() {
        return None;
    }
    crossterm::terminal::size().ok().map(|(w, _)| w as usize)
}

/// Shortens `s` to `width` characters with `…`, cutting its start when
/// `keep_end` (for paths, whose end is the file name).
pub fn fit(s: &str, width: usize, keep_end: bool) -> String {
    let n = s.chars().count();
    if n <= width {
        return s.to_owned();
    }
    let keep = width.saturating_sub(1);
    if keep_end {
        format!("…{}", s.chars().skip(n - keep).collect::<String>())
    } else {
        format!("{}…", s.chars().take(keep).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_text_keeping_start_or_end() {
        assert_eq!(fit("abc", 3, true), "abc");
        assert_eq!(fit("station/2024/fault.cfg", 10, true), "…fault.cfg");
        assert_eq!(fit("SUBSTATION NORTH 1", 10, false), "SUBSTATIO…");
    }
}
