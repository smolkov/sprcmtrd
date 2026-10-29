# `frequency`: System frequency and ROCOF

System frequency over time and its rate of change (ROCOF). Measured on one channel (default: the first voltage) over --cycles cycles ending at each row: the periods between rising zero crossings, or the turn of the phasor (with frequency tracking). The ROCOF compares each measurement with the one of the cycles before it.

```
sprcmtrd frequency [OPTIONS] <FILE>
```

```sh
sprcmtrd frequency tests/data/reclose_1999_ascii.cfg --from -60 --to 120
```

The voltage of phase A at the fault: the phase jump at the inception shows as a
short frequency excursion, and after the breaker opens there is no signal left
to measure.

```
File       tests/data/reclose_1999_ascii.cfg
Channel    A4 VA [kV]
Method     zero crossings over 1 cycle (20 samples), every 1 cycle (10 rows)
Nominal    50 Hz
Frequency  min 50.000 Hz at 40.0 ms, max 52.253 Hz at 20.0 ms, mean 50.367 Hz
ROCOF      largest -115.154 Hz/s at 40.0 ms

time      f [Hz]  Δf [Hz]  df/dt [Hz/s]
-60.0 ms  50.000  0.000    0.000
-40.0 ms  50.000  0.000    0.000
-20.0 ms  50.000  0.000    0.000
0.0 ms    51.053  +1.053   +53.174
20.0 ms   52.253  +2.253   +62.022
40.0 ms   50.000  0.000    -115.154
60.0 ms   50.000  0.000    0.000
80.0 ms   50.000  0.000    0.000
100.0 ms  50.000  0.000    0.000
120.0 ms  -       -        -
```

- One channel, by default the first voltage channel (`--channel` for another).
- `--method crossings` (default): the frequency of the last `--cycles` periods
  between rising zero crossings. `--method phasor`: the turn of the fundamental
  phasor between two windows of `--cycles` cycles, as a relay measures it.
- `Δf` is the deviation from the nominal frequency, `df/dt` the rate of change
  (ROCOF) against the measurement of the cycles before.
- For frequency events (generator trips, islanding) measure over more cycles,
  e.g. `--cycles 5 --step 100ms`: the ROCOF of one cycle is noisy.
- `--format csv` or `json` for spreadsheets and scripts.

See [Analysis methods](../analysis/index.md#frequency) for how both methods
work and where they reach their limits.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--channel <CHANNEL>` | Analog channel to measure (index or id). Default: the first voltage channel, else the first analog channel |
| `--method <METHOD>` | How the frequency is measured. Values: `crossings` (periods between rising zero crossings), `phasor` (turn of the fundamental phasor between windows) [default: crossings] |
| `--from <FROM>` | Start of the time range in ms relative to the trigger |
| `--to <TO>` | End of the time range in ms relative to the trigger |
| `--step <STEP>` | Distance between rows: cycles (`1`, `0.5`) or a time (`10ms`, `1s`) [default: 1] |
| `--cycles <CYCLES>` | Measurement length in cycles of the nominal frequency (whole cycles with --method crossings) [default: 1] |
| `--frequency <FREQUENCY>` | Nominal frequency in Hz. Default: the line frequency of the recording |
| `--format <FORMAT>` | Output format. Values: `text` (aligned table), `csv` (comma separated values with a header row), `json` (one JSON object) [default: text] |

The [global options](index.md#global-options) apply too. `sprcmtrd frequency --help`
shows this list with examples. Alias: `freq`.
