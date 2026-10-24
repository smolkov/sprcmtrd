# sprcmtrd

[![CI](https://github.com/smolkov/sprcmtrd/actions/workflows/ci.yml/badge.svg)](https://github.com/smolkov/sprcmtrd/actions/workflows/ci.yml)

A terminal viewer for COMTRADE recordings (IEEE C37.111 / IEC 60255-24), the
standard format for oscillography and status data recorded by protection relays
and disturbance recorders during power system faults.

`sprcmtrd` has two faces:

- **An interactive TUI** to browse recordings and inspect analog channels as
  line plots and binary channels as status strips, with zoom, pan and a cursor.
- **CLI commands** (`info`, `list`, `check`, `view`, `events`, `stats`, `phasors`, `export`, `cut`, `convert`) that print to stdout and can be used
  in scripts and over SSH.

## Features

- COMTRADE revisions **1991, 1999 and 2013**
- Data formats **ASCII, BINARY, BINARY32 and FLOAT32**, and the 2013 single-file
  **`.cff`** format
- `.hdr` and `.inf` content shown with the recording
- Scaling (`a * raw + b`) with primary / secondary conversion
- Multiple sample rates, timestamp based recordings, missing samples shown as gaps
- Time relative to the trigger (or absolute time of day)
- Large files: a 100 MB `.dat` opens in well under a second (memory mapped,
  min/max decimation for plotting so peaks are never lost)
- Dark, light and no-color themes; configurable key bindings

## Installation

Prebuilt binaries for Linux (x86_64, aarch64), macOS (Intel, Apple Silicon) and
Windows (x86_64) are attached to each
[GitHub release](https://github.com/smolkov/sprcmtrd/releases), with a
`SHA256SUMS` file to verify them.

Building from source requires Rust 1.88 or newer.

```sh
cargo install --path .
```

or build and run from the repository:

```sh
cargo build --release
./target/release/sprcmtrd
```

## Usage

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
sprcmtrd export FILE        # samples as CSV or JSON
sprcmtrd cut FILE -o OUT    # time window / channels as a new recording
sprcmtrd convert FILE -o OUT  # other revision, data format or .cff
sprcmtrd completions SHELL  # tab-completion script for your shell
```

`FILE` can be any file of a recording: `fault.cfg`, `fault.dat`, `fault.cff`, …
The other files of the recording are found by name.

Global options:

| Option                        | Meaning                                              |
|-------------------------------|------------------------------------------------------|
| `--theme dark\|light\|auto`   | TUI theme (default `auto`, from `COLORFGBG`)         |
| `--secondary`                 | Show analog values on the secondary side             |
| `--no-color`                  | No colors (also set by the `NO_COLOR` variable)      |
| `-v`, `-vv`, `-vvv`           | More log output                                      |

### `info`

```sh
sprcmtrd info tests/data/fault_2013.dat
sprcmtrd info --json tests/data/combined_2013.cff | jq .config.analog
```

Prints station, device, revision, data format, start and trigger time, duration,
sample rates, the analog and binary channel tables, and the `.hdr` / `.inf`
content (`--no-extra` skips it). `--json` prints the same data as JSON. Only the
configuration is read, so `info` is fast even for large recordings.

### `list`

```sh
sprcmtrd list tests/data --sort name
```

```
file                    station       rev   format    start                duration  rate   #A  #D  size      error
broken.cfg (!no data)   -             -     -         -                    -         -      -   -   -         cfg line 2: channel count 5 does not match 2A + 1D
combined_2013.cff       SUBSTATION A  2013  FLOAT32   2024-03-15 09:00:00  199.8 ms  4 kHz  4   4   21.4 kB
fault_1999.cfg          SUBSTATION A  1999  BINARY    2024-03-15 08:30:00  199.5 ms  2 kHz  4   4   7.2 kB
fault_2013.cfg          SUBSTATION B  2013  BINARY32  2026-10-20 14:00:00  2.000 s   2 kHz  8   10  168.0 kB
old_1991.cfg            SUBSTATION A  1991  ASCII     1994-03-15 08:30:00  99.00 ms  1 kHz  4   4   3.4 kB
reclose_1999_ascii.cfg  SUBSTATION C  1999  ASCII     2026-10-20 15:30:00  4.000 s   1 kHz  6   6   173.0 kB
```

- Subdirectories are searched by default; `--no-recursive` searches only `DIR`.
  Hidden directories and symbolic links to directories are skipped.
- `--filter REGEX` keeps recordings whose file name matches.
- Search by content:
  - `--station REGEX`: station name, ignoring case.
  - `--since DATE` / `--until DATE`: start time, e.g. `2024-03-15` or
    `2024-03-15 08:30`; a date alone is the whole day.
  - `--min-duration` / `--max-duration`: e.g. `500ms`, `2s`, `1min`.

  ```sh
  sprcmtrd list /data/recorder --station "line 4" --since 2026-10-01 --min-duration 1s
  ```

  Recordings that can't be parsed are left out when one of these is set.
- In a terminal, recordings are printed as they are found, with a spinner
  showing the progress of the search. `--sort time|name|size` prints a sorted
  table when the search is done instead. Piped output is always a table sorted
  by start time (or `--sort`), without spinner.
- Recordings that cannot be parsed are listed with the error instead of being
  dropped; a configuration without data file is marked `(!no data)`.
- `--valid` shows only recordings that load completely (configuration and data),
  hiding broken ones. It reads every data file, so it is slower.

### `view`

```sh
sprcmtrd view tests/data/fault_2013.cfg --channels IB,TRIP_B,CB_CLOSED \
    --from -20 --to 120 --width 72 --height 4
```

```
A2 IB [A]
        ⢈   ⡰⠋⠳⡀     ⡠⠲⡄      ⡠⢄      ⢀⢤⡀      ⢀⣀             7.244k
        ⠠  ⢠⠃  ⢣    ⡰⠁ ⠘⡄    ⡜  ⢣    ⢠⠊ ⠸⡀    ⢰⠁⠈⢣
⠤⠤⠤⠴⠲⠒⠶⠤⢴⠄⡠⠃⠄⠠ ⠄⢧ ⠄⡰⠁⠄⠠ ⠵⡠ ⠄⡸ ⠄⠠ ⡇⠠ ⢄⠮ ⠄⠠⢱⠄⠠ ⢤⠣ ⠄⠨⢆⡤⠤⠤⠤⠤⠤⠤⠤⠤⠄
        ⢈⠑⠁     ⠈⠢⠴⠃     ⠣⣀⡔⠁    ⠘⢆⢀⠎     ⠣⡀⣠⠃    ⠈⡇
                                   ⠁       ⠉⠁                 -4.602k
-20.0 ms                                            120.0 ms

▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▌████████████████████████▌ D5 TRIP_B
███████████████████████████████████████████████████▌▁▁▁▁▁▁▁▁ D7 CB_CLOSED
```

| Option                | Meaning                                                              |
|-----------------------|----------------------------------------------------------------------|
| `--channels LIST`     | Comma separated: analog index or id (`3`, `IA`), binary `d2` or id (`TRIP`). Default: all |
| `--from MS`, `--to MS`| Time window in ms relative to the trigger                            |
| `--width COLS`        | Plot width (default: terminal width, 100 when not a terminal)        |
| `--height ROWS`       | Height of each analog plot (default 10)                              |

### `check`

```sh
sprcmtrd check tests/data
```

```
status     file                      channel  finding
✗ ERROR    broken.cfg                         cfg line 2: channel count 5 does not match 2A + 1D
✓ OK       combined_2013.cff
✓ OK       fault_1999.cfg
✓ OK       fault_2013.cfg
✓ OK       fault_2013_uppercase.cfg
! WARNING  old_1991.cfg              A1 IA    1 missing value (first at sample 51)
✓ OK       reclose_1999_ascii.cfg
✓ OK       test_ascii.cff
✓ OK       test_ascii_uppercase.CFF

9 recordings: 7 ok, 1 with warnings, 1 with errors
```

- A recording file, or a directory (all recordings in it and below,
  `--no-recursive` for the directory only).
- **error**: can't be read correctly (configuration, missing or wrong-sized
  data file, sample count). **warning**: missing values, values at or beyond
  the declared min/max (clipped?), times out of order, missing units, unusable
  ratios, channel numbering, duplicate ids, trigger outside the recording.
  **note**: unusual unit or line frequency.
- Exit code **4** when a recording has an error (`--strict`: also a warning),
  otherwise 0, so it can guard scripts and CI jobs. `--json` for details.
- In a terminal the status is colored: OK cyan (blue on light backgrounds),
  warnings yellow (magenta), errors bold red. Color is only an extra: the
  symbols and words carry the meaning, so it reads in black and white and for
  color blind readers. `--no-color` / `NO_COLOR` turn it off, `--theme light`
  picks colors for light backgrounds.

### `events`

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

### `phasors`

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

### `stats`

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

### `export`

```sh
sprcmtrd export tests/data/fault_1999.cfg --channels IA,IB,TRIP --from -1 --to 1
```

```
sample,time_ms,IA [A],IB [A],TRIP
119,-1,-123.6,-267.6,0
120,-0.5,-62.6,-310.8,0
121,0,0,-346.4,0
122,0.5,625.8,-373.4,0
123,1,1236,-391.2,0
```

| Option                 | Meaning                                                          |
|------------------------|------------------------------------------------------------------|
| `--format csv\|json`   | Default: `json` for an output file ending in `.json`, else `csv` |
| `--channels LIST`      | As in `view`. Default: all                                       |
| `--from MS`, `--to MS` | Time window in ms relative to the trigger                        |
| `--every N`            | Every Nth sample only (no filtering)                             |
| `--absolute`           | Date and time of day instead of ms relative to the trigger       |
| `-o`, `--output FILE`  | Write to FILE instead of stdout                                  |

Values are scaled (primary side, `--secondary` for secondary), binary channels
0/1, missing values empty (CSV) or `null` (JSON). JSON has one array per column
(`time_ms`, `sample`, and `values` of each channel), which loads directly into
pandas or numpy.

### `cut`

```sh
sprcmtrd cut tests/data/fault_2013.cfg --from -20 --to 100 --channels IA,IB,IC,TRIP_A,CB_CLOSED -o short.cfg
```

```
tests/data/fault_2013.cfg -> short.cfg (2013 BINARY32)
  kept samples 961..=1201 of 4001 (-20.0 ms .. 100.0 ms), 3 of 8 analog and 2 of 10 binary channels
  wrote short.cfg  362 B
  wrote short.dat  5.3 kB
  read back: 241 samples, 3 analog, 2 binary channels
```

- `--from` / `--to` (ms relative to the trigger) and `--channels` work as in
  `view`; without them everything is kept.
- The new recording has the source's revision and data format; times relative
  to the trigger stay the same. `NAME.cff` writes a combined 2013 file.
- Existing files are only replaced with `--force`, and never the source
  recording. To change revision or format too, run `convert` on the result.

### `convert`

```sh
sprcmtrd convert tests/data/combined_2013.cff -o fault.cfg --rev 1999 --format binary
```

```
tests/data/combined_2013.cff (2013 FLOAT32) -> fault.cfg (1999 BINARY)
  wrote fault.cfg  440 B
  wrote fault.dat  14.4 kB
  wrote fault.hdr  31 B
  wrote fault.inf  47 B
  note: revision 1999 has no time codes or leap second field; they are left out
  note: IA: rescaled to 16-bit integers, resolution 1.2207e-1 A
  ...
  read back: 800 samples, 4 analog, 4 binary channels
```

- The output name selects the layout: `NAME.cfg` writes `.cfg` + `.dat` (and
  `.hdr` / `.inf`), `NAME.cff` one combined file.
- `--format ascii|binary|binary32|float32` and `--rev 1991|1999|2013` default
  to the source's. BINARY32, FLOAT32 and `.cff` need revision 2013, which is
  then chosen automatically.
- Anything that loses information is printed as a `note:`: channels rescaled to
  fit 16- or 32-bit integers (with the new resolution), values rounded to
  32-bit floats, and fields an older revision cannot store.
- Existing files are only replaced with `--force`, and never the source
  recording. The result is read back before the command reports success.

### `completions`

Tab completion for **bash, zsh, fish, PowerShell** and elvish. On Tab the
shell asks `sprcmtrd`, which completes subcommands, options and option values,
and for the recording argument of `info`, `view`, `events`, `stats`, `export`,
`cut`, `convert` and the viewer: **COMTRADE recordings** (`.cfg` / `.cff`) in
the typed directory **and its subdirectories**, plus the subdirectories to step
into.

```
$ sprcmtrd view sta<Tab><Tab>
station_a/                     station_a/2024/fault_1999.cfg
station_b/                     station_b/combined_2013.cff
```

Load it on shell start (recommended, it then always matches the installed
version):

```sh
# bash: ~/.bashrc
source <(COMPLETE=bash sprcmtrd)
# zsh: ~/.zshrc, after compinit
source <(COMPLETE=zsh sprcmtrd)
# fish: ~/.config/fish/config.fish
COMPLETE=fish sprcmtrd | source
# PowerShell: $PROFILE
$env:COMPLETE = "powershell"; sprcmtrd | Out-String | Invoke-Expression; Remove-Item Env:\COMPLETE
```

Or write the script to a file with `sprcmtrd completions SHELL --output FILE`
(for example `~/.local/share/bash-completion/completions/sprcmtrd`); write it
again after updating `sprcmtrd`. Hidden directories are skipped (type `.` to
see them), and a Tab offers at most 500 recordings, so it stays quick in large
trees.

### Exit codes

| Code | Meaning                                                 |
|------|---------------------------------------------------------|
| 0    | Success                                                 |
| 1    | Other error (bad filter, unknown channel…)              |
| 2    | Invalid command line arguments                          |
| 3    | File not found or not readable                          |
| 4    | Invalid or unsupported COMTRADE content, `check` failed |

## Interactive viewer

```
 ▄▄▄ ▄▄▄  ▄ ▄▄  ▄▄▄ ▄▄▄▄   ▄▄█ ▄ ▄▄  ▄▄█                       fault_2013.cfg
▀▀▄▄ █  █ █▀   █    █ █ █ █  █ █▀   █  █                SUBSTATION B · RELAY 21
▀▀▀  █▀▀  ▀     ▀▀▀ ▀ ▀ ▀  ▀▀▀ ▀     ▀▀▀  v0.1.0   2013 · BINARY32 · 2 kHz · 4001 samples
┌ Channels ──────┐┌ Plot ──────────────────────── cursor 2.0 ms · window 2.000 s ┐
│● IA           A││IA [A]         ...analog line plots...                       │
│● IB           A││▸ 2.828k                                                     │
│  ...           ││...                                                          │
│──── binary ────││TRIP_B        1 ▁▁▁▁▁▁▌██▌▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁ │
│● TRIP_B        ││CB_CLOSED     0 ██████▌▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▌██████████████████████ │
│● CB_CLOSED     ││               -0.500 s      T ▲ 2.0 ms     0.500 s   1.500 s│
└────────────────┘└─────────────────────────────────────────────────────────────┘
q quit  o open  i info  ? help  Space toggle  + zoom  ← cursor  Shift+← pan  c config
```

- **Sidebar:** channel list; `●` shown, `○` hidden. The first six analog channels
  and up to eight binary channels that change state are shown after loading.
- **Plots:** one shared time axis for all channels. The gutter shows the channel,
  its value at the cursor and the visible value range. Binary states that differ
  from the channel's normal state are highlighted.
- **Time axis:** tick labels, the trigger `T` and the cursor `▲` with its exact
  time. The cursor always sits on a sample.
- Minimum terminal size is 80×24.

### Keys

| Key                 | Action                                        |
|---------------------|-----------------------------------------------|
| `q`, `Ctrl+c`       | Quit (closes an open overlay first)           |
| `o`, `Ctrl+p`       | Open file picker                              |
| `i`                 | Recording info overlay                        |
| `?`                 | Help overlay                                  |
| `c`, `Ctrl+,`       | Configuration overlay                         |
| `Tab`               | Switch focus sidebar / plot                   |
| `↑`/`↓`, `k`/`j`    | Select channel (sidebar), zoom (plot)         |
| `Space`             | Show / hide selected channel                  |
| `a`                 | Show all / hide all channels                  |
| `+`/`=`, `-`        | Zoom in / out (time, around the cursor)       |
| `←`/`→`, `h`/`l`    | Move cursor                                   |
| `Shift+←`/`→`, `H`/`L` | Pan the view                               |
| `0`                 | Reset zoom (whole recording)                  |
| `t`                 | Jump cursor to trigger                        |
| `p`                 | Primary / secondary values                    |
| `m`                 | Relative / absolute time                      |
| `s`                 | Show / hide sidebar                           |
| `T`                 | Dark / light theme (not saved)                |
| `Esc`               | Close overlay                                 |

`Ctrl+,` works in terminals with the kitty keyboard protocol (kitty, WezTerm,
foot, Ghostty, recent Alacritty); `c` works everywhere.

### File picker

Opens at start when no file is given, and with `o`.

- Lists the subdirectories of the current directory, then all recordings in it
  **and its subdirectories**, with paths relative to it. `Ctrl+r` switches to
  this directory only.
- `Ctrl+b` shows only valid recordings, hiding those whose configuration or data
  fails to load (like `list --valid`); press it again to show all.
- Type to filter (case-insensitive substring); start with `/` for a regular
  expression, e.g. `/^station_b/.*2026`.
- `Enter` opens a recording or directory, `Backspace` on an empty filter goes to
  the parent directory, `Esc` closes.
- The search runs in the background and results appear as they are found.

### Configuration overlay

`c` (or `Ctrl+,`) shows the theme and every key binding. Changes take effect
immediately and are saved to the configuration file:

- Theme row: `Enter` or `←`/`→` switches dark / light.
- Binding row: `Enter`, then press the new key (`Esc` cancels). Keys already
  used by another action are refused. `Backspace` restores the default.

## Configuration file

`~/.config/sprcmtrd/config.toml` (Linux; the platform config directory
elsewhere, or the path in `SPRCMTRD_CONFIG`). It is created by the
configuration overlay when missing and can also be edited by hand:

```toml
theme = "dark"                 # dark | light | auto

[keys]                         # action = list of keys, replaces the defaults
zoom_in = ["+", "="]
open = ["o", "ctrl+p"]
pan_left = ["shift+left", "H"]
```

Key names: single characters (`q`, `T`, `?`, `,`), `space`, `tab`, `enter`,
`esc`, `backspace`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`,
`pagedown`, with `ctrl+`, `alt+` or `shift+` prefixes. Action names are the
ones in the configuration overlay (`quit`, `open`, `zoom_in`, …). An invalid
file is reported at start. Saving from the overlay rewrites the whole file, so
comments are not kept. `--theme` and `--no-color` override the file.

## Logging

CLI commands log warnings to stderr. The TUI never logs to the screen: with
`-v` (or `RUST_LOG`) it writes to `~/.local/state/sprcmtrd/sprcmtrd.log`
(the platform state or cache directory elsewhere).

## Development

```sh
cargo test            # unit, TUI render and CLI integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check     # rustfmt.toml: max_width = 120
```

GitHub Actions run these on every push to `main` and every pull request
(`.github/workflows/ci.yml`): format and clippy, tests on Linux, macOS and
Windows, and a build with the minimum Rust version.

### Releasing

1. Set the new version in `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`

`.github/workflows/release.yml` checks that the tag matches the `Cargo.toml`
version, runs the tests, builds the binaries for all platforms and publishes a
GitHub release with the archives, `SHA256SUMS` and generated release notes.

### GitLab

`.gitlab-ci.yml` runs the same checks on Linux Docker runners for merge
requests and the default branch: rustfmt, clippy, tests and the minimum Rust
version. For a version tag pushed to GitLab (`git push gitlab v0.2.0`) it also
checks the tag against `Cargo.toml` and builds release binaries with
`ci/build-release.sh`. `ci/gitlab-release.sh` uploads the binaries and
`SHA256SUMS` to the project's generic package registry and creates a GitLab
release linking to them.

| Platform        | GitHub                    | GitLab                           |
|-----------------|---------------------------|----------------------------------|
| Linux x86_64    | tests + binary            | tests + binary                   |
| Linux aarch64   | binary                    | binary (cross-compiled)          |
| Windows x86_64  | tests + binary (MSVC)     | binary (MinGW, cross-compiled)   |
| macOS           | tests + binaries          | — (needs a macOS runner)         |

The GitLab release needs the package registry enabled for the project
(Settings → General → Visibility → Package registry).

| Path               | Content                                                                 |
|--------------------|-------------------------------------------------------------------------|
| `src/comtrade/`    | COMTRADE library: reading, writing, export, statistics, events, ...     |
| `src/command/`     | Commands (`info`, `list`, `view`, `stats`, ...) and output formatting   |
| `src/tui/`         | TUI: state and keys (`app.rs`), events, theme, widgets                  |
| `tests/data/`      | Synthetic sample recordings (all revisions and formats)                 |
| `spec.md`          | Specification                                                           |

The `comtrade` module does not depend on the command or TUI code and could be
used as a library on its own.

The recordings in `tests/data/` are synthetic and generated for testing; they
are not real disturbance records.
