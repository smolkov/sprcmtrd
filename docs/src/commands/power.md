# `power`: Active, reactive power and power factor

Active, reactive and apparent power and power factor of a three-phase circuit, per phase and in total. S = V·I* per phase from the phasors of a voltage and a current A/B/C group (found by unit, or --voltage / --current; phase-to-neutral voltages). Positive P flows the way the currents are measured, positive Q is inductive. With --trend, the power over time.

```
sprcmtrd power [OPTIONS] <FILE>
```

```sh
sprcmtrd power tests/data/fault_2013.cfg --at 40
```

Before the fault the line carries 60.6 MW at power factor 1; during the phase B
to ground fault phase B carries the fault current.

```
File        tests/data/fault_2013.cfg
Window      20.50 ms .. 40.00 ms (1 cycle, 40 samples, 50 Hz)
Channels    VA VB VC [kV] · IA IB IC [A]
Values      primary W, var, VA
Convention  P > 0 flows as the currents are measured, Q > 0 inductive

phase  V [kV]  I [A]   P [W]   Q [var]  S [VA]  PF
A      63.50   318.2   20.21M  0.000    20.21M  1.000
B      17.68   3.458k  61.09M  -2.445M  61.13M  0.999 lead
C      63.50   318.2   20.20M  0.000    20.20M  1.000
total                  101.5M  -2.445M  101.5M  1.000 lead

Flow: forward, 101.5M W as the currents are measured
```

- Per phase S = V·I* (RMS), P = Re S in W, Q = Im S in var, |S| in VA; the total
  is the sum of the phases. PF = P / |S| with `lag` (Q > 0) or `lead` (Q < 0).
  Values are primary (`--secondary` for secondary).
- The `Flow` line gives the direction of the active power: `forward` in the
  direction the currents are measured, `reverse` against it.
- Voltage and current channels are found by unit; `--voltage VA,VB,VC
  --current IA,IB,IC` chooses them. The voltages must be phase to neutral.
- `--trend` gives the total power over time (`--from`, `--to`, `--step`);
  `--phases` adds P and Q per phase. `--format csv` always has every phase.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--at <AT>` | End of the window in ms relative to the trigger (without --trend) [default: 0] |
| `--cycles <CYCLES>` | Window length in cycles of the fundamental frequency [default: 1] |
| `--voltage <VOLTAGE>` | Voltage channels of phases A, B, C to neutral (indices or ids). Default: found by unit |
| `--current <CURRENT>` | Current channels of phases A, B, C (indices or ids). Default: found by unit |
| `--trend` | Power over time instead of at one moment |
| `--phases` | With --trend in text: P and Q of each phase too, not only the totals |
| `--from <FROM>` | Start of the time range in ms relative to the trigger (with --trend) |
| `--to <TO>` | End of the time range in ms relative to the trigger (with --trend) |
| `--step <STEP>` | Distance between rows with --trend: cycles (`1`) or a time (`10ms`) [default: 1] |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--format <FORMAT>` | Output format. Values: `text` (aligned table), `csv` (comma separated values with a header row), `json` (one JSON object) [default: text] |

The [global options](index.md#global-options) apply too. `sprcmtrd power --help`
shows this list with examples.
