# `locate` command

- **Issue:** none, from `todo.md` (signal analysis ideas)
- **Status:** done, released in 0.7.0. The measurement window is chosen in `analysis::locate::window` from the `Fault` fields, so `analysis::fault` needed no change; `parse_k0` and the new `--z1` / `--z0` parser share `parse_polar`. Not checked against a real recording with known line data yet.

## Goal

`sprcmtrd locate <FILE> --z1 … --z0 … --length KM`: single-ended fault
location from the recording of one line end. It gives the distance to the
fault in km and in % of the line, with the reactance method and with Takagi,
so the line patrol knows where to look and the two results can be compared.

It builds on `fault` (inception, faulted phases, pre-fault and fault windows)
and on `impedance` (loop quantities with residual compensation).

## Background

For the faulted loop with voltage V and current I (ground loops compensated:
I + k0·3I0, k0 = (Z0 − Z1) / (3·Z1)) and the positive sequence impedance per
km z1:

- **Reactance method:** d = Im(V / I) / Im(z1). It ignores the fault
  resistance Rf, but Rf seen through a current that differs in phase from the
  fault current (load flow, infeed from the other end) adds a reactance error.
- **Takagi:** uses the superimposed current ΔI = I(fault) − I(pre-fault),
  which is close in phase to the fault current, to remove Rf:
  d = Im(V · ΔI*) / Im(z1 · I · ΔI*). It's exact for a homogeneous system
  (source and line impedance angles equal) and better than the reactance method
  under load. For ground loops ΔI is the faulted phase's ΔI.

Both need the voltages: `fault_1999` (only VA) can't be located.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Line data | `--z1 <Z>` and `--z0 <Z>` in Ω/km primary, `--length <km>`. A value is `R+jX` (`0.03+j0.33`) or `MAG@ANGLE` (`0.33@85`), like `--k0` | The total line impedance in Ω (`--z1 9.9@85`, length only for km). Or the todo's `--line-impedance Z1,Z0`: one option, harder to read |
| `--z0` | Required for ground faults, not needed for phase faults; a ground fault without it is an error that names the option | Default k0 = 0: silently wrong distances for ground faults |
| Loop | From the fault type of `analysis::fault`: A-N → A-N loop; A-B and A-B-N → A-B; A-B-C → A-B (any phase-phase loop sees Z1). `--loop` overrides | All loops of the faulted phases, one row each: more to read, and the same distance for A-B-C |
| Measurement window | One cycle ending in the middle of the fault, at least one cycle after inception (the DC offset and the transition have decayed); for faults shorter than two cycles the `fault` window. `--at <ms>` and `--cycles` override | The `fault` window (a quarter cycle after inception): includes DC offset, which shifts the DFT phasor |
| Pre-fault for Takagi | The `fault` pre-fault window (the cycle before inception) | A window chosen by the user: `--pre <ms>` can come later |
| Values | Always primary: the line data is primary. `--secondary` is an error ("the line data is primary") | Convert the line data to secondary with the CT/VT ratios |
| Results outside the line | Shown as computed, with a note: below 0 "behind the relay", above 100 % "beyond the line end (or wrong line data)" | Clamp to 0 … 100 %: hides wrong data |
| Fault resistance | Not shown in v1 | Rf from Takagi (needs an assumption about the remote infeed) |
| Search range, channels | `--from` / `--to` and `--current` / `--voltage` as in `fault`; `--frequency`; `--json` | |

Decided by existing behaviour, no need to confirm:

- Channels: `VoltageCurrent::new` (both groups, as in `impedance`), units
  scaled to V and A.
- No fault found: "No fault found." (exit 0), like `fault`. No voltage
  group: error naming `--voltage` / `--current` (exit 1). Invalid line data
  (`--length` ≤ 0, Im(z1) ≤ 0): command line error (exit 2).
- Missing values in a window: no result, a note, like `trend`.

## Output

```
File        tests/data/reclose_1999_ascii.cfg
Channels    VA VB VC [kV] · IA IB IC [A]
Line        z1 0.0300+j0.4000 Ω/km, z0 …, 30.0 km, k0 …
Fault       A-B at +0.000 s
Window      +0.020 s .. +0.039 s (1 cycle, 20 samples, 50 Hz)
Loop        A-B: V 70.17 kV, I 6.058 kA, Z −1.286 + j11.51 Ω

method     distance  of line
reactance  28.8 km   95.9 %
Takagi     …         …
```

(Numbers illustrative.) JSON with `--json`: `file`, `frequency`, `voltage`,
`current`, `line` (`z1`, `z0`, `length_km`, `k0`), `found`, and when found
`fault_type`, `inception_s`, `window` (`from_s`, `to_s`, `samples`), `loop`
(`name`, `v`, `i`, `z` as `{re, im}`), `results[]` (`method`, `distance_km`,
`percent`, `note`).

## Implementation plan

### 1. Library: `src/analysis/locate.rs` (new)

- `pub fn reactance(v: Complex, i: Complex, z1: Complex) -> Option<f64>`
  (distance in km).
- `pub fn takagi(v: Complex, i: Complex, delta: Complex, z1: Complex) -> Option<f64>`.
- `pub fn fault_loop(kind: FaultType) -> Loop`, using `impedance::Loop` and
  `loop_quantities`.
- `pub fn k0(z1, z0) -> Complex`.
- `analysis::fault`: make the window choice reusable: `Fault` gets
  `fault_end()` (interruption or end of range) so `locate` can place its
  window; nothing else changes there.
- Unit tests with a **synthetic two-source line model**: sources S and R
  behind the line, a fault at m with Rf, pre-fault load flow. The voltages and
  currents at S are computed exactly for each fault type and turned into a
  recording (helper like `fault::tests::recording`). Checks:
  - a bolted fault: both methods within 0.5 %;
  - Rf with load flow: Takagi within 1 %, reactance visibly off;
  - a phase-phase and a three-phase fault;
  - a fault beyond the line end gives > 100 %;
  - no fault current (ΔI = 0) gives no Takagi result.

### 2. Command: `src/args.rs`, `src/command/locate.rs` (new)

- `Command::Locate(LocateArgs)`: `file`, `--z1`, `--z0`, `--length`,
  `--loop`, `--at`, `--cycles`, `--from`, `--to`, `--voltage`, `--current`,
  `--frequency`, `--json`.
- Value parser for `R+jX` / `MAG@ANGLE`, shared with `--k0` (`parse_k0`
  becomes a general complex parser; `--k0` keeps accepting what it does today).

### 3. Tests: `tests/cli.rs`

- `reclose_1999_ascii` (A-B, X ≈ 11.5 Ω): with `--z1 0.03+j0.4 --length 40`,
  the reactance distance ≈ 28.8 km; Takagi is shown. JSON fields.
- `fault_2013` (B-N, almost purely resistive loop in the synthetic data):
  needs `--z0`; with it, a result near 0 and no crash.
- Errors: `fault_1999` (no voltages) exit 1, missing `--z0` for a ground
  fault exit 1, `--secondary` exit 1, `--length 0` exit 2, bad `--z1` syntax
  exit 2.
- No new files in `tests/data/`. Your `long1_t1_anstatt_t2` is checked by
  hand only. I don't know its line data, so I can only check that the result
  is plausible.

### 4. Docs

- `spec.md`: `locate` in the command list and a `### locate command` section.
- `docs/src/commands/locate.md` + `SUMMARY.md` + commands index; README
  command list and a short section; `docs/src/analysis/index.md`: the two
  methods and their limits. `mdbook build docs`.
- `todo.md`: tick `locate`; `agent.md`: `locate.rs` in the architecture lines.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`.
