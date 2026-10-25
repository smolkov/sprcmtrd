# `phasors`: Phasors and symmetrical components

Phasors of the fundamental frequency and symmetrical components. A DFT over whole cycles ending at --at, as a protection relay sees the signals at that moment. Magnitudes are RMS, angles in degrees relative to the reference channel. Channels with phases A, B, C of one circuit and unit also get zero, positive and negative sequence.

```
sprcmtrd phasors [OPTIONS] <FILE>
```

```sh
sprcmtrd phasors tests/data/fault_2013.cfg --at 40 --channels IA,IB,IC,VA,VB,VC --reference VA
```

```
File       tests/data/fault_2013.cfg
Window     20.50 ms .. 40.00 ms (1 cycle, 40 samples, 50 Hz)
Reference  A5 VA
Values     primary, RMS

channel  unit  magnitude  angle
A1 IA    A     318.2      0.0°
A2 IB    A     3.458k     -117.7°
A3 IC    A     318.2      120.0°
A5 VA    kV    63.50      0.0°
A6 VB    kV    17.68      -120.0°
A7 VC    kV    63.50      120.0°

Symmetrical components
phases    circuit  unit  zero              positive       negative         unbalance
IA IB IC  LINE 4   A     1.047k ∠ -117.5°  1.365k ∠ 1.9°  1.047k ∠ 122.5°  76.7 %
VA VB VC  LINE 4   kV    15.27 ∠ 60.0°     48.22 ∠ 0.0°   15.27 ∠ -60.0°   31.7 %
```

- Phasors of the fundamental (line frequency of the recording, `--frequency`
  to override) over `--cycles` cycles (default 1) ending at `--at` ms after the
  trigger, as a relay sees them. RMS magnitudes, angles relative to
  `--reference` (default: the first channel).
- Symmetrical components (zero, positive, negative sequence and unbalance) for
  each A/B/C set of one circuit and unit. In the example the phase B to ground
  fault shows as equal zero and negative sequence currents.
- `--secondary` for secondary values, `--json` for full values. `phasor`
  works too.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--at <AT>` | End of the window in ms relative to the trigger [default: 0] |
| `--cycles <CYCLES>` | Window length in cycles of the fundamental frequency [default: 1] |
| `--channels <CHANNELS>` | Analog channels: comma separated 1-based indices or ids. Default: all |
| `--reference <REFERENCE>` | Channel the angles refer to (index or id). Default: the first channel |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd phasors --help`
shows this list with examples.
