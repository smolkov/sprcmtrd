# `impedance`: Fault loop impedance

Apparent impedance of the six fault loops, as a distance relay sees it. Phase to ground V / (I + k0·3I0), phase to phase (V1 − V2) / (I1 − I2), from the phasors of a voltage and a current A/B/C group (found by unit, or --voltage / --current). With --trend, R and X per loop over time.

```
sprcmtrd impedance [OPTIONS] <FILE>
```

```sh
sprcmtrd impedance tests/data/fault_2013.cfg --at 40
```

During the phase B to ground fault, the B-N loop drops from the load
impedance (about 200 Ω) to about 5 Ω; the healthy loops stay at the load.

```
File          tests/data/fault_2013.cfg
Window        20.50 ms .. 40.00 ms (1 cycle, 40 samples, 50 Hz)
Channels      VA VB VC [kV] · IA IB IC [A]
Compensation  none (k0 = 0; give the line's k0 with --k0)
Values        primary Ω

loop  V [kV]  I [A]   R [Ω]  X [Ω]    |Z| [Ω]  φ
A-N   63.50   318.2   199.6  0.000    199.6    0.0°
B-N   17.68   3.458k  5.108  -204.5m  5.112    -2.3°
C-N   63.50   318.2   199.6  0.000    199.6    0.0°
A-B   73.94   3.617k  14.23  -14.67   20.44    -45.9°
B-C   73.94   3.638k  15.22  13.47    20.32    41.5°
C-A   110.0   551.1   199.6  0.000    199.6    0.0°
```

- The six loops a distance relay measures: A-N, B-N, C-N as V / (I + k0·3I0)
  and A-B, B-C, C-A as (V₁ − V₂) / (I₁ − I₂), in Ω primary (`--secondary` for
  secondary). Give the line's residual compensation with `--k0 0.8@-5`.
- Voltage and current channels are found by unit; `--voltage VA,VB,VC
  --current IA,IB,IC` chooses them. `--loops BN,BC` limits the loops.
- `--trend` gives R and X per loop over time (`--from`, `--to`, `--step`), to
  compare with the relay's zones; `--format csv` or `json` for other tools.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--at <AT>` | End of the window in ms relative to the trigger (without --trend) [default: 0] |
| `--cycles <CYCLES>` | Window length in cycles of the fundamental frequency [default: 1] |
| `--voltage <VOLTAGE>` | Voltage channels of phases A, B, C (indices or ids). Default: found by unit |
| `--current <CURRENT>` | Current channels of phases A, B, C (indices or ids). Default: found by unit |
| `--k0 <K0>` | Residual compensation k0 = (Z0 − Z1) / (3·Z1) as magnitude, optionally with angle in degrees (`0.8` or `0.8@-5`). Default 0: no compensation [default: 0] |
| `--loops <LOOPS>` | Loops to show (`AN,BN,CN,AB,BC,CA`). Default: all six |
| `--trend` | R and X of each loop over time instead of at one moment |
| `--from <FROM>` | Start of the time range in ms relative to the trigger (with --trend) |
| `--to <TO>` | End of the time range in ms relative to the trigger (with --trend) |
| `--step <STEP>` | Distance between rows with --trend: cycles (`1`) or a time (`10ms`) [default: 1] |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--format <FORMAT>` | Output format. Values: `text` (aligned table), `csv` (comma separated values with a header row), `json` (one JSON object) [default: text] |

The [global options](index.md#global-options) apply too. `sprcmtrd impedance --help`
shows this list with examples.
