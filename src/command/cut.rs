//! `cut` command: a time window and/or some channels of a recording as a new
//! recording.

use crate::term::outln;
use anyhow::bail;

use super::convert;
use crate::args::CutArgs;
use crate::comtrade::Comtrade;
use crate::context::Context;
use crate::format::time;

impl CutArgs {
    /// Runs the `cut` command. The new recording keeps the source's revision and
    /// data format and is written like `convert` writes (`.cfg` or `.cff`,
    /// `force` to replace existing files, read back before success).
    pub fn run(self, _ctx: &Context) -> anyhow::Result<()> {
        let Self {
            file,
            output,
            selection,
            force,
        } = self;
        let src = Comtrade::open(&file)?;
        let (analog, binary) = selection.channels(&src)?;
        let (t0, t1) = selection.range(&src)?;
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
        outln!(
            "{} -> {} ({} {})",
            file.display(),
            output.display(),
            saved.revision,
            saved.format
        )?;
        outln!(
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
        )?;
        if !(first..=last).contains(&0.0) {
            outln!("  note: the trigger is outside the window; times stay relative to it")?;
        }
        saved.print()?;
        Ok(())
    }
}
