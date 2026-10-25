# `stats`: Channel statistics

Statistics per channel: min, max, mean, RMS and peak of analog channels, state changes of binary channels.

```
sprcmtrd stats [OPTIONS] <FILE>
```

```sh
sprcmtrd stats tests/data/fault_2013.cfg --channels IA,IB,VA,TRIP_B,CB_CLOSED
```

```
File    tests/data/fault_2013.cfg
Window  -0.500 s .. 1.500 s (4001 samples)
Values  primary

Analog channels
channel  unit  min      at        max     at        mean   rms    peak    at        missing
A1 IA    A     -450.0   -0.485 s  450.0   -0.495 s  0.000  246.4  450.0   -0.495 s  0
A2 IB    A     -4.602k  0.082 s   7.244k  0.011 s   55.31  862.2  7.244k  0.011 s   0
A5 VA    kV    -89.80   -0.485 s  89.80   -0.495 s  0.000  49.18  89.80   -0.495 s  0

Binary channels
channel       first  last  changes  first change  time set
D5 TRIP_B     0      0     2        0.060 s       0.060 s
D7 CB_CLOSED  1      1     2        0.100 s       1.200 s
```

- Analog: min and max with their time, mean, RMS over the window, peak (largest
  magnitude, with sign) and its time, missing samples (left out of the rest).
- Binary: first and last state, number of changes, first change, time set.
- `--channels`, `--from` / `--to` as in `view`; `--secondary` for secondary
  values; `--json` for full values. `stat` works too.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--channels <CHANNELS>` | Channels: comma separated 1-based indices or ids (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all |
| `--from <FROM>` | Start of the time window in ms relative to the trigger |
| `--to <TO>` | End of the time window in ms relative to the trigger |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd stats --help`
shows this list with examples.
