# `trend`: Values over time

Values over time, cycle by cycle: fundamental magnitude per channel, symmetrical components per three-phase group and the system frequency. Each row is a window of --cycles cycles ending at that time, like the phasors command at one moment. Group columns: + positive, - negative, 0 zero sequence, -/+ unbalance in %.

```
sprcmtrd trend [OPTIONS] <FILE>
```

```sh
sprcmtrd trend tests/data/fault_2013.cfg --from -20 --to 60 --channels IA,IB,IC
```

The table shows the currents cycle by cycle: balanced before the trigger, then
phase B rising to 3.4 kA, with equal negative and zero sequence current.

```
File       tests/data/fault_2013.cfg
Window     1 cycle (40 samples) at 50 Hz, every 1 cycle (5 rows)
Reference  A1 IA
Values     primary, RMS of the fundamental

time       f [Hz]  IA [A]  IB [A]  IC [A]  I+ [A]  I- [A]  I0 [A]  I-/+
-20.00 ms  -       318.2   318.2   318.2   318.2   0.000   0.000   0.0 %
0.00 ms    50.000  318.2   347.4   318.2   327.8   11.08   11.08   3.4 %
20.00 ms   50.000  318.2   3.411k  318.2   1.349k  1.031k  1.031k  76.5 %
40.00 ms   50.000  318.2   3.458k  318.2   1.365k  1.047k  1.047k  76.7 %
60.00 ms   50.000  318.2   3.488k  318.2   1.375k  1.057k  1.057k  76.9 %
```

- One row per `--step` (default 1 cycle; `10ms` for a time), each the window
  of `--cycles` cycles ending there, like `phasors` at one moment.
- Columns: frequency, the fundamental (RMS) of each channel (`--angles` adds
  angles, `--rms` the true RMS with harmonics and DC), and for each A/B/C group
  the positive `+`, negative `-` and zero `0` sequence and the unbalance `-/+`
  (negative / positive in %), named after the common start of the group's ids
  (`iL1 iL2 iL3` → `iL+ iL- iL0 iL-/+`). In CSV: `iL_pos_A`, `iL_neg_A`,
  `iL_zero_A`, `iL_unbalance_pct`.
- `--format csv` or `json` for spreadsheets and scripts.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--channels <CHANNELS>` | Analog channels: comma separated 1-based indices or ids. Default: all |
| `--from <FROM>` | Start of the time range in ms relative to the trigger |
| `--to <TO>` | End of the time range in ms relative to the trigger |
| `--step <STEP>` | Distance between rows: cycles (`1`, `0.5`) or a time (`10ms`, `1s`) [default: 1] |
| `--cycles <CYCLES>` | Window length in cycles of the fundamental frequency [default: 1] |
| `--reference <REFERENCE>` | Channel the angles and the frequency refer to (one of the shown channels). Default: the first channel |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--angles` | Also show the angle of each channel |
| `--rms` | Also show the true RMS of each channel (with harmonics and DC) |
| `--format <FORMAT>` | Output format. Values: `text` (aligned table), `csv` (comma separated values with a header row), `json` (one JSON object) [default: text] |

The [global options](index.md#global-options) apply too. `sprcmtrd trend --help`
shows this list with examples.
