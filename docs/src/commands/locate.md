# `locate`: Fault location

Distance to the fault from the line end where the recording was made, with the
reactance method and with Takagi, so the line patrol knows where to look.

```
sprcmtrd locate [OPTIONS] --z1 <Z1> --length <LENGTH> <FILE>
```

```sh
sprcmtrd locate tests/data/reclose_1999_ascii.cfg --z1 0.03+j0.4 --length 40
```

A phase A to B fault on a 40 km line with 0.4 Ω/km reactance: the A-B loop
sees 11.7 Ω, about 29 km. Both methods agree, because this synthetic fault has
almost no fault resistance.

```
File      tests/data/reclose_1999_ascii.cfg
Channels  VA VB VC [kV] · IA IB IC [A]
Line      z1 0.0300+j0.4000 Ω/km, 40 km, primary
Fault     A-B at 0.000 s
Window    20.00 ms .. 39.00 ms (1 cycle, 20 samples, 50 Hz)
Loop      A-B: V 70.17kV, I 5.977kA, Z -1.234+j11.67 Ω

method     distance  of line  note
reactance  29.19 km  73.0 %
Takagi     28.81 km  72.0 %
```

- The fault is found as in [`fault`](fault.md); the loop follows from the
  faulted phases (A-N for a phase A to ground fault, A-B for A-B, A-B-N and
  A-B-C). `--loop BC` chooses another one.
- Line data per km on the primary side: `--z1` and `--z0` as `R+jX`
  (`0.03+j0.33`) or `MAG@ANGLE` (`0.33@85`), `--length` in km. `--z0` is
  needed for ground faults (residual compensation k0).
- The **reactance** method is exact for a bolted fault; with fault resistance
  and load flow it is off. **Takagi** uses the change of the current against
  the pre-fault cycle to remove the fault resistance. See
  [Analysis methods](../analysis/index.md#fault-location).
- The window is one cycle in the middle of the fault, at least one cycle
  after inception (no DC offset); `--at` and `--cycles` choose another.
- A distance below 0 is noted as "behind the relay", above the line length as
  "beyond the line end (or wrong line data)"; they are not clamped.
- Primary values only; `--secondary` is an error. `--json` for scripts.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--z1 <Z1>` | Positive sequence impedance of the line in Ω/km primary: `R+jX` or `MAG@ANGLE` |
| `--z0 <Z0>` | Zero sequence impedance of the line in Ω/km primary (needed for ground faults) |
| `--length <LENGTH>` | Length of the line in km |
| `--loop <FAULT_LOOP>` | Loop to measure (`AN`, `BC`, ...). Default: from the faulted phases |
| `--at <AT>` | End of the measurement window in ms relative to the trigger. Default: the middle of the fault |
| `--cycles <CYCLES>` | Measurement window length in cycles of the fundamental frequency [default: 1] |
| `--from <FROM>` | Start of the fault search in ms relative to the trigger |
| `--to <TO>` | End of the fault search in ms relative to the trigger |
| `--voltage <VOLTAGE>` | Voltage channels of phases A, B, C to neutral (indices or ids). Default: found by unit |
| `--current <CURRENT>` | Current channels of phases A, B, C (indices or ids). Default: found by unit |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too, except
`--secondary`. `sprcmtrd locate --help` shows this list with examples.
