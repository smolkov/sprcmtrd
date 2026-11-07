# `fault`: Fault summary

The first things to know about a fault: when it started, which phases it
involves, how long the current flowed, how high it got, and how fast the
protection and the breaker operated.

```
sprcmtrd fault [OPTIONS] <FILE>
```

```sh
sprcmtrd fault tests/data/fault_2013.cfg
```

A phase B to ground fault at the trigger: phase B carries 3.4 kA more than the
load and its voltage drops by 72 %. The protection starts after 10 ms, trips
after 60 ms, and the breaker interrupts the current after 100 ms.

```
File       tests/data/fault_2013.cfg
Currents   IA IB IC [A]
Voltages   VA VB VC [kV]
Values     primary, RMS of the fundamental at 50 Hz
Inception  0.000 s (2026-10-20 14:00:00.500000), sample 1001
Fault      B-N
Duration   0.100 s, current interrupted at +0.100 s

phase           pre-fault [A]  fault [A]  ΔI [A]  peak [A]  peak at   pre-fault [V]  fault [V]  drop
A               318.2          318.2      0.000   450.0     +0.005 s  63.50k         63.50k     0.0 %
B      faulted  318.2          3.721k     3.403k  7.244k    +0.011 s  63.50k         17.68k     72.2 %
C               318.2          318.2      0.000   449.4     +0.008 s  63.50k         63.50k     0.0 %
3I0    ground   0.000          3.403k     3.403k

Operate times from inception
event                since inception  time      channel
inception            +0.000 s         0.000 s
start                +0.010 s         +0.010 s  D2 START_B
trip                 +0.060 s         +0.060 s  D5 TRIP_B
breaker open         +0.100 s         +0.100 s  D7 CB_CLOSED
current interrupted  +0.100 s         +0.100 s
```

- **Inception** is the first sample where a current departs from the cycle
  before it (the superimposed current i(n) − i(n − one cycle), as relays
  detect a fault). The threshold is 20 % of the pre-fault peak, at least 2 % of
  the largest current, so an unloaded line works too.
- **Faulted phases** come from ΔI, the change of each phase's phasor against
  the pre-fault cycle, so load current doesn't count: a phase is faulted when
  its ΔI is at least half the largest, ground (`N`) is involved when 3·ΔI0 is
  at least 20 % of it. The table shows the RMS of the fundamental before and
  during the fault, ΔI and the instantaneous peak (with DC offset). The fault
  cycle starts a quarter cycle after inception.
- **Duration** ends when the faulted phases stay below 5 % of their peak for
  a cycle (or until the end of the recording).
- **Operate times** are measured from inception. Start, trip and breaker
  channels are found by name (`START`/`PICKUP`, `TRIP`, `CB`/`BREAKER`); for
  other names give them with `--start`, `--trip`, `--breaker`. A breaker
  position named `…CLOSE…` opens when it goes to 0, others when they go to 1.
- Currents are found by unit (A, kA); `--current IL1,IL2,IL3` chooses them.
  Voltages are optional; with them the table shows the voltage drop.
- Only the first fault is analysed. For a reclose onto the fault, start the
  search after the first one with `--from`. When no current flowed before the
  inception, a note says the line was energized: the currents alone can't tell
  a fault from load then.
- `--json` for scripts; currents in A, voltages in V, times in s.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--current <CURRENT>` | Current channels of phases A, B, C (indices or ids). Default: found by unit |
| `--voltage <VOLTAGE>` | Voltage channels of phases A, B, C (indices or ids). Default: found by unit, if any |
| `--start <START>` | Binary channel of the protection start (`dN` or id). Default: found by name |
| `--trip <TRIP>` | Binary channel of the trip command (`dN` or id). Default: found by name |
| `--breaker <BREAKER>` | Binary channel of the breaker position (`dN` or id); open is a change to 1, or to 0 when the id contains CLOSE. Default: found by name |
| `--from <FROM>` | Start of the search in ms relative to the trigger |
| `--to <TO>` | End of the search in ms relative to the trigger |
| `--frequency <FREQUENCY>` | Fundamental frequency in Hz. Default: the line frequency of the recording |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd fault --help`
shows this list with examples.
