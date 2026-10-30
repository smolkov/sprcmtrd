# Commands

Without a command `sprcmtrd` starts the [interactive viewer](../viewer/index.md).
With one, it prints to stdout, so the output can be piped, saved and used in
scripts.

```
sprcmtrd                    # TUI, file picker in the current directory
sprcmtrd PATH               # TUI with PATH opened (recording file or directory)
sprcmtrd info FILE          # summary of a recording
sprcmtrd list [DIR]         # recordings in DIR and its subdirectories
sprcmtrd view FILE          # summary and plots printed to stdout
sprcmtrd check PATH         # validate recordings (exit code 4 on errors)
sprcmtrd events FILE        # timeline of binary channel changes
sprcmtrd stats FILE         # min, max, mean, RMS, peak; binary changes
sprcmtrd phasors FILE       # phasors and symmetrical components at a time
sprcmtrd trend FILE         # the same over time, cycle by cycle
sprcmtrd frequency FILE     # system frequency and its rate of change
sprcmtrd harmonics FILE     # harmonic spectrum and THD at a time
sprcmtrd impedance FILE     # apparent impedance of the fault loops
sprcmtrd power FILE         # active, reactive power and power factor
sprcmtrd export FILE        # samples as CSV or JSON
sprcmtrd cut FILE -o OUT    # time window / channels as a new recording
sprcmtrd convert FILE -o OUT  # other revision, data format or .cff
sprcmtrd completions SHELL  # tab-completion script for your shell
```

| Command | What it does |
|---|---|
| [`info`](info.md) | Summary of a recording |
| [`list`](list.md) | Recordings in a directory |
| [`check`](check.md) | Validate recordings |
| [`view`](view.md) | Summary and plots on stdout |
| [`events`](events.md) | Timeline of binary channel changes |
| [`stats`](stats.md) | Channel statistics |
| [`phasors`](phasors.md) | Phasors and symmetrical components |
| [`trend`](trend.md) | Values over time |
| [`frequency`](frequency.md) | System frequency and ROCOF |
| [`harmonics`](harmonics.md) | Harmonic spectrum and THD |
| [`impedance`](impedance.md) | Fault loop impedance |
| [`power`](power.md) | Active, reactive power and power factor |
| [`export`](export.md) | Samples as CSV or JSON |
| [`cut`](cut.md) | Time window as a new recording |
| [`convert`](convert.md) | Other revision, format or layout |
| [`completions`](completions.md) | Shell completion script |

## Recordings

`FILE` can be any file of a recording: `fault.cfg`, `fault.dat`, `fault.cff`, …
The other files of the recording (`.dat`, `.hdr`, `.inf`) are found by name;
extensions may be upper or lower case.

## Channels and time windows

Commands that work on channels take `--channels` with comma separated entries:
an analog channel by 1-based index (`3`) or id (`IA`), a binary channel by `dN`
(`d2`) or id (`TRIP`). Without it, all channels are used.

`--from` and `--to` give a time window in ms relative to the trigger (negative
before it). Samples within 1 ns of an edge count as inside.

## Global options

These apply to every command and to the viewer:

| Option                        | Meaning                                              |
|-------------------------------|------------------------------------------------------|
| `--theme dark\|light\|auto`   | TUI theme (default `auto`, from `COLORFGBG`)         |
| `--secondary`                 | Show analog values on the secondary side             |
| `--no-color`                  | No colors (also set by the `NO_COLOR` variable)      |
| `-v`, `-vv`, `-vvv`           | More log output                                      |

## Exit codes

| Code | Meaning                                                 |
|------|---------------------------------------------------------|
| 0    | Success                                                 |
| 1    | Other error (bad filter, unknown channel…)              |
| 2    | Invalid command line arguments                          |
| 3    | File not found or not readable                          |
| 4    | Invalid or unsupported COMTRADE content, `check` failed |
