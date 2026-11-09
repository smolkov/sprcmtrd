# `compare --plot --difference`: B − A per pair

- **Issue:** none, from `todo.md` (follow-up of `compare --plot`, see
  `tasks/compare-trend-plot.md`)
- **Status:** plan, waiting for approval

## Goal

`compare --plot` shows A above B. Small differences (a ratio error of a few
%, a CT starting to saturate, a sample offset) don't show in two plots on a
large scale. A third plot of B − A per pair, on its own scale, shows them.

## Background

A and B have their own sample times, and B is shifted by the alignment d, so
their samples rarely coincide. B is resampled onto A's samples: for each of
A's times t, B's value at t − d (B's time) is interpolated linearly between
B's two neighbouring samples, then scaled to A's unit. At 1 kHz and 50 Hz,
linear interpolation is off by at most 1 − cos(π·50/1000) ≈ 1.2 % of the peak
between two samples. That is enough to see where A and B part, but it isn't
a measurement. When B samples more slowly than A, this error shows as a
small ripple in B − A.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Option | `--difference` (with `--plot`): adds the third plot. Without `--plot`, an error | Always draw it with `--plot`: one more plot per pair also when nobody wants it |
| Quantity | B − A, with B as plotted (shifted, scaled). `--invert-b` applies to B in the plot too, so for two line ends measured into the line `--invert-b --difference` gives −(A + B), whose size is the differential current | A + B for `--differential`: a second meaning of the same plot |
| Y range | Its own range (usually much smaller than A's) | The same range as A and B: the difference is a flat line |
| Label | `IA B − A [A], max |B − A| 12.3 (0.4 % of A's peak)` over the plotted range | Only the name |
| Where B has no value | A gap in the plot: outside B's span, or when a neighbour sample is missing | Hold the last value |
| Resampling | Linear interpolation | Sinc / cubic: more code for a terminal plot |

## Implementation plan

### 1. Library: `src/analysis/compare.rs`

- `pub fn value_at(ch: &AnalogChannel, time: &[f64], t: f64, side) -> Option<f64>`:
  linear interpolation between the samples around `t`; `None` outside the
  time span or when a neighbour has no value; the sample itself when `t` is
  on one (within 1e-9 s).
- Unit tests: exact on a sample, linear between two, `None` outside and next
  to a missing value.

### 2. Command: `src/args.rs`, `src/command/compare.rs`

- `--difference` (`requires = "plot"`, so without `--plot` it is a usage
  error, exit 2).
- `--invert-b` also turns B in `--plot`; its help text changes accordingly.
- The difference plot: `comtrade::decimated_segments` over A's samples in
  `--from` / `--to` with the value `value_at(B, t − d) · scale − A`, its own y
  range, and the label with the largest |B − A| and its % of A's peak.

### 3. Tests: `tests/cli.rs`

- `fault_2013` against `test_ascii.cff`: `max |B − A| 0.000`.
- The two-ended pair: `IB` with `--invert-b --difference` stays near 0 (load
  through the line). `IA` is large (the internal fault). The labels show up.
- `--difference` without `--plot`: exit 2.

### 4. Docs

- `spec.md`, `docs/src/commands/compare.md` (option, the label, the
  interpolation limit), README `compare` line.
- `todo.md`: tick the item.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`.
