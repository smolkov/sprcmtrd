# TUI: a harmonics view over time

- **Issue:** GitLab, "Darstellung der Harmonischen" (number to fill in)
- **Status:** built as planned. `harmonics::sliding` computes its own window of exactly one cycle (the oldest sample weighted by the part inside) instead of reusing the RMS window starts: off nominal that keeps the 3rd harmonic within 0.03 % instead of 2 %. One point per braille dot column. The help overlay shows the nine order keys on one line.

## Goal

Like the RMS view (`r`), a view of the plot area that shows the harmonics
of each analog channel over time. At first only the fundamental (order 1) is
shown; more orders can be switched on and off one at a time, so the plot
stays readable. The windows are fitted to the measured frequency, as in the
RMS view.

## How the RMS view works today

- `r` switches `App::plot_view` between `Waveform` and `Rms`, and the block
  title reads `RMS`. The value in the gutter is the RMS at the cursor.
- The window is one cycle that ends at each sample.
  `Recording::rms_seconds(i)` takes the cycle from `Track::period_at`:
  zero crossings of the strongest voltage, else the strongest current
  (`analysis::frequency::Track`, measured once per recording).
- The y axis starts at 0.

## Calculation

For each plotted point at sample `i`:

- The window is one cycle of the measured frequency f(i) = 1 / `rms_seconds(i)`,
  ending at `i` (the same samples as the RMS view).
- The DFT at h·f(i) gives the order-h phasor; its RMS is |X| / √2.
- With the window fitted to the actual frequency, the harmonics stay at
  whole multiples. With a fixed 50 Hz window at 55 Hz, the fundamental leaks
  into every order: in `55Hz.cfg`, `harmonics --at 1500` shows a "2nd
  harmonic" of 39 % that isn't there.

**Cost:** one DFT costs one cycle of samples per order. The plot computes
it only for the points it draws, at most 2 per column (the braille
resolution), not for every sample. Zoomed in to fewer samples than that,
it computes every sample. For 3 orders, 160 columns and 80 samples per
cycle, that's about 80 000 operations per channel and frame.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Key for the view | `f` (free; "Fourier"), configurable as `harmonics_view`. Title `Harmonics`. `r` and `f` each switch between their view and the waveform; `r` in the harmonics view goes to RMS | `h`: already left / pan |
| Switching orders | In the harmonics view, the digits `1` … `9` turn the orders 1 … 9 on and off (bindings `harmonic_1` … `harmonic_9`). `0` stays "reset zoom". The footer says which orders are shown, e.g. `harmonics 1 3 5` | An overlay with a list of orders 1 … 50 and space to toggle: more keys and more code, and orders above 9 are rarely needed in a terminal plot |
| Default | Only order 1. The choice applies to all channels and stays when another recording is opened (like the RMS DC setting) | Per channel: more state and more keys |
| Several orders of one channel | One plot row per channel and order: `IA h1 [A]`, `IA h3 [%]`. That reads without color. `agent.md` says color must never be the only cue, and braille lines of different orders in one plot can't be told apart | All orders in one plot, by color: unreadable without color, and h1 would flatten the small orders |
| Unit | h1 as RMS in the channel's unit. h ≥ 2 in % of the fundamental at the same moment (the usual measure, e.g. 2nd-harmonic blocking for inrush). The gutter shows the value at the cursor | All in the channel's unit: a 3 % harmonic is a flat line near 0 |
| Y axis | From 0, as in the RMS view | |
| Frequency | As the RMS view: `Track` (zero crossings of the strongest voltage, else current). Without a track: the nominal frequency. No line frequency: the view can't be opened ("no line frequency", like `r`) | `frequency::at` (positive sequence, as `trend` now): a second way to measure the frequency in the TUI. It can replace `Track` for both views later |
| Orders above the sample rate | An order at or above half the sample rate (`harmonics::max_order`) is left out with a footer note, e.g. `order 9 is above half the sample rate` | |
| DC / `d` | `d` changes only the RMS view. The harmonics view has no DC row (`dc-offset` and `harmonics` cover it) | A DC row as "order 0" |
| `harmonics` command | Not changed in this task. Fitting its window to the measured frequency (`--at` off nominal) goes to `todo.md` as its own task | Do it here too: the CLI needs its own decisions (which frequency, how to show it) |

## Implementation plan

### 1. Library: `src/analysis/harmonics.rs`

- `pub fn sliding(ch, time, points: &[usize], seconds: impl Fn(usize) -> f64, orders: &[usize], side) -> Vec<Vec<Option<f64>>>`:
  for each point and order, the RMS of the order over the window of
  `seconds(i)` ending at `i`, from a DFT at order / `seconds(i)`. `None`
  where no whole window fits or a value is missing. It reuses the window
  starts of `analysis::rms` (made `pub(crate)` there).
- Unit tests:
  - A signal with 3rd and 5th harmonics at 50 Hz gives their RMS.
  - The same at 55 Hz with 55 Hz windows gives the same values; with 50 Hz
    windows the leakage shows.
  - `None` before the first whole window and next to a missing value.

### 2. TUI

- `tui/app.rs`: `PlotView::Harmonics` and `App::harmonics: [bool; 9]`
  (order 1 on). `Action::HarmonicsView`, `Action::Harmonic(n)`. The digits
  only act in the harmonics view; there they set a footer message.
- `tui/keys.rs`: the bindings (`f`, `1` … `9`), help texts. The
  configuration overlay lists them like every binding.
- `tui/widgets/plot.rs`: in the harmonics view, one row per visible channel
  and shown order. The gutter shows `IA h3 [%]`, the value at the cursor and
  the range. The points are picked per column and computed with
  `harmonics::sliding`. Scrolling and the minimum height work as now (more
  rows, so it scrolls sooner).
- `tui/widgets/footer.rs` / help: the new keys.

### 3. Tests (TUI, as the RMS view tests in `tui/widgets/mod.rs`)

- `f` switches to `Harmonics`, and again back to the waveform.
- The title, one row `IA h1`. `3` adds `IA h3 [%]`, `3` again removes it.
- The digits do nothing in the waveform view.
- A recording without a line frequency can't open the view.
- A synthetic recording with a 3rd harmonic of 20 %: the gutter value at the
  cursor reads `20.0`.

### 4. Docs

- `docs/src/viewer/index.md` (a "Harmonics view" item next to the RMS view,
  key table), `docs/src/viewer/keys.md`.
- `spec.md` TUI section, README viewer keys.
- `todo.md`: tick it; add "`harmonics` command: window fitted to the measured
  frequency".

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`. `list_content_filters` needs `55Hz.cfg` out of
`tests/data/`.
