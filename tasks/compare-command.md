# `compare` command

- **Issue:** none, from `todo.md` (signal analysis ideas)
- **Status:** built as planned. With `--pair`, only the given pairs are compared (no extra pairs by id); `--channels` filters either list. The JSON has `window_a` / `window_b` instead of one `window`, and the differential's channel ids. Later `--json` became `--format text|csv|json` with `--trend` and `--plot` (see `tasks/compare-trend-plot.md`).

## Goal

`sprcmtrd compare <A> <B>`: two recordings of the same event side by side.
Typical cases:

- **Both ends of a line**, recorded by the relays at each end. With synchronized
  clocks, the currents show where the fault is: a fault on the line is fed
  from both ends, so the differential current is large. A fault outside the line
  passes through, so the differential current is small.
- **Relay and fault recorder** at one bay: the same signals should agree.
  This checks ratios, wiring and polarity.
- **Before and after a change**, for example a test before and after new
  settings.

The comparison uses phasors at one moment, as `phasors` does. Each recording
gets its own DFT, so different sample rates need no resampling.

## Background

Each recording's phasor angles are referenced to its own time axis (the
DFT uses t from the recording's trigger). Suppose B's axis runs `d` seconds
behind A's: t_A = t_B + d. Then B's phasor in A's frame is
P_B · e^(−jω·d). After that rotation, the angles of A and B can be compared
directly. Comparing angles only makes sense when the alignment is right.
With unsynchronized clocks, a few ms of error is already tens of degrees
(1 ms = 18° at 50 Hz).

Differential current per phase, with both currents measured into the line
(the usual convention): I_diff = |I_A + I_B|, I_restraint = (|I_A| + |I_B|) / 2.
Load or an external fault gives I_diff ≈ 0 (plus the line charging current).
A fault on the line gives I_diff ≈ the fault current.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Alignment | `--align time` (default): by the absolute trigger times, for synchronized relays (GPS / PTP). `--align trigger`: both triggers at 0. `--align fault`: the fault inception of A on that of B (as `fault` finds it), for unsynchronized clocks. `--shift <ms>` moves B on top | Only trigger alignment: wrong for two ends whose relays trigger at different moments |
| Recordings that don't overlap | With `--align time`, an error when the two time spans have no common part, suggesting `--align trigger` or `fault` | Silently fall back to trigger |
| Channel pairs | By equal id (case-insensitive), analog only. `--channels IA,IB` limits the list. `--pair IA=IL1,IB=IL2` maps different names (A's id = B's id). No common channel: an error naming `--pair` | Pair by position (channel 1 with 1): wrong whenever the channel lists differ |
| Units | V/kV and A/kA are scaled to the same unit (shown in A's unit). Other units must be equal, otherwise that pair is an error | Compare raw values |
| Moment | `--at <ms>` in A's time (default 0, as in `phasors`) and `--cycles` (default 1); the window of B ends at the same aligned instant | Default to the fault window of A: hidden magic, and not every recording has a fault |
| Table | `channel A | channel B | unit | A | B | B/A | angle A | angle B | B − A`. Magnitudes are RMS. Angles are relative to A's first pair (`--reference`), and B's angles are rotated into A's time frame | Only the differences: you can't tell then whether a ratio of 1.000 means 0 A on both |
| Differential | `--differential`: per phase from the current groups of A and B (found by unit, as in `fault`, or `--current-a` / `--current-b`): I_A, I_B, I_diff, I_restraint, I_diff / I_restraint. `--invert-b` when B is measured out of the line | Always shown when both have a current group: confusing for the relay-vs-recorder case |
| Over time | Not in v1: one moment only. Cycle by cycle (`--trend`, as in `impedance`) can come later, also for I_diff | Include `--trend` now: it about doubles the work |
| Overlaid waveform plots | Not in v1; `view` for each recording works today | An overlay of A and B in one terminal plot |
| Frequency | The line frequency of A; an error when B's differs, unless `--frequency` is given | Each its own |
| Side | Global `--secondary` applies to both; primary by default | |
| Output | Text and `--json` | CSV: for the one-moment table, JSON is enough for scripts |

Decided by existing behaviour, no need to confirm:

- Errors: file errors exit 3; channel and alignment errors exit 1; bad option
  values exit 2.
- A missing value in a window, or a window before the start or past the end
  of B: that row shows "-" and there is a note, as in `phasors`.

## Output

```
A          tests/data/fault_2013.cfg (SUBSTATION B)
B          end_b.cfg (SUBSTATION C)
Alignment  absolute time: B's trigger is +12.0 ms after A's
Window     20.50 ms .. 40.00 ms in A (1 cycle, 50 Hz); B ends at 28.00 ms in its own time
Values     primary; RMS, angles relative to A IA

A    B    unit  A       B       B/A    angle A   angle B   B − A
IA   IA   A     318.2   318.2   1.000  0.0°      180.0°    180.0°
IB   IB   A     3.458k  1.200k  0.347  -117.7°   -120.3°   -2.6°
…

Differential (both measured into the line)
phase  I A     I B     I diff  I restraint  diff / restraint
A      318.2   318.2   0.000   318.2        0.0 %
B      3.458k  1.200k  4.650k  2.329k       199.7 %
```

(Numbers illustrative.) JSON: `a` and `b` (`file`, `station`), `alignment`
(`mode`, `offset_s`, `shift_s`), `window`, `side`, `frequency`, `pairs[]`
(`a`, `b`, `unit`, `a_rms`, `b_rms`, `ratio`, `a_angle`, `b_angle`,
`angle_difference`), `differential` (`phases[]` with `phase`, `a`, `b`,
`diff`, `restraint`, `percent`) when asked for.

## Implementation plan

### 1. Library: `src/analysis/compare.rs` (new)

- `pub enum Align { Time, Trigger, Fault }`.
- `pub fn offset(a: &Comtrade, b: &Comtrade, align, shift) -> Result<f64, AlignError>`:
  d, B's time = A's time − d. `Fault` uses `fault::analyse` on both current
  groups; `AlignError` covers no overlap and no fault in A or B.
- `pub fn pairs(a, b, channels, explicit) -> Result<Vec<(usize, usize)>, PairError>`.
- `pub fn rotate(p: Complex, d: f64, frequency) -> Complex`: B's phasor in A's
  time frame.
- `pub struct Differential { a, b, diff, restraint }` and `pub fn differential(ia, ib, invert)`.
- Unit tests with two synthetic recordings built with
  `Comtrade::from_readers`: the same 50 Hz signal with B's trigger 12 ms later
  (absolute) and a different sample rate. After rotation, the angle difference
  is 0 with `Time`, and 12 ms · 18°/ms with `Trigger`. Also: `Fault` alignment
  finds a known shift, no overlap gives an error, pairing by id / `--pair` /
  case-insensitive, and the differential for through current (≈ 0) and an
  internal fault.

### 2. Command: `src/args.rs`, `src/command/compare.rs` (new)

- `Command::Compare(CompareArgs)`: `a`, `b`, `--at`, `--cycles`, `--align`,
  `--shift`, `--channels`, `--pair`, `--reference`, `--differential`,
  `--current-a`, `--current-b`, `--invert-b`, `--frequency`, `--json`.
- Shell completion of both file arguments, as for the other commands.

### 3. Tests: `tests/cli.rs`

- `fault_2013.cfg` against `test_ascii.cff` (the same data in another
  format): all ratios 1.000, angle differences 0.0°.
- Two-ended pair: the test writes the remote end as a small ASCII `.cfg` /
  `.dat` into a temporary directory (not into `tests/data/`), with currents
  that look like a through fault and like an internal fault, and B's trigger
  shifted. Checks: I_diff near 0 for the through fault, large for the
  internal one; `--invert-b`; `--align trigger` shows the shift as an angle.
- Errors: no common channel names `--pair`, no overlap with `--align time`,
  a different frequency, bad `--pair` syntax (exit 2). JSON fields.

### 4. Docs

- `spec.md`: `compare` in the command list, a `### compare command` section
  and the structure lines.
- `docs/src/commands/compare.md` + `SUMMARY.md` + commands index; README
  command list and a short section; `docs/src/analysis/index.md`: alignment,
  the angle rotation and the differential current.
- `todo.md`: tick `compare` (and add `--trend` / overlay plots as follow-ups);
  `agent.md`: the new files in the architecture lines.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`.
