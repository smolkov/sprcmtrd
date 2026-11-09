# `fault` command

- **Issue:** none, from `todo.md` (signal analysis ideas)
- **Status:** done, released in 0.7.0. With three additions: the fault window moves past missing values, an interruption shortly before the end of the recording counts (at least a quarter cycle), and a note when no current flowed before inception (energized line). Without options, all channels matching a role's name are candidates and the earliest change counts (`fault_2013` has `START_A`/`B`/`C`).

## Goal

`sprcmtrd fault <FILE>`: a fault analysis summary of one recording, the first
thing a protection engineer wants to know:

- inception: the first sample where the current departs from the pre-fault cycle,
- faulted phases (A-N, B-C, A-B-N, A-B-C, …),
- fault duration until the current is interrupted,
- peak current (instantaneous, with DC offset) and the fault current (RMS of the
  fundamental) per phase,
- operate times next to the binary events: start → trip → breaker open →
  current interrupted.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Inception method | Superimposed current, sample by sample: Δi(n) = i(n) − i(n − N) with N samples per cycle, as relays detect a fault. Inception is the first sample of the first run of 3 samples with \|Δi\| above the threshold on any phase | Compare cycle RMS with the pre-fault cycle: simpler, but only to within a cycle |
| Inception threshold | 20 % of the largest pre-fault peak of the three phases, but at least 2 % of the largest absolute current in the recording (unloaded line before the fault) | A `--threshold` option in A; can be added later |
| Faulted phases | From the superimposed phasors ΔI = I(fault) − I(pre-fault), one cycle each: a phase is faulted when \|ΔI\| ≥ 50 % of the largest \|ΔI\|; ground (N) when 3·\|ΔI0\| ≥ 20 % of the largest \|ΔI\|. Works with currents only (`fault_1999` has only VA) | Also use voltage drops: needs all three voltages and doesn't help in the samples |
| Fault window | Pre-fault: the cycle ending one sample before inception. Fault: the cycle starting a quarter cycle after inception (skips the transition), cut at the interruption | The cycle starting at inception: includes the step |
| Interruption | The first sample after which all faulted phases stay below 5 % of their fault peak for one whole cycle; the duration ends at the last sample above it. Without one: "not interrupted within the recording" | Detect a return to load current too (fault cleared by another breaker): rare in recorder files, harder to tell from a load change |
| Several faults (reclose onto a fault) | Only the first fault in `--from` / `--to`; a later one is found with `--from` after the first | All faults in the recording: needs a "fault ended" state machine and a list output |
| Binary roles | Options `--start <CH>`, `--trip <CH>`, `--breaker <CH>`; without them, the first binary whose id contains `START` / `PICKUP`, `TRIP`, `CB` / `BREAKER` (case-insensitive). Breaker open is its change to 1, or to 0 when the id contains `CLOSE`. A role that isn't found is left out and a note names the option | Name patterns only (German recorders: `Anregung`, `AUS`, `LS-Stellung` collide, e.g. `LS-Stellung Aus` is a breaker position, not a trip) |
| Operate time reference | Times relative to inception (operate times), plus the time relative to the trigger | Relative to the start signal |
| Phase names | `A`, `B`, `C`, `N` in the output, whatever the channel phases are called (`L1`, `R`) — the groups already map them | The channel's own phase names |

Decided by existing behaviour, no need to confirm:

- Current channels as in `power` / `impedance`: `--current A,B,C`, default the
  first current three-phase group (unit A, kA, mA) — a new `find_current` next
  to `find_pair`, because `find_pair` needs a voltage group too. No current
  group is an error naming `--current` (exit 1).
- `--voltage A,B,C` optional: when given or found, the fault table also shows
  the voltage per phase during the fault and its drop from pre-fault.
- `--frequency` as in `phasors`; global `--secondary` selects secondary values.
- No fault found: "No fault found in …" and exit 0 (like "No changes" in `events`).
- A missing value (`99999`) in a window leaves that value out (`-`), like `trend`.

## Output

```
File        tests/data/fault_1999.cfg
Currents    A1 IA, A2 IB, A3 IC
Inception   +0.5 ms (2024-03-15 08:30:00.060500)
Fault       A-N
Duration    59.5 ms, current interrupted at +60.0 ms

phase  pre-fault  fault    ΔI       peak     at
A      282.8 A    2.828 kA 2.546 kA 4.01 kA  +5.5 ms
B      282.8 A    282.8 A  0.000 A  …
C      …
3I0    …

Operate times (from inception)
start             +0.0 ms   D1 START
trip              +20.0 ms  D2 TRIP
breaker open      +60.0 ms  D3 CB_OPEN
current interrupted +60.0 ms
```

(Numbers illustrative; layout via `format::key_values` and `format::table`.)
JSON with `--json`: `file`, `side`, `frequency`, `current`, `voltage`,
`inception` (`sample`, `time_s`, `absolute`), `fault_type`, `phases[]`
(`phase`, `faulted`, `pre_fault`, `fault`, `delta`, `peak`, `peak_time_s`),
`residual`, `interruption` (`time_s` or null), `duration_s`, `operate[]`
(`role`, `time_s`, `since_inception_s`, `index`, `id`).

## Implementation plan

### 1. Library: `src/analysis/fault.rs` (new)

- `pub fn inception(currents: [&AnalogChannel; 3], time, range, per_cycle, side) -> Option<usize>`
- `pub fn interruption(currents, faulted, time, from, per_cycle, side) -> Option<usize>`
- `pub fn classify(pre: [Complex; 3], fault: [Complex; 3]) -> FaultType` with
  `FaultType { phases: [bool; 3], ground: bool }` and `name()` (`A-N`, `B-C`, `A-B-N`, `A-B-C`).
- `pub fn peak(ch, range, time, side) -> Option<(usize, f64)>`
- `pub fn analyse(...) -> Option<Fault>` putting it together, reusing
  `phasor::Window` / `phasor::phasor` and `events::events` for the binary roles.
- `analysis::impedance::find_current(channels) -> Option<Group>`.
- Unit tests with synthetic recordings (a helper like `phasor::tests::recording`
  with a step at a given time): inception sample, each fault type (A-N, B-C,
  B-C-N, A-B-C), unloaded line before the fault, no fault, not interrupted,
  missing values.

### 2. Command: `src/args.rs`, `src/command/fault.rs` (new)

- `Command::Fault(FaultArgs)` with `file`, `--current`, `--voltage`, `--start`,
  `--trip`, `--breaker`, `--from`, `--to`, `--frequency`, `--json`.
- `impl FaultArgs { pub fn run(self, ctx) }` formats text and JSON; reuses
  `command::group`, `time_window`, `select_channels` (for the binary options).

### 3. Tests: `tests/cli.rs`

- `fault_1999` → A-N, `fault_2013` → B-N, `reclose_1999_ascii` → A-B with the
  operate times of its binaries; `--json` fields; `--current` with a voltage
  channel is an error; a recording without currents is an error; `--from`
  after the interruption finds no fault (or the reclose fault).
- No new files in `tests/data/` (the untracked `long1_t1_anstatt_t2.*` stays
  yours; I'll check the command against it by hand: expected L1-N).

### 4. Docs

- `spec.md`: `fault` in the command list and a `### fault command` section.
- `docs/src/commands/fault.md`, a line in `docs/src/SUMMARY.md`, the commands
  index, `README.md` command list; `mdbook build docs`.
- `todo.md`: tick `fault`; `agent.md`: `fault.rs` in the architecture lines.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`.
