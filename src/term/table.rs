//! Table printed row by row as rows arrive.

use std::ops::RangeInclusive;

use super::fit;

/// A table printed row by row as rows arrive, so its columns can't be
/// measured in advance: fixed widths, the first column taking what the
/// terminal leaves. Lines are cut at the terminal width instead of wrapping.
#[derive(Debug, Clone)]
pub struct LiveTable {
    first: usize,
    widths: Vec<usize>,
    width: usize,
}

/// Space between columns.
const GAP: usize = 2;

impl LiveTable {
    /// A table with the columns `widths` after the first one. The first
    /// column gets what a terminal `terminal_width` wide leaves, within
    /// `first`; without a terminal width lines are not cut.
    pub fn new(widths: &[usize], first: RangeInclusive<usize>, terminal_width: Option<usize>) -> Self {
        let width = terminal_width.unwrap_or(usize::MAX);
        let others: usize = widths.iter().map(|w| w + GAP).sum();
        Self {
            first: width.saturating_sub(others).clamp(*first.start(), *first.end()),
            widths: widths.to_vec(),
            width,
        }
    }

    /// A row of `cells`: the first keeps the end of long text (paths), the
    /// others are padded or shortened to their width. Cells beyond the
    /// widths follow unpadded.
    pub fn row<S: AsRef<str>>(&self, cells: &[S]) -> String {
        let mut s = String::new();
        for (i, cell) in cells.iter().enumerate() {
            let cell = cell.as_ref();
            match i.checked_sub(1).map(|j| self.widths.get(j)) {
                None => push_cell(&mut s, cell, self.first, true),
                Some(Some(&w)) => push_cell(&mut s, cell, w, false),
                Some(None) => {
                    s.push_str(cell);
                    s.push_str("  ");
                }
            }
        }
        self.cut(&s)
    }

    /// A row with the first cell and then `text` across the other columns
    /// (for a message instead of values).
    pub fn row_with_text(&self, first: &str, text: &str) -> String {
        let mut s = String::new();
        push_cell(&mut s, first, self.first, true);
        s.push_str(text);
        self.cut(&s)
    }

    fn cut(&self, s: &str) -> String {
        fit(s.trim_end(), self.width, false)
    }
}

/// Appends `cell` fitted and padded to `width`, then the column gap.
fn push_cell(s: &mut String, cell: &str, width: usize, keep_end: bool) {
    let cell = fit(cell, width, keep_end);
    let pad = width.saturating_sub(cell.chars().count());
    s.push_str(&cell);
    s.extend(std::iter::repeat_n(' ', pad + GAP));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_table_has_fixed_columns() {
        let table = LiveTable::new(&[4, 3], 6..=20, Some(30));
        // 30 - (4 + 2) - (3 + 2) = 19 for the first column.
        assert_eq!(
            table.row(&["a.cfg", "S", "1"]),
            format!("a.cfg{}S     1", " ".repeat(16))
        );
        let long = table.row(&[format!("{}/a.cfg", "x".repeat(40)).as_str(), "S", "1"]);
        assert!(long.starts_with('…') && long.contains("/a.cfg  S"));
        assert_eq!(long.chars().count(), table.row(&["a.cfg", "S", "1"]).chars().count());
        // Wider cells are shortened, extra cells follow, and lines are cut.
        assert_eq!(table.row(&["a", "STATION", "1"]), format!("a{}STA…  1", " ".repeat(20)));
        assert_eq!(table.row(&["a", "S", "1", "extra text"]).chars().count(), 30);

        // Within the limits of the first column, also without a terminal.
        let narrow = LiveTable::new(&[4, 3], 6..=20, Some(10));
        assert!(narrow.row(&["abcdefghij", "S", "1"]).starts_with("…fghij  S"));
        let wide = LiveTable::new(&[4, 3], 6..=20, None);
        assert_eq!(wide.row_with_text("a", &"x".repeat(100)).chars().count(), 22 + 100);
    }
}
