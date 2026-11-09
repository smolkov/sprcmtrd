# `compare --trend` and `compare --plot`

- **Issue:** none, from `todo.md` (follow-ups of `compare`, see
  `tasks/compare-command.md`)
- **Status:** built as planned. `--pair` brings the pair columns back like `--channels`. The chart code moved to `command/chart.rs`; `step_grid` (in `command/mod.rs`) is shared with `impedance` / `power --trend`, and `window_b` moved to `analysis::compare`. The library `trend` takes `Aligned` (A, B, offset) and `Pair`s (channels and B's scale).

## Goal

`compare` shows one moment. These two options show how A and B develop over
the event:

- `--trend`: the ratio, the angle difference and the differential current
  cycle by cycle, as `impedance --trend` does for the loops. For two line ends
  you can read off when the differential current rises (fault inception) and
  falls (breaker open at each end).
- `--plot`: the waveforms of each channel pair on one time axis, so wiring,
  polarity, a CT saturating at one end, or a missed alignment show at a glance.

## What I checked

- `textplots` 0.8.7 (already a dependency, used by `view`) can draw several
  shapes in one chart, and colored lines via `linecolorplot` (needs the `rgb`
  crate as a direct dependency). A quick prototype with A as lines and B as
  points shows that at terminal resolution, the points of a dense waveform join
  into a line. Without color the two curves can't be told apart, and
  `agent.md` says color must never be the only cue.
- `analysis::trend` steps over one recording's own samples. With B
  aligned and maybe at another sample rate, B's windows must end at the same
  instants as A's, so the compare trend needs its own small loop (it reuses
  `Window::ending_at` and the DFT, as `compare` does now).

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Plot layout | Per pair, **A above B**, both on the same y range and the same time axis (A's time, B shifted by the alignment and scaled to A's unit), labelled `IA (A)` / `IA (B)`. Readable without color, and the shapes line up column by column | One overlaid chart, A and B in two colors (`linecolorplot`, new `rgb` dependency); without color (pipe, `NO_COLOR`), stacked as proposed. Two layouts to test, and overlapping curves mix colors in a braille cell |
| Difference plot | Not in v1 | A third plot of B − A per pair: needs resampling B onto A's samples (linear interpolation) |
| Plot options | `--from` / `--to` (A's time, default A's whole recording), `--width`, `--height` as in `view`; `--channels` / `--pair` choose the pairs. `--plot` ignores `--at` / `--cycles` | |
| Trend rows | One row per step over `--from` / `--to` in A's time, `--step` in cycles or ms (default 1 cycle), windows of `--cycles` ending at the step time in A and at the aligned instant in B | Steps on B's samples: confusing, the table would be in B's time |
| Trend columns | Per pair `<id> B/A` and `<id> B−A` (angle). With `--differential`: per phase `<phase> diff [A]` and `<phase> %` (I_diff / I_restraint). With `--differential` and no `--channels`, only the differential columns (otherwise 8 pairs make 16 more columns); `--channels` brings pairs back | Always all columns: with all channels and `--differential` that's 22 columns |
| Output format | Replace `--json` with `--format text\|csv\|json`, as `impedance` and `trend` have it. `csv` only with `--trend`. `compare` isn't released yet, so nothing depends on `--json` | Keep `--json` and add `--format`: two options for the same thing |
| `--trend` and `--plot` together | An error: one or the other | Both outputs one after the other |
| Steps where B has no samples | Empty cells (text `-`, CSV empty, JSON `null`), as other trends do for missing values | Leave those rows out |

Decided by existing behaviour:

- `--step` parses as in `trend` (`1`, `0.5`, `10ms`). No window fits in the
  range: an error, as in `impedance --trend`.
- The alignment (`--align`, `--shift`), pairs, unit scaling and the angle
  rotation are the same as for one moment. The angle reference isn't needed for
  the trend (differences only).

## Output

```
$ sprcmtrd compare end_a.cfg end_b.cfg --trend --differential --from -20 --to 80
A          end_a.cfg (end_a)
B          end_b.cfg (end_b)
Alignment  absolute time: B's trigger at +12.00 ms in A's time
Window     1 cycle at 50 Hz, step 1 cycle, 5 rows
Values     primary

time      A diff [A]  A %      B diff [A]  B %    C diff [A]  C %
-1.0 ms   0.000       0.0 %    0.000       0.0 %  0.000       0.0 %
19.0 ms   4.239k      199.8 %  0.000       0.0 %  0.000       0.0 %
…
```

```
$ sprcmtrd compare end_a.cfg end_b.cfg --plot --channels IA --from -40 --to 80
A          end_a.cfg (end_a)
B          end_b.cfg (end_b)
Alignment  absolute time: B's trigger at +12.00 ms in A's time
Window     -40.0 ms .. 80.0 ms in A's time
Values     primary

IA (A) [A]
<braille plot>
IA (B) [A], B's IA, shifted by +12.00 ms
<braille plot, same y range>
```

(Numbers illustrative.) JSON with `--trend`: the header fields as for one
moment, plus `step_s`, `samples`, and `steps[]` (`time_s`, `pairs[]` with
`a`, `ratio`, `angle_difference`, and `differential[]` with `phase`, `diff`,
`percent`). `--plot` is text only: `--format csv/json` with `--plot` is an
error.

## Implementation plan

### 1. Library: `src/analysis/compare.rs`

- `pub struct TrendStep { time: f64, pairs: Vec<Option<(Complex, Complex)>> }`
  and `pub fn trend(a, b, channels: &[(&AnalogChannel, &AnalogChannel, f64)], d, range, samples, step, frequency, side) -> Vec<TrendStep>`:
  A's windows end at every `step`-th sample in `range`, and B's at the aligned
  instant (`None` outside B). The phasors are peak values, B's rotated and scaled.
- Unit tests with the synthetic recordings of the existing tests: the ratio and
  angle stay constant over the steps for a steady signal, B outside its span
  gives `None`, and B at another sample rate gives the same values.

### 2. Command: `src/args.rs`, `src/command/compare.rs`

- New options `--trend`, `--plot`, `--from`, `--to`, `--step`, `--width`,
  `--height`; `--format` replaces `--json`.
- `--plot`: `plot_segments` of A over the range, and of B over the
  aligned range with times shifted by d and values scaled. The chart code
  of `view` moves to a shared function (`command::view::chart`, or
  `format`) so both draw alike; the y range comes from A and B together.
- The trend table, CSV and JSON as above.

### 3. Tests: `tests/cli.rs`

- The two-ended pair from the `compare` tests:
  - `--trend --differential`: phase A's diff is near 0 before 1.000 s and
    near 4.24 kA after.
  - CSV header and row count; JSON `steps`.
- `--plot`: both labels per pair, the shift in the label, and a fixed
  `--width` / `--height` for a stable layout.
- `fault_2013` against `test_ascii.cff` with `--trend`: ratio 1.000 in every
  row.
- Errors: `--trend` with `--plot`, `--format csv` without `--trend`,
  `--format json` with `--plot`.
- The existing `compare` tests switch from `--json` to `--format json`.

### 4. Docs

- `spec.md` `compare` section; `docs/src/commands/compare.md` (options table,
  a trend and a plot example); README `compare` section.
- `todo.md`: tick both follow-ups (add "difference plot B − A" as an idea).
- `tasks/compare-command.md`: note that `--json` became `--format`.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`.
