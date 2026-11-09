# `trend`: a steadier frequency from the strongest signal

- **Issue:** [smo/sprcmtrd#2](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/2), "Frequenzberechnung ungenau bei sprcmtrd trend"
- **Status:** done, released in 0.7.0. `Step::frequency` was removed from `analysis::trend` instead of kept: the command computes the frequency per step with `frequency::at` from all channels. Before the voltage comes on in `55Hz.cfg`, the single voltage uNEcalc (a 50 Hz signal of 3.7 kV) is used, as the source order says.

## Problem

`sprcmtrd trend 55Hz.cfg` should show 55 Hz from about 1140 ms on. The
values ripple instead (1500 … 2000 ms: 54.50 … 55.42 Hz). The header says
`Reference A1 iL1`: the frequency comes from the first shown channel, here a
current of a few hundred mA, although the recording has three voltages.

## Cause

`trend` takes the turn of one channel's phasor between two windows. That has
two weaknesses:

1. **The channel:** the first shown channel (or `--reference`), even when it
   is a weak current and voltages are there.
2. **The image:** the DFT runs at the nominal 50 Hz. At 55 Hz, a single
   phase's phasor carries an image (the negative-frequency part of the cosine
   doesn't cancel over a window that isn't whole cycles of 55 Hz). It turns
   the other way and makes the angle, and so the frequency, ripple at twice
   the line frequency.

The `frequency` command already handles both: frequency tracking (it
measures again with windows of whole cycles of the frequency found), and
`Track` picks the strongest voltage, else the strongest current. `trend`
uses neither.

The **positive sequence** of a three-phase group removes the image without
tracking: for three phases 120° apart, the images form a negative sequence
and drop out of (A + a·B + a²·C) / 3. A check on `55Hz.cfg`, uL1E … uL3E,
1500 … 2000 ms, 1-cycle windows every cycle (prototype in Python):

| Measured from | min | max | mean |
|---|---|---|---|
| uL1E, as `trend` now | 54.505 | 55.417 | 54.995 |
| uL1E, with tracking (as `frequency --method phasor`) | 54.929 | 55.072 | 55.005 |
| positive sequence uL1E uL2E uL3E | 54.996 | 55.004 | 55.000 |
| positive sequence, with tracking | 55.000 | 55.001 | 55.000 |

For comparison, `frequency` (zero crossings of uL1E) gives 54.88 … 55.15 Hz.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Source | Per step, the first usable of: the positive sequence of the strongest voltage group, of the strongest current group, the strongest single voltage, the strongest single current. "Strongest" by the peak over the whole recording, as `Track` does. This is the order the issue asks for: voltages first, then currents | Always the strongest channel at that step: the source would jump between phases from row to row |
| Which channels | All analog channels of the recording, not only the shown ones (`--channels IA` still measures from the voltages) | Only the shown channels: then `--channels IA` gives a current frequency again |
| Usable at a step | Both windows (this one and the one before) have a phasor of at least 5 % of the source's peak (`MIN_LEVEL`, as in `frequency`); otherwise the next source; none: `-`. In `55Hz.cfg`, nothing is measured before the voltage comes on at 1120 ms (now: 51.88, 50.14, … from currents of a few mA) | Use the strongest source even when it is near 0: noise as a frequency |
| Tracking | Yes, 2 rounds, as in `frequency`: it removes the rest of the ripple (55.004 → 55.001) and helps single channels most | No tracking: simpler, but single channels keep ±0.07 Hz |
| `--reference` | Only for the angles, as its name says. A new `--frequency-from <CH>` measures from one channel (with tracking) instead of the automatic choice | `--reference` also sets the frequency, as now: one option for two things |
| Showing the source | Header line `Frequency  from uL+ (uL1E uL2E uL3E), else iL+ (iL1 iL2 iL3)`. If the source changes within the table, a column `f from` (`uL+`, `iL+`, `uL1E`, …) after `f [Hz]`. CSV and JSON always have it (`frequency_source`) | Always the column: wider tables for the usual case of one source |
| First row | Gets a frequency too, if two windows fit (the earlier window is measured directly instead of taken from the step before) | |

## Also found (not in this task)

- `frequency 55Hz.cfg --method phasor --cycles 10` shows 49.99 Hz. Over
  10 cycles (200 ms), 5 Hz off nominal turns the phasor by a full turn
  (|Δf|·Δt = 1 > ½), so the turn wraps. Tracking would need a first
  estimate from a shorter window. For `todo.md`.
- `frequency` could use the same positive-sequence source by default. Also
  for `todo.md`, with this task's library function ready for it.

## Implementation plan

### 1. Library: `src/analysis/frequency.rs`

- `pub enum Source { Group(Group), Channel(usize) }` with a name for the
  output (`uL+`, `uL1E`).
- `pub fn sources(channels: &[&AnalogChannel], time) -> Vec<Source>`: the
  order above, each with its peak.
- `pub fn at(channels, time, end, samples, frequency, sources, side) -> Option<(f64, usize)>`:
  frequency and the index of the source used, from the turn between the
  windows ending at `end − samples` and `end`, with tracking (reuses
  `turn_at`'s idea for a phasor function of a group).
- Unit tests:
  - Three phases at 55 Hz give 55.000 ± 0.001 with a 50 Hz window.
  - A single phase with tracking is within 0.01 Hz.
  - With voltages near 0, the current group is used.
  - Without any signal: `None`.
  - Voltages that are only unbalanced still give the right frequency.

### 2. `src/analysis/trend.rs` and `src/command/trend.rs`

- `trend::trend` no longer computes the frequency from the reference
  (`Step::frequency` stays and comes from `frequency::at`; the reference
  parameter goes).
- `TrendArgs`: `--frequency-from <CH>`. `--reference` help: angles only.
- Header line, `f from` column (when the source changes), CSV
  `frequency_source`, JSON `frequency_source` per step.
- Check the other users of `trend::trend` (`trend_steps` in `impedance`,
  `power`; the TUI). They pass `reference 0` and don't show the frequency,
  so nothing changes for them.

### 3. Tests

- CLI test with a synthetic 55 Hz recording (written into a temporary
  directory, not into `tests/data/`, and not `55Hz.cfg`, which is untracked and
  4.7 MB):
  - Currents first in the channel list, voltages later. The frequency is
    55.000 in every row, the header names the voltage group.
  - `--frequency-from IA` measures from IA.
  - With the voltages zero for the first half, the `f from` column shows the
    change from `I+` to `V+`.
- `trend_through_the_fault` (fault_2013): check that the frequencies stay
  50.000. The breaker-open row still shows `-`.

### 4. Docs

- `docs/src/commands/trend.md`, `docs/src/analysis/index.md` (frequency
  section: the image, positive sequence, tracking, the source order),
  `spec.md` `trend` section, README `trend`.
- `todo.md`: the two items above.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`. `list_content_filters` needs `55Hz.cfg` out of
`tests/data/` to pass.
