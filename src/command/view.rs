//! `view` command: summary and plots printed to stdout.

use super::chart::{self, LABEL_WIDTH};
use crate::args::ViewArgs;
use crate::comtrade::{Cell, Comtrade};
use crate::context::Context;
use crate::format::{key_values, summary, time};
use crate::term;

impl ViewArgs {
    /// Runs the `view` command.
    pub fn run(self, ctx: &Context) -> anyhow::Result<()> {
        let c = Comtrade::open(&self.file)?;
        let side = ctx.global.side();
        let (analog, binary) = self.selection.channels(&c)?;
        let (t0, t1) = self.selection.range(&c)?;
        let width = self.width.map(usize::from).or_else(term::terminal_width).unwrap_or(100);
        let plot_cols = width.saturating_sub(LABEL_WIDTH).max(16);
        let range = c.index_range(t0, t1);

        let name = c.name();
        let mut out = key_values(&summary(&name, &c.config));
        out += &key_values(&[
            ("Window", format!("{} .. {}", time(t0, t1 - t0), time(t1, t1 - t0))),
            ("Values", format!("{side:?}").to_lowercase()),
        ]);

        for i in analog {
            let ch = c.analog(i).expect("selected index is valid");
            out += &format!("\nA{} {} [{}]\n", ch.info.index, ch.info.id, ch.info.unit);
            let segments = ch.plot_segments(c.time(), range.clone(), (t0, t1), plot_cols * 2, side);
            out += &chart::plot(&segments, (t0, t1), None, plot_cols, self.height.max(1) as usize);
        }

        if !binary.is_empty() {
            out += "\n";
        }
        for i in binary {
            let ch = c.binary(i).expect("selected index is valid");
            let strip: String = ch
                .strip(c.time(), (t0, t1), plot_cols)
                .into_iter()
                .map(|cell| match cell {
                    Cell::Empty => ' ',
                    Cell::Low => '▁',
                    Cell::High => '█',
                    Cell::Mixed => '▌',
                })
                .collect();
            out += &format!("{strip} D{} {}\n", ch.info.index, ch.info.id);
        }
        print!("{out}");
        Ok(())
    }
}
