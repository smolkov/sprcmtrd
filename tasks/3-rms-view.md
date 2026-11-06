# RMS view in the plot area

- **Issue:** [smo/sprcmtrd#3](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/3), "Zusätzlicher View für die RMS-Werte"
- **Branch:** `3-zusatzlicher-view-fur-die-rms-werte`
- **Status:** built with the proposed decisions, including stage 2

## Goal

The plot area shows analog channels as recorded. Add a second view, switched with
a key, that shows the sliding RMS of each channel, either with the DC part or with
it removed.

Optional in the issue: fit the RMS window to the frequency of the fundamental.
This is stage 2 below, in its own commit, so it can be dropped or postponed.

The issue says "mit einem umschalten" without naming what; this is read as a key,
like the other view switches.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Keys | `r` switches waveform / RMS, `d` switches with / without DC. Both are free and can be rebound | One key cycling waveform → RMS → RMS without DC: fewer keys, but more presses |
| Default after switching on | With DC, the true RMS, equal to `trend --rms` for the same window | DC removed |
| Window length | One cycle, fixed. It removes the ripple at the fundamental and reacts within one cycle | A configurable number of cycles |
| Window position | Trailing: it ends at the sample, as in `phasors` and `trend`, so a step shows up over the following cycle, as a relay sees it | Centred: lines up with the binary strips, but looks into the future and differs from `trend` |
| Start of the recording | No value until a whole window fits, shown as a gap | A partial window: values for the first cycle that aren't comparable |
| Frequency-fitted window (stage 2) | Build it, always on where a frequency can be measured, falling back to nominal elsewhere | Postpone until issue #2 is settled, because it needs the same choice of reference channel |

Decided by existing behaviour, no need to confirm:

- A window with a missing sample has no value and shows as a gap, like `true_rms`.
- Without DC is `sqrt(mean(v²) − mean(v)²)` over the same window, clamped at 0
  against float noise.
- The view and DC setting apply to all analog channels and stay when another file
  is opened, like `p` for primary / secondary. Binary strips, cursor, zoom, pan and
  the time axis don't change.
- The y axis starts at 0 in the RMS view, so a steady value with small noise doesn't
  look like a large swing.

## Implementation plan

The code below hasn't been compiled. Re-read `spec.md` before starting, as
`agent.md` asks.

### Stage 1: RMS view with a nominal window

#### 1. Compute the sliding RMS (`src/analysis/rms.rs`, new)

- [x] Add the module to `src/analysis/mod.rs`, and to the `src/analysis/` line in
      the architecture section of `agent.md`:

  ```rust
  /// What happens to the DC part of the signal.
  pub enum Dc {
      /// Included: the true RMS.
      Keep,
      /// Removed: the mean of the window is subtracted.
      Remove,
  }

  /// Sliding RMS of `ch` for each sample in `range`, over the window that ends
  /// at the sample. `seconds` gives the window length at a sample. `None` where a
  /// whole window doesn't fit, has fewer than 4 samples, or has a missing value.
  pub fn sliding(
      ch: &AnalogChannel,
      time: &[f64],
      range: Range<usize>,
      seconds: impl Fn(usize) -> f64,
      dc: Dc,
      side: Side,
  ) -> Vec<Option<f64>>
  ```

- [x] Find each window's samples from the sample times, so recordings with several
      sample rates work. A window holds the samples with
      `t ≥ t_end − seconds + dt / 2`, which gives the same `round(T / dt)` samples
      as `Window::ending_at`.
- [x] Use running totals of `v`, `v²` and missing samples from the start of the
      first window to the end of `range`. Each window is then O(1), and the cost per
      channel and draw is O(samples in view + window), the same order as
      `plot_segments`. Values are never stored between draws.

#### 2. Share the decimation (`src/comtrade/mod.rs`)

- [x] Move the body of `AnalogChannel::plot_segments` into a function that takes the
      values as a closure, and let `plot_segments` call it. The RMS view then keeps
      min/max per column and its gaps without a second copy:

  ```rust
  pub fn decimated_segments(
      time: &[f64],
      range: Range<usize>,
      span: (f64, f64),
      columns: usize,
      value: impl FnMut(usize) -> Option<f64>,
  ) -> Vec<Vec<(f64, f64)>>
  ```

  The existing `plot_segments_decimate_and_split_on_gaps` test covers the move.

#### 3. Add the actions and state (`src/tui/keys.rs`, `src/tui/app.rs`)

- [x] Add `Action::RmsView` and `Action::RmsDc` at the end of `Action`, so a key
      already bound to another action in a user's `config.toml` keeps working.
- [x] Add the bindings to `BINDINGS` after `Action::Side`, without footer hints: the
      footer is already cut off at 80 columns.

  ```rust
  Binding {
      action: Action::RmsView,
      name: "rms_view",
      keys: &["r"],
      help: "Toggle waveform / sliding RMS",
      hint: None,
  },
  Binding {
      action: Action::RmsDc,
      name: "rms_dc",
      keys: &["d"],
      help: "RMS with / without DC",
      hint: None,
  },
  ```

- [x] Add `App::plot_view` (`PlotView::Waveform` or `PlotView::Rms`) and `App::rms_dc`
      (`Dc::Keep` at the start).
- [x] `r` switches the view and shows the window and DC setting in the footer, for
      example `RMS with DC, 1 cycle (20.00 ms)`. Without a usable line frequency
      (`config.line_frequency` can be 0), it stays on the waveform and shows an error.
      Without a recording it shows the usual "no recording loaded" message.
- [x] `d` switches the setting and shows it in the footer, in either view.

#### 4. Draw the view (`src/tui/widgets/plot.rs`)

- [x] In `analog_plot`, in the RMS view, build `segments` from `rms::sliding` and
      `decimated_segments`, start the y axis at 0, and take the `▸` cursor value from
      a one-sample `rms::sliding` call. Everything else in the function stays.
- [x] Show the view in the block title in place of `Plot`: `RMS` or `RMS-DC`. The
      title has little room at 80×24 with the sidebar open, so check that it still
      reads there; the footer message gives the details.

#### 5. Add the stage 1 tests

- [x] Add the tests listed under [Tests](#tests).

#### 6. Update the documentation

- [x] Add the keys to the key tables in `spec.md`, `README.md`,
      `docs/src/viewer/index.md` and `docs/src/viewer/keys.md`.
- [x] Describe the view in the Plots section of `spec.md` and the viewer sections of
      `README.md` and `docs/src/viewer/index.md`, including the trailing window.
- [x] Describe the method in `docs/src/analysis/index.md`.
- [x] Record the task in `todo.md`.

### Stage 2: window fitted to the fundamental (optional, separate commit)

Off the nominal frequency, a fixed window no longer holds whole cycles, so the RMS
ripples. Fitting the window removes that.

- [x] Add a function to `src/analysis/frequency.rs` that picks one reference channel
      for the recording: the strongest voltage, otherwise the strongest current, by
      unit, as `Quantity` in `impedance.rs` tells them apart. Issue #2 asks for the
      same choice in `trend`, so it stays reusable, but `trend` doesn't change here.
- [x] Measure the frequency with the existing `crossings` and `from_crossings`, over
      one period ending at the sample. Where nothing can be measured (no signal, a
      pause, a missing value), use the nominal window.
- [x] Compute the crossings once per recording and keep them in `Recording`, because
      they cover all samples. They don't depend on primary / secondary.
- [x] Pass `|i| 1.0 / frequency_at(i)` as `seconds` to `rms::sliding`. The window can
      get shorter or longer from one sample to the next; the running totals allow that.
- [x] Name the reference channel in the footer message from `r`, for example
      `RMS with DC, 1 cycle (fitted to UL1)`.
- [x] Add the stage 2 tests under [Tests](#tests) and describe the method in `spec.md`
      and `docs/src/analysis/index.md`.

## Tests

Stage 1:

- [x] Unit, `rms::sliding`, on synthetic recordings like those in `analysis/trend.rs`:
  - A 100 A peak sine gives 70.71 A.
  - With 10 A DC added: 71.41 A with DC, 70.71 A without.
  - A pure DC signal gives 0 without DC.
  - A step reaches the new value exactly one cycle later.
  - No value before the first whole window, around a missing sample, or with fewer
    than 4 samples per window.
  - The same value as `trend --rms` at the same window end, on a rate where a cycle
    isn't a whole number of samples (60 Hz at 1 kHz).
  - Scaled by the channel's factor on the secondary side.
  - Several sample rates in one recording.
- [x] Unit, `keys.rs`: `r` and `d` map to the new actions; a user's existing binding
      of `r` or `d` to another action still wins.
- [x] TUI, `tui/widgets/mod.rs`:
  - `r` changes the title and the `▸` value, and `r` again restores the waveform
    screen exactly.
  - `d` changes the value on a recording with DC.
  - Binary strips and the time axis stay the same.
  - The title reads at 80×24 with the sidebar open.
  - Line frequency 0 keeps the waveform and shows the error.
- [x] Real terminal: in `tmux`, run `tests/data/fault_1999.cfg` and the recording from
      issue #7, and check `r`, `d`, zoom, pan and cursor in the RMS view.
- [x] Large file: time zooming and panning in a release build on a generated recording
      of about 1 million samples, with and without the RMS view. If it lags noticeably,
      report it before adding any cache.
- [x] Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
      `cargo test --locked`, `cargo +1.88 check --locked --all-targets` and
      `mdbook build docs`.

Stage 2:

- [x] Unit: a 55 Hz sine in a 50 Hz recording ripples with the nominal window and stays
      flat, within 0.5 %, with the fitted one.
- [x] Unit: the reference is the strongest voltage, or the strongest current without
      usable voltages. Without a measurable frequency, the window is the nominal one.
- [x] Real terminal: `55Hz.cfg` / `.dat`, attached to issue #1, outside the repository.

## Results

- All listed commands pass.
- Real terminal, in `tmux`: `tests/data/fault_1999.cfg`, `long1_t1_anstatt_t2`, and
  `55Hz` from issue #1, whose RMS lines stay thin and flat over 16 s (fitted to `uL3E`).
- Large file, 1 million samples at 10 kHz, release build, 6 channels, drawing data only:

  | View | Waveform | RMS |
  |---|---|---|
  | Whole recording | 53 ms | 405 ms |
  | A tenth of it | 5 ms | 36 ms |

  Measuring the frequency once takes 35 ms. As planned, no cache was added: at this
  size the whole-recording RMS view lags noticeably on each key, while a recording of
  65 000 samples like `55Hz` should scale to about 26 ms (estimated, not measured).
- At 80×24 with a long sidebar the title has no room for `channels 1–4 of 14`, as
  already happens with `Plot`. `RMS-DC` stays readable, and the footer message starts
  with the DC setting because long messages are cut off on the right.

## Not included

- A numeric RMS column, or RMS in the sidebar.
- RMS of binary channels, and any change to `trend`, `stats` or the frequency commands,
  including the reference channel in `trend` (issue #2).
- Peak or fundamental-only views.
- A configurable number of cycles.
- Mouse control or footer hints for the new keys.
