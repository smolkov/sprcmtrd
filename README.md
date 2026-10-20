# sprcmtrd

[![CI](https://github.com/smolkov/sprcmtrd/actions/workflows/ci.yml/badge.svg)](https://github.com/smolkov/sprcmtrd/actions/workflows/ci.yml)

A terminal viewer for COMTRADE recordings (IEEE C37.111 / IEC 60255-24), the
standard format for oscillography and status data recorded by protection relays
and disturbance recorders during power system faults.

`sprcmtrd` has two faces:

- **An interactive TUI** to browse recordings and inspect analog channels as
  line plots and binary channels as status strips, with zoom, pan and a cursor.
- **CLI commands** (`info`, `list`, `view`) that print to stdout and can be used
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
file                   station       rev   start                duration  rate   #A  #D  size      error
broken.cfg (!no data)  -             -     -                    -         -      -   -   -         cfg line 2: channel count 5 does not match 2A + 1D
combined_2013.cff      SUBSTATION A  2013  2024-03-15 09:00:00  199.8 ms  4 kHz  4   4   21.4 kB
fault_1999.cfg         SUBSTATION A  1999  2024-03-15 08:30:00  199.5 ms  2 kHz  4   4   7.2 kB
fault_2013.cfg         SUBSTATION B  2013  2026-10-20 14:00:00  2.000 s   2 kHz  8   10  168.0 kB
old_1991.cfg           SUBSTATION A  1991  1994-03-15 08:30:00  99.00 ms  1 kHz  4   4   3.4 kB
```

- Subdirectories are searched by default; `--no-recursive` searches only `DIR`.
  Hidden directories and symbolic links to directories are skipped.
- `--filter REGEX` keeps recordings whose file name matches.
- `--sort time|name|size` (default `time`).
- Recordings that cannot be parsed are listed with the error instead of being
  dropped; a configuration without data file is marked `(!no data)`.

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

### `completions`

Prints a tab-completion script for **bash, zsh, fish, PowerShell** or elvish
(`--output FILE` writes it to a file instead of stdout). It completes
subcommands, options and option values (`--theme`, `--sort`, shells), and file
and directory paths, stepping into subdirectories as you type.

```sh
# bash
sprcmtrd completions bash --output ~/.local/share/bash-completion/completions/sprcmtrd
# zsh: a directory in $fpath, e.g. with fpath=(~/.zfunc $fpath) in ~/.zshrc
sprcmtrd completions zsh --output ~/.zfunc/_sprcmtrd
# fish
sprcmtrd completions fish --output ~/.config/fish/completions/sprcmtrd.fish
# PowerShell
sprcmtrd completions powershell >> $PROFILE
```

Start a new shell afterwards. Regenerate the script after updating `sprcmtrd`.

### Exit codes

| Code | Meaning                                    |
|------|--------------------------------------------|
| 0    | Success                                    |
| 1    | Other error (bad filter, unknown channel…) |
| 2    | Invalid command line arguments             |
| 3    | File not found or not readable             |
| 4    | Invalid or unsupported COMTRADE content    |

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

| Path               | Content                                                       |
|--------------------|---------------------------------------------------------------|
| `src/comtrade/`    | COMTRADE library: `.cfg`, `.dat`, `.cff` parsing, discovery    |
| `src/command/`     | `info`, `list`, `view` commands and output formatting         |
| `src/tui/`         | TUI: state and keys (`app.rs`), events, theme, widgets        |
| `tests/data/`      | Synthetic sample recordings (all revisions and formats)       |
| `spec.md`          | Specification                                                 |

The `comtrade` module does not depend on the command or TUI code and could be
used as a library on its own.

The recordings in `tests/data/` are synthetic and generated for testing; they
are not real disturbance records.
