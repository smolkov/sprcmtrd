# `compare`: Two recordings of one event

Two recordings of the same event side by side: the relays at both ends of a
line, a relay and the fault recorder of the same bay, or a test before and
after a change of settings. For each pair of channels: the RMS in A and B,
the ratio and the angle difference. With `--differential`, the differential
current of two line ends.

```
sprcmtrd compare [OPTIONS] <A> <B>
```

```sh
sprcmtrd compare end_a.cfg end_b.cfg --at 40 --differential
```

Both ends of a line during a phase A fault on it. B's relay triggered 12 ms
later and records in kA at 2 kHz. With both currents measured into the line,
phase A is fed from both ends, so its differential current is the fault
current. Phases B and C carry load through the line: B is A turned by 180°,
and the differential current is 0.

```
A          end_a.cfg (end_a)
B          end_b.cfg (end_b)
Alignment  absolute time: B's trigger at +12.00 ms in A's time
Window     21.00 ms .. 40.00 ms in A (1 cycle, 20 samples, 50 Hz); 8.50 ms .. 28.00 ms in B (40 samples)
Values     primary; RMS, angles relative to A IA

A   B   unit  A       B       B/A    angle A  angle B  B − A
IA  IA  A     2.828k  1.414k  0.500  0.0°     5.0°     5.0°
IB  IB  A     353.6   353.6   1.000  -60.0°   120.0°   180.0°
IC  IC  A     353.6   353.6   1.000  180.0°   0.0°     180.0°

Differential: A IA IB IC · B IA IB IC, both measured into the line
phase  I A [A]  I B [A]  I diff [A]  I restraint [A]  diff / restraint
A      2.828k   1.414k   4.239k      2.121k           199.8 %
B      353.6    353.6    0.000       353.6            0.0 %
C      353.6    353.6    0.000       353.6            0.0 %
```

- **Pairs:** channels with the same id (case doesn't matter). `--pair
  IA=IL1,IB=IL2,IC=IL3` pairs channels with different names; `--channels`
  keeps only some channels of A. V/kV and A/kA are scaled to A's unit.
- **Alignment:** `--align time` (default) lines up the absolute trigger
  times, right when both relays have synchronized clocks (GPS, PTP).
  `--align trigger` puts both triggers at 0. `--align fault` lines up the
  fault inceptions, as [`fault`](fault.md) finds them, for clocks that aren't
  synchronized. `--shift <ms>` moves B later on top of that.
- **Window:** `--at` and `--cycles` as in [`phasors`](phasors.md), in A's
  time; B's window ends at the same instant. The sample rates may differ.
- **Angles:** relative to A's first channel (`--reference`), B's turned into
  A's time. They are only as good as the alignment: 1 ms is 18° at 50 Hz.
- **Differential:** the current groups of A and B (found by unit, or
  `--current-a` / `--current-b`), both measured into the line; `--invert-b`
  when B's currents are measured out of it. I_diff = |I_A + I_B|,
  I_restraint = (|I_A| + |I_B|) / 2. See [Comparing two
  recordings](../analysis/index.md#comparing-two-recordings).
- `--json` for scripts.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<A>` | Recording A (any file of it: .cfg, .dat, .cff, ...); times refer to A |
| `<B>` | Recording B, compared with A |
| `--at <AT>` | End of the window in ms relative to A's trigger. Default 0 |
| `--cycles <CYCLES>` | Window length in cycles of the fundamental frequency. Default 1 |
| `--align <ALIGN>` | How the time axes are aligned: `time` (default), `trigger`, `fault` |
| `--shift <SHIFT>` | Moves B later by this many ms, after the alignment |
| `--channels <CHANNELS>` | Only these channels of A (indices or ids). Default: all pairs |
| `--pair <PAIR>` | Channel pairs with different ids: `A_ID=B_ID`, comma separated. Default: equal ids |
| `--reference <REFERENCE>` | Channel of A the angles refer to (index or id). Default: the first pair |
| `--differential` | Differential current of two line ends from the current groups of A and B |
| `--current-a <CURRENT_A>` | Current channels of phases A, B, C in A (indices or ids). Default: found by unit |
| `--current-b <CURRENT_B>` | Current channels of phases A, B, C in B (indices or ids). Default: found by unit |
| `--invert-b` | B's currents are measured out of the line (for `--differential`) |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of A |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd compare
--help` shows this list with examples.
