# `trend`: clearer names for the symmetrical components

- **Issue:** [smo/sprcmtrd#1](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/1), "symmetrische Komponenten bei sprcmtrd trend besser anzeigen"
- **Status:** done, released in 0.7.0. The JSON `label` stays without the space of `G1 `. The header unit test lives in `command/trend.rs`.

## Problem

`trend` names the sequence columns of a three-phase group with the common
start of the group's channel ids plus `1`, `2`, `0` and `2/1`. For the ids
`IA IB IC` this gives `I1 I2 I0`, which works. For the ids that relays often
use, `iL1 iL2 iL3` (in `55Hz.cfg`), the label is `iL`, and the positive and
negative sequence columns become `iL1` and `iL2`, the same names as the
phase channels:

```
time  f [Hz]  iL1 [A]  iL2 [A]  iL3 [A]  …  iL1 [A]  iL2 [A]  iL0 [A]  iL2/1  uL1 [V] …
```

The CSV is worse: two columns are both named `iL1_A` and two `iL2_A`, so
pandas or a spreadsheet renames or overwrites one of them.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Text headers | As the issue suggests: `iL+ [A]`, `iL- [A]`, `iL0 [A]`, `iL-/+` (`I+ I- I0 I-/+` for IA IB IC). `+`, `-` and `0` can't clash with a phase name | `iL₁ iL₂ iL₀` (subscripts): still looks like `iL1`, and is hard to type in `grep` |
| CSV headers | Words, so they stay valid names in pandas, R and Excel formulas: `iL_pos_A`, `iL_neg_A`, `iL_zero_A`, `iL_unbalance_pct` | `iL+_A`, `iL-_A`: R and some tools turn `+` / `-` into `.` |
| JSON | Unchanged: `sequences` already has `positive`, `negative`, `zero`, `unbalance` | |
| Label without a common start | `G1+`, `G1-`, `G10`: `G10` reads like group 10, so use `G1 +`, `G1 -`, `G1 0` (with a space) only for the `G` labels | Always with a space: wider columns |
| Other commands | `phasors` already writes `zero`, `positive`, `negative`. `impedance` and `power` show no sequence columns. Nothing else changes | |
| Compatibility | The CSV header changes. That breaks scripts that read `I1_A` by name. Note it in the changelog / README | Keep the old CSV names: the clash stays |

## Also found

`sprcmtrd trend 55Hz.cfg | head` (or quitting `less` early) ends with a
panic, "failed printing to stdout: Broken pipe". That's a separate bug, and
probably in every command that uses `print!`. I'd put it in `todo.md` as a
task of its own, not fix it here.

## Implementation plan

### 1. `src/command/trend.rs`

- `Table::headers`: the new text and CSV names for the group columns.
- `label`: unchanged (the common start), plus the space rule for `G<n>`.

### 2. Help text: `src/args.rs`

- `Command::Trend` doc: "Group columns: + positive, - negative, 0 zero
  sequence, -/+ unbalance in %."

### 3. Tests

- Unit test of the headers for `IA IB IC` (`I+ [A]`, …, `I_pos_A`, …) and for
  a group without a common start (`G1 +`).
- CLI test: a synthetic recording with the ids `iL1 iL2 iL3`, written into a
  temporary directory (not into `tests/data/`, and not `55Hz.cfg`, which is
  untracked and 4.7 MB). The text header has `iL+ [A]` and no second
  `iL1 [A]`; the CSV header has no duplicate names.
- Update the existing tests that expect `I1 [A]` / `I2/1` / `I1_A`.

### 4. Docs

- `docs/src/commands/trend.md` (example and column description),
  `spec.md` `trend` section (columns, CSV header), README `trend` example.
- `todo.md`: the broken-pipe item; agent.md needs no change.

### 5. Branch and checks

- Build on the issue branch `1-symmetrische-…`, first moved forward to
  `main` (it has no commits of its own).
- `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
  `mdbook build docs`.
