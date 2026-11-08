# `dc-offset`: DC offset of the fault current

A fault current starts with a DC part that decays with the time constant
τ = L/R of the fault loop. It drives current transformers into saturation
(the longer τ, the more) and the breaker has to interrupt it: so τ and the DC
part matter for CT dimensioning and breaker duty.

```
sprcmtrd dc-offset [OPTIONS] <FILE>
```

```sh
sprcmtrd dc-offset tests/data/fault_2013.cfg
```

Phase B carries a DC part of 3 kA, 60 % of the AC peak, decaying with
τ = 40 ms (X/R 12.6 at 50 Hz). Phases A and C carry load only.

```
File      tests/data/fault_2013.cfg
Currents  IA IB IC [A]
Values    primary
Fault     B-N at 0.000 s, interrupted at +0.100 s
Fit       0.00 ms .. 59.50 ms (3 cycles, 120 samples, 50 Hz)

phase           DC [A]  DC %    τ        X/R   AC RMS [A]  peak [A]  peak at   fit error
A               0.000   0.0 %   -        -     318.2       450.0     +0.005 s  0.0 %
B      faulted  3.000k  60.0 %  40.0 ms  12.6  3.536k      7.244k    +0.011 s  0.0 %
C               0.000   0.0 %   -        -     318.2       449.4     +0.008 s  0.0 %

DC at inception in % of the AC peak; τ only for a DC part of at least 5 %.
```

- The fault is found as in [`fault`](fault.md): inception, faulted phases and
  the interruption of the current.
- From inception over `--cycles` cycles (default 3, cut at the
  interruption), each phase current is fitted as a sine plus D·e^(−t/τ); see
  [DC offset](../analysis/index.md#dc-offset).
- **DC** is D at inception with its sign, in A and in % of the AC peak.
  **τ** and **X/R** = ω·τ are shown only for a DC part of at least 5 % of the
  AC peak, since the τ of a tiny DC term is noise. `> 1 s` means the DC part
  hardly decays within the window.
- **AC RMS** is the RMS of the fitted sine, **peak** the largest instantaneous
  value from inception to the interruption (AC and DC together).
- **Fit error** is the RMS of the residual in % of the AC peak. A few % or
  more means the model doesn't fit: a saturated CT, an arc, or a fault
  shorter than the window.
- `--json` for scripts; currents in A, times in s.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--cycles <CYCLES>` | Fit window from inception in cycles of the fundamental frequency (at least 1). Default 3 |
| `--current <CURRENT>` | Current channels of phases A, B, C (indices or ids). Default: found by unit |
| `--from <FROM>` | Start of the fault search in ms relative to the trigger |
| `--to <TO>` | End of the fault search in ms relative to the trigger |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd dc-offset
--help` shows this list with examples.
