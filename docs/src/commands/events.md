# `events`: Timeline of binary channel changes

Timeline of binary channel changes (start, trip, breaker open, ...).

```
sprcmtrd events [OPTIONS] <FILE>
```

```sh
sprcmtrd events tests/data/fault_2013.cfg
```

```
File     tests/data/fault_2013.cfg
Window   -0.500 s .. +1.500 s (4001 samples)
Trigger  2026-10-20 14:00:00.500000

time      Δ         absolute                    channel       change
0.000 s             2026-10-20 14:00:00.500000  trigger
+0.010 s  +0.010 s  2026-10-20 14:00:00.510000  D2 START_B    0 → 1
+0.060 s  +0.050 s  2026-10-20 14:00:00.560000  D5 TRIP_B     0 → 1
+0.100 s  +0.040 s  2026-10-20 14:00:00.600000  D2 START_B    1 → 0
+0.100 s  +0.000 s  2026-10-20 14:00:00.600000  D7 CB_CLOSED  1 → 0
+0.120 s  +0.020 s  2026-10-20 14:00:00.620000  D5 TRIP_B     1 → 0
+0.850 s  +0.730 s  2026-10-20 14:00:01.350000  D8 RECLOSE    0 → 1
+0.900 s  +0.050 s  2026-10-20 14:00:01.400000  D7 CB_CLOSED  0 → 1
+0.950 s  +0.050 s  2026-10-20 14:00:01.450000  D8 RECLOSE    1 → 0

8 changes in 10 binary channels
```

- One row per change of a binary channel: time relative to the trigger, `Δ`
  since the previous row (operate times), time of day, channel and change. The
  trigger is a row of its own.
- `--channels` (binary channels), `--from` / `--to` as in `view`; `--json` for
  scripts.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--channels <CHANNELS>` | Binary channels: comma separated `dN` indices or ids. Default: all |
| `--from <FROM>` | Start of the time window in ms relative to the trigger |
| `--to <TO>` | End of the time window in ms relative to the trigger |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd events --help`
shows this list with examples.
