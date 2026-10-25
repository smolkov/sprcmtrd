# `harmonics`: Harmonic spectrum and THD

Harmonic spectrum and THD per channel at one moment. A DFT over whole cycles ending at --at, like phasors. Harmonics and DC in % of the fundamental; orders up to --max-order, at most below half the sample rate.

```
sprcmtrd harmonics [OPTIONS] <FILE>
```

```sh
sprcmtrd harmonics tests/data/fault_2013.cfg --at 40 \
    --channels IA,IB,IC --max-order 7
```

During the fault, phase B carries a decaying DC offset (29 % of the
fundamental), which also shows as small harmonics; phases A and C are pure
sines.

```
File    tests/data/fault_2013.cfg
Window  20.50 ms .. 40.00 ms (1 cycle, 40 samples, 50 Hz)
Orders  up to 7
Values  primary; DC and harmonics in % of the fundamental

order        IA [A]  IB [A]  IC [A]
fundamental  318.2   3.458k  318.2
DC           0.0 %   29.1 %  0.0 %
2            0.0 %   2.3 %   0.0 %
3            0.0 %   1.6 %   0.0 %
4            0.0 %   1.2 %   0.0 %
5            0.0 %   1.0 %   0.0 %
6            0.0 %   0.8 %   0.0 %
7            0.0 %   0.7 %   0.0 %
THD 2–7      0.0 %   3.4 %   0.0 %
```

- The spectrum of the window of `--cycles` cycles (default 1) ending at `--at`
  ms after the trigger, like `phasors`: the fundamental (RMS), DC and each
  harmonic in % of the fundamental, and the THD over the orders shown.
- Orders up to `--max-order` (default 50), at most below half the sample rate.
  `--angles` adds the harmonic angles, `--json` full values.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--at <AT>` | End of the window in ms relative to the trigger [default: 0] |
| `--cycles <CYCLES>` | Window length in cycles of the fundamental frequency [default: 1] |
| `--channels <CHANNELS>` | Analog channels: comma separated 1-based indices or ids. Default: all |
| `--max-order <MAX_ORDER>` | Highest harmonic order (limited to below half the sample rate) [default: 50] |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--angles` | Also show the angle of each harmonic (relative to the fundamental) |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd harmonics --help`
shows this list with examples.
