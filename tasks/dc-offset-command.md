# `dc-offset` command

- **Issue:** none, from `todo.md` (signal analysis ideas)
- **Status:** built as planned. The current group helper of `command/fault.rs` is shared with `pub(super)`. `tau_above_limit` was added to the JSON so "> 1 s" is visible there too.

## Goal

`sprcmtrd dc-offset <FILE>`: the decaying DC component of the fault current
per phase: its value at inception, the decay time constant τ, the X/R ratio
that follows from it and the asymmetric peak. These matter for CT
dimensioning (transient saturation grows with τ) and for breaker duty (the
DC part at contact separation).

It builds on `fault` for the inception, the faulted phases and the
interruption.

## Background

After inception, the current of an R-L circuit is a sine plus a decaying DC term:

i(t) = a·sin(ωt) + b·cos(ωt) + D·e^(−t/τ),  t from inception

τ = L/R, so X/R = ω·τ. For a fixed τ the model is linear in a, b, D, so τ is
found by a one-dimensional search (a coarse grid, then golden section
refinement): for each τ, solve the 3×3 least squares problem and keep the τ
with the smallest residual. No new dependency.

A check on the samples with a prototype of this fit:

| Recording | phase | D at inception | AC peak | τ | residual |
|---|---|---|---|---|---|
| `fault_2013` | B | 3.00 kA | 5.00 kA | 40.0 ms | 0.43 A |
| `reclose_1999_ascii` | A / B | +2.50 / −2.50 kA | 4.20 kA | 30.0 ms | 0.5 A |
| `long1_t1_anstatt_t2` | A | 36 A | 3.33 kA | not meaningful (1 % DC) | 16 A |

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Phases | The three phase currents of the current group, as in `fault`; faulted phases marked. `--current` picks the group | Any analog channel via `--channels` (also useful for a transformer inrush, but that isn't an R-L decay) |
| Fit window | From inception, 3 cycles (`--cycles`), cut at the interruption. At least 1 cycle, otherwise no fit | Start a quarter cycle after inception (skips the arc transition, but loses the largest DC part) |
| Small DC | Below 5 % of the AC peak, show D but give no τ ("no significant DC"): τ of a tiny DC term is noise (see `long1`) | Always show τ |
| τ search range | 1 ms … 1 s; at the 1 s limit show "> 1 s" (almost no decay within the window) | Unbounded |
| Shown values | DC at inception (A and % of AC peak), τ (ms), X/R, AC RMS, measured peak and its time, the fit residual in % of the AC peak (a bad fit is visible) | Also DC at a given time (`--at`, e.g. the contact separation): could come later |
| Unfaulted phases | Shown too, usually "no significant DC" | Faulted phases only |
| Search range, options | `--from` / `--to`, `--current`, `--frequency`, `--json`, global `--secondary` as in `fault` | |

Decided by existing behaviour, no need to confirm:

- No fault found: "No fault found." (exit 0), like `fault`; no current group:
  error naming `--current` (exit 1).
- A missing value in the fit window: that sample is left out of the fit.

## Output

```
File       tests/data/fault_2013.cfg
Currents   IA IB IC [A]
Values     primary
Fault      B-N at 0.000 s, interrupted at +0.100 s
Fit        0.000 s .. +0.060 s (3 cycles, 120 samples, 50 Hz)

phase           DC [A]  DC %   τ        X/R   AC RMS [A]  peak [A]  peak at   fit error
A               0.000   0.0 %  -        -     318.2       450.0     +0.005 s  0.0 %
B      faulted  3.001k  60.0 % 40.0 ms  12.6  3.536k      7.244k    +0.011 s  0.0 %
C               0.000   0.0 %  -        -     318.2       449.4     +0.008 s  0.0 %

τ only for phases with a DC part of at least 5 % of the AC peak.
```

(Numbers from the prototype.) JSON with `--json`: `file`, `frequency`,
`current`, `found`, and when found `fault_type`, `inception_s`,
`interruption_s`, `window` (`from_s`, `to_s`, `samples`), `phases[]`
(`phase`, `channel`, `faulted`, `dc`, `dc_percent`, `tau_s`, `x_over_r`,
`ac_rms`, `peak`, `peak_s`, `fit_error_percent`).

## Implementation plan

### 1. Library: `src/analysis/dc_offset.rs` (new)

- `pub struct Decay { dc: f64, tau: Option<f64>, ac_peak: f64, residual: f64 }`.
- `pub fn fit(time: &[f64], values: &[Option<f64>], t0: f64, frequency: f64) -> Option<Decay>`:
  grid and golden section over τ, 3×3 least squares per τ (normal equations,
  solved with Gaussian elimination).
- `pub fn analyse(currents, time, fault: &Fault, cycles, frequency, side)`:
  the window from `Fault` (inception, interruption) and one `Decay` per
  phase, with the peak from the fault data.
- Unit tests on synthetic signals: known D and τ are found (within 1 %),
  also with a phase angle, noise and a missing sample; a pure sine gives no
  τ; a nearly constant DC gives "> 1 s"; too few samples give `None`.

### 2. Command: `src/args.rs`, `src/command/dc_offset.rs` (new)

- `Command::DcOffset(DcOffsetArgs)`: `file`, `--cycles` (default 3),
  `--from`, `--to`, `--current`, `--frequency`, `--json`.

### 3. Tests: `tests/cli.rs`

- `fault_2013`: phase B τ ≈ 40 ms, DC ≈ 3.0 kA; A and C without τ.
- `reclose_1999_ascii`: A and B τ ≈ 30 ms with opposite signs. JSON fields.
- `fault_1999` (no DC): no τ. `--cycles 0` exit 2.
- No new files in `tests/data/`.

### 4. Docs

- `spec.md`: `dc-offset` in the command list and a `### dc-offset command` section.
- `docs/src/commands/dc-offset.md` + `SUMMARY.md` + commands index; README
  command list and a short section; `docs/src/analysis/index.md`: the fit
  and its limits (one exponential, a CT that saturates distorts the fit).
- `todo.md`: tick `dc-offset`; `agent.md`: the new files in the architecture lines.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`.
