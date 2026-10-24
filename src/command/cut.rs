//! `cut` command: a time window and/or some channels of a recording as a new
//! recording.

use anyhow::bail;

use super::{Context, convert, select_channels, time, time_window};
use crate::args::CutArgs;
use crate::comtrade::Comtrade;

impl CutArgs {
    /// Runs the `cut` command. The new recording keeps the source's revision and
    /// data format and is written like `convert` writes (`.cfg` or `.cff`,
    /// `force` to replace existing files, read back before success).
    pub fn run(self, _ctx: &Context) -> anyhow::Result<()> {
        let Self {
            file,
            output,
            from: from_ms,
            to: to_ms,
            channels,
            force,
        } = self;
        let src = Comtrade::open(&file)?;
        let (analog, binary) = select_channels(&src, &channels)?;
        let (t0, t1) = time_window(&src, from_ms, to_ms)?;
        let range = src.index_range(t0, t1);
        if range.is_empty() {
            let (first, last) = src.time_span();
            let span = last - first;
            bail!(
                "no samples in the time window; the recording runs from {} to {}",
                time(first, span),
                time(last, span)
            );
        }
        let cut = src.slice(range.clone(), &analog, &binary);
        let saved = convert::save(&src, &cut, &output, None, None, force)?;

        let (first, last) = cut.time_span();
        let span = last - first;
        println!(
            "{} -> {} ({} {})",
            file.display(),
            output.display(),
            saved.revision,
            saved.format
        );
        println!(
            "  kept samples {}..={} of {} ({} .. {}), {} of {} analog and {} of {} binary channels",
            range.start + 1,
            range.end,
            src.len(),
            time(first, span),
            time(last, span),
            analog.len(),
            src.analog_channels().len(),
            binary.len(),
            src.binary_channels().len()
        );
        if !(first..=last).contains(&0.0) {
            println!("  note: the trigger is outside the window; times stay relative to it");
        }
        saved.print();
        Ok(())
    }
}
