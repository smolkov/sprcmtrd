# sprcmtrd — specification

## Overview

`sprcmtrd` is a terminal viewer for COMTRADE files, written in Rust. It has two
faces:

- **CLI commands** (`info`, `list`, `view`, `stats`, `export`, `cut`, `convert`) that print to stdout and can be used in scripts.
- **Interactive TUI** (built on [ratatui]) for browsing files and inspecting
  analog and binary channels as plots.

COMTRADE (Common Format for Transient Data Exchange for Power Systems, IEEE Std
C37.111 / IEC 60255-24) is the standard format for oscillography and status data
recorded during power system disturbances, faults and simulation tests.

| Item              | Value                                   |
|-------------------|-----------------------------------------|
| Crate name        | `sprcmtrd`                              |
| Binary name       | `sprcmtrd`                              |
| Rust edition      | 2024 (matches `Cargo.toml`)             |
| Async runtime     | Tokio (used for TUI event loop only)    |
| Target platforms  | Linux, macOS, Windows terminals         |

### Goals

- Open and display COMTRADE recordings quickly, also large ones (≥ 100 MB `.dat`).
- Give a readable summary of a recording without leaving the terminal.
- Plot analog channels and binary channel states with zoom and pan.
- Work over SSH and in plain terminals (no GUI dependencies).

### Non-goals (for v1)

- Editing COMTRADE files. Writing is limited to `convert` and `cut` (a new
  recording in another revision, format or layout, or a part of one).
- Signal analysis (RMS, phasors, harmonics, FFT). May come later.
- Comparing several recordings side by side.
- Mouse-first interaction (keyboard is primary; mouse is optional).

## COMTRADE support

### Standard revisions

| Revision | Support | Notes                                                                 |
|----------|---------|-----------------------------------------------------------------------|
| 1991     | Yes     | No revision year in `.cfg` line 1; shorter analog/digital lines.      |
| 1999     | Yes     | Main target.                                                          |
| 2013     | Yes     | Adds time code / leap second lines, UTF-8, `.cff` single-file format. |

The revision is detected from line 1 of the `.cfg`. If the year is missing, assume 1991.

### File types

| Extension | Content                      | Required | v1 support                     |
|-----------|------------------------------|----------|--------------------------------|
| `.cfg`    | Configuration (text)         | Yes      | Full parse                     |
| `.dat`    | Sample data                  | Yes      | Full parse                     |
| `.hdr`    | Free-text header             | No       | Show as raw text in `info`     |
| `.inf`    | Extra information (INI-like) | No       | Show as raw text in `info`     |
| `.cff`    | 2013 combined file           | No       | Split into sections, then same as above |

File extensions are matched case-insensitively (`.CFG` and `.cfg` both work).
Given any one file of a recording (for example `fault.dat`), the matching
siblings are found by replacing the extension.

### Data file formats

| `ft` in `.cfg` | Analog sample type | Notes                                |
|----------------|--------------------|--------------------------------------|
| `ASCII`        | parsed to `i32`/`f64` | comma separated, one row per sample |
| `BINARY`       | `i16`              | little endian                        |
| `BINARY32`     | `i32`              | little endian (2013)                 |
| `FLOAT32`      | `f32`              | little endian (2013)                 |

Binary record layout: `u32` sample number, `u32` timestamp, analog values,
then binary channels packed in 16-bit words (LSB = first channel).

### Values and time

- **Scaling:** engineering value = `a * raw + b`. Primary/secondary conversion
  uses the `primary`, `secondary` and `PS` fields of each analog channel.
  The user can toggle primary/secondary view.
- **Missing values:** `0x8000` for `BINARY`, `0x80000000` for `BINARY32`, empty
  field for `ASCII`. Missing samples are shown as gaps, not as zero.
- **Timestamps:** when the sample rate in `.cfg` is non-zero, time is computed
  from sample number and rate(s). When it is `0`, the timestamp column
  × `timemult` (µs) is used. A missing timestamp (`0xFFFFFFFF`) falls back to the rate.
- **Multiple sample rates** (`nrates > 1`) must be supported.
- **Time axis** is shown relative to the trigger time (t = 0 at trigger) by
  default, with an option for absolute time.

## Command line interface

Argument parsing uses [clap] (derive API).

```
sprcmtrd                       # start TUI in current directory
sprcmtrd [PATH]                # start TUI with PATH (file or directory) opened
sprcmtrd info <FILE>           # print summary of a recording
sprcmtrd list [DIR]            # list recordings in DIR (default: .)
sprcmtrd view <FILE>           # print summary and terminal plots to stdout
sprcmtrd stats <FILE>          # min, max, mean, RMS, peak; binary changes
sprcmtrd export <FILE>         # samples as CSV or JSON
sprcmtrd cut <FILE> -o <OUT>   # time window / channels as a new recording
sprcmtrd convert <FILE> -o <OUT> # write as another revision / format / .cff
```

Global options:

| Option              | Meaning                                         |
|---------------------|-------------------------------------------------|
| `--theme <dark|light|auto>` | TUI theme (default `auto`)              |
| `--secondary`       | Show values in secondary instead of primary     |
| `--no-color`        | Disable colors (also honors `NO_COLOR` env var) |
| `-v`, `--verbose`   | More log output (to stderr / log file)          |
| `-h`, `--help`, `-V`, `--version` | standard                          |

`<FILE>` may be any file of a recording (`.cfg`, `.dat`, `.cff`, …); see §2.2.

### Exit codes

| Code | Meaning                                  |
|------|------------------------------------------|
| 0    | Success                                  |
| 1    | Generic error                            |
| 2    | Invalid command line arguments (clap)    |
| 3    | File not found / not readable            |
| 4    | Invalid or unsupported COMTRADE content  |

### `info` command

Prints a summary of one recording:

- Station name, device id, revision, file format (`ft`)
- Start time, trigger time, duration
- Line frequency, sample rate(s), total number of samples
- Number of analog and binary channels
- Table of analog channels: index, id, phase, circuit, unit, a, b, primary/secondary, min/max
- Table of binary channels: index, id, phase, circuit, normal state
- Content of `.hdr` / `.inf` if present (can be skipped with `--no-extra`)

Option `--json` prints the same data as JSON for scripting.

### `list` command

Searches for recordings in a directory and prints one row per recording.

- Arguments: `[DIR]` (default `.`), `--no-recursive`, `--filter <REGEX>` (on file name),
  `--valid`.
- Subdirectories are searched recursively by default; `--no-recursive` limits the
  search to `DIR`. Hidden directories and symbolic links to directories are skipped
  (no endless loops on link cycles). File names are shown relative to `DIR`.
- A recording is identified by its `.cfg` (or `.cff`). A `.cfg` without a `.dat` is
  listed with a warning marker.
- Columns: file name, station, revision, data format (ASCII, BINARY, BINARY32,
  FLOAT32), start time, duration, sample rate, #analog, #binary, `.dat` size.
- Only the `.cfg` is parsed (fast). The `.dat` is not read.
- Search runs in the background (own thread); found recordings are sent to the
  output as they are found.
- In a terminal without `--sort`, rows are printed as they are found (search
  order) with fixed column widths: the file column takes what the terminal width
  leaves (long paths keep their end), errors follow the file name, and lines are
  cut at the terminal width instead of wrapping.
- With `--sort name|time|size`, or when stdout is not a terminal (pipes,
  scripts), the rows are sorted (by start time by default) and printed as one
  aligned table at the end.
- While searching, a spinner with the number of recordings found and the
  directory being searched is shown on stderr (only when stderr is a terminal);
  it is cleared when the search ends.
- Files that fail to parse are listed with an error column instead of being dropped.
- `--valid` shows only recordings that load completely, as the viewer would open
  them: the `.cfg` parses, the `.dat` exists and its samples parse. This reads the
  `.dat` of every recording, so it is slower than the default listing.
- Recursive search in children directories is enabled by default.

### `view` command

Non-interactive view printed to stdout (useful for logs and reports).

- Arguments: `<FILE>`, `--channels <LIST>` (indices or ids, default all),
  `--from <t>` / `--to <t>` (time window in ms relative to trigger),
  `--width <cols>` / `--height <rows>` (default: terminal size).
- Output: short summary (like `info`, without channel tables), then one line plot
  per selected analog channel and one status strip per binary channel.
- Plots are rendered with textplots (no extra plotting crate).


### `stats` command

Statistics per channel over a time window (alias `stat`).

- Arguments: `<FILE>`, `--channels <LIST>` and `--from` / `--to` as in `view`,
  `--json`; the global `--secondary` selects secondary values.
- Analog channels: min and max with the time of their first occurrence, mean
  (DC part), RMS over the window (true RMS of all samples, not of the
  fundamental), peak (value with the largest magnitude, with sign) and its time,
  number of missing samples (left out of all other values).
- Binary channels: state at the first and last sample, number of changes, time
  of the first change, time spent in state 1 (each sample counts until the next).
- Text output: window and side, then one table for analog and one for binary
  channels; values with 4 significant digits and SI prefix, times relative to
  the trigger. JSON: `file`, `side`, `from_s`, `to_s`, `samples`, `analog[]`,
  `binary[]` with full values (times in s).
- Results are rounded to 12 significant digits, and a mean below 1e-12 of the
  peak counts as 0, so float noise doesn't show.
- Library: `comtrade::stats` (`analog`, `binary`).

### `export` command

Writes samples as CSV or JSON for spreadsheets and scripts (Excel, Python,
gnuplot).

- Arguments: `<FILE>`, `--format csv|json` (default: `json` when `--output`
  ends in `.json`, otherwise `csv`), `--channels <LIST>` and `--from` / `--to`
  (as in `view`), `--every N` (every Nth sample, plain picking without
  filtering), `--absolute`, `-o, --output <FILE>` (default stdout). The global
  `--secondary` selects secondary values.
- Library: `comtrade::export` (`Export` with the selected channels, sample
  range, `every`, side and time column; `Export::write` with `Format::Csv` /
  `Format::Json`). The command resolves the options and handles the output file.
- Values are scaled (`a * raw + b`, converted to the selected side) and rounded
  to 12 significant digits, so scaling noise doesn't show. Binary channels are
  `0` / `1`.
- CSV: header row `sample,time_ms,<id> [<unit>]…,<binary id>…`; one row per
  sample; `sample` is the 1-based sample number, `time_ms` the time relative to
  the trigger. With `--absolute` the column is `time` with date and time of day
  (`2024-03-15T08:30:00.060000`). Missing values are empty fields; fields with
  commas or quotes are quoted.
- JSON: one object with `station`, `device`, `revision`, `start_time`,
  `trigger_time`, `side`, then one array per column: `time_ms` (or `time`),
  `sample`, `analog[]` (`index`, `id`, `phase`, `circuit`, `unit`, `values` with
  `null` for missing) and `binary[]` (`values` of 0/1).
- `--output` replaces an existing file but never a file of the source recording.
  A summary (`wrote N samples × M channels to FILE`) goes to stderr. Writing to
  a closed pipe (`| head`) ends quietly.

### `cut` command

Writes a time window and/or some channels of a recording as a new, smaller
recording.

- Arguments: `<FILE>`, `-o, --output <OUT>`, `--from` / `--to` (ms relative to
  the trigger) and `--channels <LIST>` as in `view`, `--force`.
- The output keeps the source's revision and data format and is written like
  `convert` writes: `.cfg` (+ `.dat`, `.hdr`, `.inf`) or `.cff` (which chooses
  revision 2013), `--force` for existing files, never over the source, read
  back before success. Channels are numbered from 1 again in the given order.
- Times: the start time moves to the first kept sample, the trigger time stays,
  so times relative to the trigger are the same as in the source (the trigger
  may be outside the window; this is reported). Sample rate segments are
  counted again for the kept samples.
- A window without samples is an error that names the recording's time range.
- Library: `Comtrade::slice(samples, analog, binary)` builds the new recording
  in memory; the command writes it with the `convert` code.

### `convert` command

Writes a recording as another revision, data format or file layout.

- Arguments: `<FILE>`, `-o, --output <OUT>`, `--format ascii|binary|binary32|float32`,
  `--rev 1991|1999|2013`, `--force`.
- The output extension selects the layout: `NAME.cfg` writes `NAME.cfg` +
  `NAME.dat` (plus `.hdr` / `.inf` when the source has them; sibling extensions
  follow the case of the given one), `NAME.cff` writes one combined file.
- Defaults: the source format and revision. Revision 2013 is chosen when the
  format (BINARY32, FLOAT32) or the `.cff` layout needs it; an explicit `--rev`
  that can't hold them is an error (exit 1).
- Values: raw values are copied when the target format holds them exactly
  (integers that fit). Otherwise integer targets are rescaled: the raw values
  are multiplied so the channel's peak uses the full range (±32767 / ±2147483647;
  the minimum is the missing value marker), `a` is divided by the same factor
  and min/max are set to the range, so the scaled values stay the same within
  the new resolution. FLOAT32 rounds to 32-bit floats. Missing values stay
  missing (2013 ASCII: empty field, older: `99999`).
- Timestamps are written from the sample times (in `timemult` µs; 1991 has no
  `timemult`, so µs). Sample rates, start and trigger time are kept.
- Revision 1991 has no transformer ratios, primary/secondary flag, binary
  channel phase and circuit, `timemult` or time codes, and two-digit years
  (1970–2069). 1999 has no time codes. Fields that are left out are reported.
  Upgrading to 2013 writes `0,0` for time codes the source doesn't have.
- Every change that loses information (rescaled channels with their new
  resolution, float rounding, dropped fields) is printed as a `note:` line.
- Safety: existing output files are only replaced with `--force`; files of the
  source recording are never overwritten. The result is read back with the
  parser before the command reports success.
- Library: `comtrade::write` (`encode`, `cfg_text`, `Encoded::cff`) builds the
  file contents; the command writes them to disk.

### `completions` command

Generates tab-completion scripts for your shell.

- Arguments: `<SHELL>` (bash, zsh, fish, powershell), `--output <FILE>` (default stdout).
- Example: `sprcmtrd completions bash --output /etc/bash_completion.d/sprcmtrd`
- Completions for file paths in current directories. Recursively supports subdirectories.
- Implementation: `clap_complete` generates the script from the clap definitions,
  so subcommands, options and values (`--theme`, `--sort`, shells) are always in
  sync. File arguments use `ValueHint::FilePath`, `list DIR` uses
  `ValueHint::DirPath` and the TUI `PATH` `ValueHint::AnyPath`; the shell completes
  paths and steps into subdirectories as you type (bash falls back to its file
  completion via `complete -o default`).
- Elvish is supported too (part of `clap_complete::Shell`).
- `--output` errors name the path; a missing directory exits with code 3 (file
  not found), other write errors with 1. On success a confirmation is printed to
  stderr and stdout stays empty.
 

## Architecture

```
src/
  main.rs            entry point: parse args, set up logging, dispatch
  lib.rs             library root, re-exports public API
  args.rs            clap definitions (Cli, Command, global options)
  error.rs           error types (thiserror)
  config.rs          application configuration (parsing, loading, defaults)
  logging.rs          logging setup (env_logger, log levels)
  comtrade/
    mod.rs           Comtrade, AnalogChannel, BinaryChannel, public API
    cfg.rs           .cfg parser (all revisions)
    dat.rs           .dat reader (ASCII, BINARY, BINARY32, FLOAT32)
    cff.rs           .cff splitter (2013)
    write.rs         .cfg / .dat / .cff writer (convert)
    export.rs        samples as CSV / JSON (export)
    stats.rs         channel statistics (stats)
    discover.rs      find sibling files / recordings in a directory
  command/
    mod.rs           Command::run dispatch
    info.rs
    list.rs
    view.rs
    stats.rs        Channel statistics
    export.rs       Samples as CSV / JSON
    cut.rs          Time window / channels as a new recording
    convert.rs      Write a recording as another revision / format
    completions.rs  Generate tab-completion scripts for your shell
  tui/
    mod.rs           run(): terminal setup/teardown, event loop
    app.rs           App state, actions, key handling
    event.rs         event stream (keys, resize, tick, background load results)
    theme.rs         dark / light theme
    widgets/
      header.rs
      channels.rs    channel list sidebar
      plot.rs        analog line plot + binary status strip
      footer.rs
      filepicker.rs  overlay
      info.rs        recording info overlay
      help.rs        key help overlay
tests/
  data/              sample COMTRADE files (each revision and format)
  cli.rs             integration tests for commands
```

Layering rule: `comtrade` knows nothing about `command` or `tui`. It is a plain,
synchronous library and could be published as its own crate later.

## Comtrade module

### Data model

```rust
pub struct Comtrade {
    pub config: Config,              // parsed .cfg
    pub header: Option<String>,      // .hdr text
    pub info: Option<String>,        // .inf text
    analog: Vec<AnalogChannel>,
    binary: Vec<BinaryChannel>,
    time: Vec<f64>,                  // seconds relative to trigger
}

pub struct Config {
    pub station_name: String,
    pub device_id: String,
    pub revision: Revision,          // Rev1991 | Rev1999 | Rev2013
    pub line_frequency: f64,
    pub sample_rates: Vec<SampleRate>,  // (rate_hz, last_sample)
    pub start_time: NaiveDateTime,
    pub trigger_time: NaiveDateTime,
    pub data_format: DataFormat,     // Ascii | Binary | Binary32 | Float32
    pub time_mult: f64,
    pub time_code: Option<String>,   // 2013
    pub local_code: Option<String>,  // 2013
    pub analog: Vec<AnalogInfo>,
    pub binary: Vec<BinaryInfo>,
}

pub struct AnalogChannel {
    pub info: AnalogInfo,            // id, phase, circuit, unit, a, b, skew, min, max, primary, secondary, ps
    samples: Samples,                // raw values in file type
}

pub enum Samples { I16(Vec<i16>), I32(Vec<i32>), F32(Vec<f32>) }

pub struct BinaryChannel {
    pub info: BinaryInfo,            // id, phase, circuit, normal_state
    samples: BitVec,                 // one bit per sample
}
```

Raw values are kept in their native type to save memory; scaled values are
computed on demand (`AnalogChannel::value(i) -> Option<f64>`,
`AnalogChannel::values() -> impl Iterator<Item = Option<f64>>`).

### Public API

- `Comtrade::from_cfg(path)` — parse configuration only (used by `list`).
- `Comtrade::open(path)` — find sibling files, parse `.cfg`, read `.dat`, read
  `.hdr`/`.inf` if present. Accepts `.cfg`, `.dat` or `.cff`.
- `Comtrade::from_readers(cfg, dat)` — parse from any `Read` sources (for tests).
- `analog_channels()` / `binary_channels()` — iterators.
- `time()` — slice of time values.
- `discover::find_recordings(dir, recursive) -> Vec<RecordingPath>`.

### Parsing rules

- Be tolerant where the standard is often broken in practice: trailing spaces,
  missing trailing fields, `\r\n` or `\n` line endings, Latin-1 text in old files.
- Be strict on structure: wrong channel counts or a `.dat` whose size does not
  match `#samples × record size` produce an error with file name and line number.
- `.dat` files are read with a buffered reader or memory map (`memmap2`) to
  handle large files.

### Errors

One error type `comtrade::Error` (via `thiserror`) with variants such as
`Io`, `MissingFile`, `Cfg { line, message }`, `Dat { sample, message }`,
`Unsupported`. Commands use `anyhow` for context.


## TUI module

`tui::run(initial_path)` sets up the terminal (raw mode, alternate screen),
runs the event loop and always restores the terminal, also on panic
(install a panic hook).

### Event loop

- `tokio::select!` over crossterm `EventStream`, a tick interval (for status
  messages / spinners) and a channel for background task results.
- Loading a `.dat` runs in `tokio::task::spawn_blocking` so the UI stays
  responsive; the footer shows a loading indicator.
- Redraw only when state changed or the terminal was resized.

### Layout

```
┌ sprcmtrd v0.1.0 ─────────────────── fault_01.cfg · SUBSTATION A · 1999 · 4.8 kHz ┐
│ Channels        │ IA [A]  ╭─╮   ╭─╮   ╭─╮                                         │
│ ▶ IA       A    │        ─╯ ╰─╮ │ ╰─╮ │ ╰──                                       │
│   IB       A    │ IB [A]  ...                                                     │
│   IC       A    │ VA [kV] ...                                                     │
│   VA       kV   │                                                                 │
│ ─ binary ─      │ TRIP    ▁▁▁▁▁▁▁▁████████████▁▁▁▁▁                               │
│   TRIP          │ CB_OPEN ▁▁▁▁▁▁▁▁▁▁▁▁▁████████████                               │
│   CB_OPEN       │        -20ms     0      20ms     40ms    cursor: 12.4 ms        │
├─────────────────┴─────────────────────────────────────────────────────────────────┤
│ o open  i info  +/- zoom  ←/→ pan  space toggle  ? help  q quit     loaded 2.1 MB │
└───────────────────────────────────────────────────────────────────────────────────┘
```

- **Header:** left: app name and version (big text only if terminal is ≥ 100 columns,
  otherwise one line); right: current file, station, revision, sample rate.
- **Sidebar:** channel list (analog, then binary) with unit; selected channels
  are drawn in the plot area. Can be hidden.
- **Plot area:** analog line plots stacked vertically, binary status strips below,
  one shared time axis with a vertical cursor and trigger marker.
- **Footer:** key hints for the current mode and status (loading, errors, cursor values).
- **Overlays:** file picker, info, help. Drawn on top of the main layout with a
  cleared background.
- **Small terminals:** minimum supported size 80×24. Below that, show a
  "terminal too small" message instead of a broken layout.

### Plots

- Analog: ratatui `Chart` with Braille markers. Each channel has its own Y axis
  scaled to its visible min/max; the unit is shown in the label.
- Binary: one-row status strip per channel (`▁` = 0, `█` = 1); states different
  from the normal state are highlighted.
- Downsampling: when there are more samples than horizontal cells, keep
  min and max per column (min/max decimation) so peaks are not lost.
- Zoom and pan act on the shared time axis for all channels at once.
- Cursor: shows time and value of every visible channel at the cursor position.
- Always show cursor time in milliseconds on the plot time axis
- Analog plots alternate background shades so neighbouring channels are easy to
  tell apart: 1st, 3rd, ... lighter, 2nd, 4th, ... darker (whole row, gutter
  included). One shade is the theme background, the other a subtle gray from the
  256-color palette (234 on dark, 254 on light). No shading with `--no-color`.

### Key bindings

| Key                 | Action                                   |
|---------------------|------------------------------------------|
| `q`, `Ctrl+c`       | Quit (closes overlay first if one is open) |
| `o`, `Ctrl+p`       | Open file picker                         |
| `i`                 | Toggle info overlay                      |
| `?`                 | Toggle help overlay                      |
| `Tab`               | Switch focus sidebar ↔ plot              |
| `↑`/`↓`, `k`/`j`    | Move selection in sidebar                |
| `Space`             | Show / hide selected channel             |
| `a`                 | Show all / hide all channels             |
| `+`/`-`             | Zoom in / out (time)                     |
| `←`/`→`, `h`/`l`    | Move cursor; with `Shift` pan the view   |
| `0`                 | Reset zoom (show whole recording)        |
| `t`                 | Jump cursor to trigger                   |
| `p`                 | Toggle primary / secondary values        |
| `T`                 | Toggle dark / light theme                |
| `Ctrl+,`, `c`       | Open configuration overlay               |
| `Esc`               | Close overlay                            |

Note: `Ctrl+Shift+P` is not used because most terminals cannot tell it apart from
`Ctrl+P`. Key bindings are defined in one table in `app.rs` so footer hints and
help overlay are generated from it.

### File picker overlay

- Opens with `o` / `Ctrl+p`; starts in the directory of the current file (or `.`).
- Text input at top; list below: immediate subdirectories first (for navigation),
  then recordings from the directory **and all its subdirectories** (recursive by
  default, same rules as `list`: hidden directories and directory symlinks are
  skipped). Recordings are shown with their path relative to the picker directory.
- `Ctrl+r` toggles between recursive search and this directory only; the choice is
  kept when changing directories.
- `Ctrl+b` toggles between all recordings (default) and valid recordings only, the
  same check as `list --valid` (configuration and data load completely). The
  choice is kept when changing directories; the title shows `valid only` while it
  is on. Toggling restarts the scan, which then reads every `.dat` and is slower.
- Filtering on the relative path: case-insensitive substring by default; regex when the query starts
  with `/` (invalid regex shows an error in the overlay, list stays unchanged).
- Each row: file name, station, start time, #channels (from `.cfg` only).
- `↑`/`↓` select, `Enter` open, `Backspace` on empty input goes to parent
  directory, `Esc` close.
- Directory scan runs in the background; results appear as they arrive. Changing
  directory, toggling `Ctrl+r` / `Ctrl+b` or closing the picker cancels a running scan.
- Recursive search in subdirectories.

### Configuration overlay

- Opens with `Ctrl+,`; shows current configuration for theme and key bindings.
- `Esc` closes the overlay.
- Changes take effect immediately and are saved to the configuration file.
- Configuration file is located at `~/.config/sprcmtrd/config.toml`.
- If configuration file does not exist, it will be created with default values.

Layout (centered, 75 % × 85 %, title shows the configuration file path):

```
┌ Configuration · ~/.config/sprcmtrd/config.toml ─────────────────────────┐
│Appearance                                                               │
│  Theme           ◀ dark ▶            dark or light background           │
│                                                                         │
│Key bindings                                                             │
│  quit            q, Ctrl+c           Quit (closes an open overlay first)│
│▌ zoom_in         press a key…        Zoom in (time)                    ▐│
│  ...                                                                    │
│                                                                         │
│saved to ~/.config/sprcmtrd/config.toml                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

- `↑`/`↓` (`k`/`j`) select a row; the list scrolls with the selection.
- Theme row: `Enter`, `Space`, `←`/`→` switch dark / light.
- Binding row: `Enter` waits for the new key (`Esc` cancels) and replaces the
  action's keys with it; a key already used by another action is refused with a
  message. `Backspace` / `Delete` restores the default keys.
- The bottom line shows the result of the last change (saved, conflict, error) or
  a hint for the selected row.
- The file is rewritten with all settings (theme and every binding); comments in
  it are not kept. `--theme` and `--no-color` still override the saved theme.
- Without a configuration directory, changes apply for the session and the
  overlay says they are not saved.
- `Ctrl+,` cannot be sent by most terminals with the legacy key encoding. The TUI
  enables the kitty keyboard protocol (disambiguate escape codes) when the
  terminal supports it (kitty, WezTerm, foot, Ghostty, recent Alacritty) and
  turns it off again on exit and on panic. `c` opens the overlay in all terminals.

### Theme

- Two themes, dark and light, defined in `theme.rs` as a `Theme` struct with
  named roles (background, text, muted, border, accent, selection, warning, error,
  trigger marker, and a palette of ≥ 8 colors for channels).
- Uses the 16 ANSI colors by default so it follows the user's terminal color scheme.
- `--theme auto` picks dark unless the terminal reports a light background.
- `T` switches theme at runtime. `--no-color` / `NO_COLOR` uses no colors, only
  modifiers (bold, reverse).
- Channel colors follow standard phase colors where the phase field is known
  (A/B/C/N), otherwise the palette in order.

## Command module

`command::Command` (from `args.rs`) has an `async fn run(self) -> anyhow::Result<()>`
that dispatches to `info::run`, `list::run`, `view::run`, or `tui::run` when no
subcommand is given. CLI commands do synchronous work; only the TUI needs the
async runtime.

### Completions


* Supports various shells like Bash, Zsh, Fish, and PowerShell.


## Configuration

Use configuration file at `~/.config/sprcmtrd/config.toml` for theme and key bindings.

## Logging

* Write log in files if running in TUI mode.


## Dependencies

| Crate                 | Use                          |
|-----------------------|------------------------------|
| `ratatui`, `crossterm` (with `event-stream`) | TUI       |
| `tokio`               | runtime, `select!`, `spawn_blocking` |
| `futures`             | `StreamExt` for `EventStream` |
| `clap` (derive)       | argument parsing             |
| `chrono`              | date/time parsing            |
| `thiserror`, `anyhow` | errors                       |
| `regex`               | file picker / list filter    |
| `serde`, `serde_json` | `info --json`                |
| `memmap2`             | large `.dat` files (optional)|
| `tracing`, `tracing-subscriber` | logging to file (never to the TUI screen) |
| `textplots`              | plotting in the terminal       |

## Testing

- Unit tests for the `.cfg` parser for each revision, including malformed lines.
- Unit tests for the `.dat` reader for each data format, missing values and
  multiple sample rates.
- Sample files in `tests/data/` (small, generated or public examples with a known license).
- Integration tests for `info`, `list`, `view` with `assert_cmd` and snapshot
  tests (`insta`) of the output.
- TUI rendering tests with ratatui `TestBackend` and snapshot tests of the buffer.
- Performance check: open a 100 MB `BINARY` file and show the first frame in < 2 s.

## Milestones

1. **Parser:** `.cfg` + `.dat` (1999, ASCII + BINARY), `info` command.
2. **Discovery:** `list` command, sibling file detection.
3. **Plots:** `view` command with off-screen ratatui rendering.
4. **TUI:** layout, channel list, plots, zoom/pan/cursor, help.
5. **TUI extras:** file picker, info overlay, themes.
6. **Full standard:** 1991 and 2013 revisions, BINARY32, FLOAT32, `.cff`.

## Open questions

- Should `view` support exporting plots (SVG/PNG) or stay text-only?
- Should time windows accept sample numbers in addition to milliseconds?

[ratatui]: https://ratatui.rs
[clap]: https://docs.rs/clap
