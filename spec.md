# sprcmtrd — specification

## Overview

`sprcmtrd` is a terminal viewer for COMTRADE files, written in Rust. It has two
faces:

- **CLI commands** (`info`, `list`, `check`, `view`, `events`, `stats`, `phasors`, `trend`, `harmonics`, `impedance`, `export`, `cut`, `convert`) that print to stdout and can be used in scripts.
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

### Instructions

- Project language: Rust, English
- Do not use number in markdown headings

### Goals

- Open and display COMTRADE recordings quickly, also large ones (≥ 100 MB `.dat`).
- Give a readable summary of a recording without leaving the terminal.
- Plot analog channels and binary channel states with zoom and pan.
- Analyse recordings: statistics, binary events, phasors and symmetrical
  components, and further signal analysis (harmonics, impedances, …).
- Work over SSH and in plain terminals (no GUI dependencies).

### Non-goals (for v1)

- Editing COMTRADE files. Writing is limited to `convert` and `cut` (a new
  recording in another revision, format or layout, or a part of one).
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
sprcmtrd check <PATH>          # validate recordings (exit code 4 on errors)
sprcmtrd events <FILE>         # timeline of binary channel changes
sprcmtrd stats <FILE>          # min, max, mean, RMS, peak; binary changes
sprcmtrd phasors <FILE>        # phasors and symmetrical components at a time
sprcmtrd trend <FILE>          # the same over time, cycle by cycle
sprcmtrd harmonics <FILE>      # harmonic spectrum and THD at a time
sprcmtrd impedance <FILE>      # apparent impedance of the fault loops
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
  `--valid`, and the content filters below.
- Content filters, to search large archives by what the recordings contain:
  - `--station <REGEX>`: station name, case-insensitive.
  - `--since <DATE>` / `--until <DATE>`: start time as written in the
    recording (no time zone conversion). `2024-03-15`, `2024-03-15 08:30`,
    `2024-03-15T08:30:00(.fff)`; a date alone means the start of the day for
    `--since` and its end for `--until`, so one date for both is that day.
  - `--min-duration <D>` / `--max-duration <D>`: `500ms`, `2s`, `1.5min`, `1h`;
    a bare number is ms (like `--from` / `--to`).
  - They need the configuration: recordings that fail to parse, and for the
    duration filters recordings without a known duration (timestamps only),
    are left out when such a filter is set.
  - Invalid dates and durations are command line errors (exit 2), an invalid
    `--station` regex exits with 1 (like `--filter`). Nothing found:
    "no COMTRADE recordings matching the filters found in DIR" (exit 0).
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


### `check` command

Validates recordings, for scripts and CI.

- Arguments: `<PATH>` (a recording file, or a directory: all recordings in it
  and its subdirectories, same rules as `list`), `--no-recursive`, `--strict`,
  `--json`.
- Findings have a severity:
  - **error**: the recording can't be read (completely or correctly): the
    configuration doesn't parse, the data file is missing, its size doesn't fit
    the record size, the sample count differs from the configuration, …
  - **warning**: readable, but something is wrong: missing values (count and
    first sample), raw values outside the declared min/max or exactly at it
    (clipped?), time not increasing (timestamps out of order), channel numbers
    not 1..n, duplicate or empty channel ids, no unit, multiplier `a` = 0,
    transformer ratio 0 (1999+), min above max, no declared sample count,
    trigger outside the recording.
  - **note** (doesn't affect the status): unusual unit, line frequency other
    than 50 / 60 / 16.7 Hz.
- Text output: a table `status | file | channel | finding` with one row per
  finding (the file named on its first row only) and one `✓ OK` row for a
  recording without findings; files relative to the directory. For a directory
  a summary line (`9 recordings: 7 ok, 1 with warnings, 1 with errors`).
- Colors, for accessibility only an extra cue: every status has a word and a
  symbol (`✓ OK`, `i NOTE`, `! WARNING`, `✗ ERROR`). OK uses the accent color
  (cyan on dark, blue on light backgrounds) instead of green, so OK and errors
  don't differ by red / green alone; warnings are yellow (dark) or magenta
  (light, yellow is hard to read on white); errors bold red; notes gray; the
  header bold. The 16 ANSI colors follow the terminal's scheme; light or dark
  comes from `--theme` / the configuration / `COLORFGBG`, like the TUI. Colors
  only on a terminal, never with `--no-color` or `NO_COLOR`
  (`term::Palette`).
- JSON: an array of `{file, status, issues: [{severity, channel, message}],
  samples}`.
- For a directory, a spinner on stderr shows the search and the recording
  being checked (only when stderr is a terminal).
- Exit code 4 when a recording has an error, or a warning with `--strict`;
  0 otherwise (also for a directory without recordings, with a message). A
  missing path exits with 3.
- Library: `comtrade::check::check(&RecordingPath) -> Report`.

### `events` command

Timeline of binary channel changes (start, trip, breaker open, reclose, ...).

- Arguments: `<FILE>`, `--channels <LIST>` (binary channels: `dN` or id;
  naming an analog channel is an error), `--from` / `--to` as in `view`,
  `--json`.
- A change is reported at the first sample with the new state; the first
  sample of the window is the starting state. Changes at the same sample are
  listed in channel order.
- Text output: one row per change with the time relative to the trigger
  (`+60.0 ms`), `Δ` since the previous row, date and time of day, channel
  (`D5 TRIP_B`) and change (`0 → 1`); the trigger is a row of its own when it
  lies in the window, so operate times (start → trip → breaker open) can be read
  from the `Δ` column. Ends with the number of changes and channels; "No
  changes" when there are none.
- JSON: `file`, `trigger_time`, `from_s`, `to_s`, `events[]` with `sample`
  (1-based), `time` (s), `index`, `id`, `from`, `to` and `absolute`.
- Library: `analysis::events::events`.

### `impedance` command

Apparent impedance of the six fault loops, as a distance relay measures it.

- Arguments: `<FILE>`, `--at <ms>` (default 0), `--cycles <N>` (default 1),
  `--voltage <A,B,C>` and `--current <A,B,C>` (both or neither), `--k0
  <MAG[@ANGLE]>` (default 0), `--loops <LIST>` (`AN,BN,CN,AB,BC,CA`, default
  all), `--trend` with `--from` / `--to` / `--step` (as in `trend`),
  `--frequency <Hz>`, `--format text|csv|json`; global `--secondary`.
- Channels: without `--voltage` / `--current`, the first voltage A/B/C group
  (unit V, kV, mV, MV) and a current group (A, kA, mA), preferably of the same
  circuit (`analysis::impedance::find_pair`). Without them it is an error that
  names the options.
- Loops from the fundamental phasors (same window as `phasors`), scaled to V and
  A: phase to ground Z = V / (I + k0·3I0) with 3I0 = I_A + I_B + I_C, phase to
  phase Z = (V₁ − V₂) / (I₁ − I₂). k0 = (Z0 − Z1) / (3·Z1) of the line; 0 means
  no compensation, which the output says. Ω are primary, or secondary with
  `--secondary` (each channel converted with its ratios).
- At one moment: a table `loop | V | I | R | X | |Z| | φ` with the loop voltage
  and current in the channels' units, so it can be judged whether a loop
  carries fault current; no loop current gives a row without values.
- `--trend`: R and X of each loop per step (text, CSV `time_ms,BN_r_ohm,…`,
  JSON), the trajectory to compare with relay zones.
- R or X below 1e-6 of |Z| is shown as 0 (residue of integer samples).
- Library: `analysis::impedance` (`Loop`, `loop_quantities`, `impedance`,
  `find_pair`, `unit_factor`).

### `harmonics` command

Harmonic spectrum and total harmonic distortion per channel at one moment.

- Arguments: `<FILE>`, `--at <ms>` (window end relative to the trigger,
  default 0), `--cycles <N>` (window length, default 1), `--channels <LIST>`
  (analog; naming a binary channel is an error), `--max-order <N>` (default
  50, at least 2), `--frequency <Hz>`, `--angles`, `--json`; global
  `--secondary`.
- Method: DFT at each order of the fundamental over the same window as
  `phasors` (`analysis::phasor::dft`); exact over whole cycles. The highest
  order is limited to below half the sample rate (40 samples per cycle → 19th);
  the output says so when `--max-order` is above it.
- Values: the fundamental as RMS; DC (mean over the window) and each harmonic
  in % of the fundamental peak; THD = RMS of the harmonics 2…max / fundamental,
  labelled with the orders it covers (`THD 2–19`). Angles (with `--angles`)
  relative to the channel's fundamental shifted by the order, φₕ − h·φ₁, so they
  describe the waveform's shape wherever the window starts; `-` for a harmonic
  below 0.05 %.
- Text: header lines (window, orders, side) and a table with a row per value
  (fundamental, DC, 2 … max, THD) and a column per channel (unit in the
  header). A channel with a missing value in the window shows `-` and a note.
- JSON: `file`, `side`, `frequency`, `from_s`, `at_s`, `samples`, `max_order`,
  `channels[]` with `fundamental`, `dc`, `thd`, `harmonics[]` (`order`,
  `magnitude` RMS, `percent`, `angle`) or `error`.
- Library: `analysis::harmonics` (`spectrum`, `max_order`, `Spectrum`).

### `frequency` command

System frequency over time and its rate of change (alias `freq`).

- Arguments: `<FILE>`, `--channel <CH>` (analog; default the first channel
  with a voltage unit, else the first analog channel; a binary channel is an
  error), `--method crossings|phasor` (default crossings), `--from` / `--to`,
  `--step` (as in `trend`), `--cycles <N>` (measurement length, default 1;
  whole with crossings), `--frequency <Hz>` (nominal, default the line
  frequency), `--format text|csv|json`; global `--secondary`.
- Crossings: rising zero crossings, linearly interpolated, with hysteresis
  (armed below −5 % of the channel's peak); f = N / (time from the first to the
  last of the N + 1 crossings up to the row). None across a missing value or
  when the last crossing is more than 1.5 nominal periods before the row.
- Phasor: f = f₀ + Δφ / (2π Δt) between the windows of N cycles ending at the
  row and N cycles earlier, repeated twice at the frequency found with
  windows of whole cycles of it (frequency tracking). None when a phasor is
  below 5 % of the channel's peak.
- ROCOF: against the measurement of the N cycles before, over the time between
  the middles of both measurements.
- Text: header lines (channel, method, nominal, min / max / mean frequency with
  times, largest ROCOF) and a table `time | f | Δf | df/dt`. CSV:
  `time_ms,frequency_hz,deviation_hz,rocof_hz_per_s`. JSON: `channel`,
  `method`, `nominal`, `cycles`, `samples`, `step_samples`, `summary`
  (`min`, `max`, `rocof` as `value` / `time_s`, `mean`) and `points[]`
  (`time_s`, `frequency`, `deviation`, `rocof`). Missing values as in `trend`.
- Library: `analysis::frequency` (`series`, `crossings`, `from_crossings`,
  `from_turn`, `summary`); `trend` uses `from_turn` too.

### `trend` command

Values over time, cycle by cycle: the phasors command for a series of moments.

- Arguments: `<FILE>`, `--channels <LIST>` (analog; naming a binary channel is
  an error), `--from` / `--to` (time range in ms relative to the trigger,
  default the whole recording), `--step <S>` (distance between rows: a bare
  number is cycles, default 1; with a unit a time, `10ms`), `--cycles <N>`
  (window length, default 1), `--reference <CH>` (one of the shown channels;
  angles and frequency refer to it; default the first), `--frequency <Hz>`,
  `--angles`, `--rms`, `--format text|csv|json`; global `--secondary`.
- Each row is the window of `N` cycles ending at the row's time (same method as
  `phasors`); the first row is the first window that fits in the range. The
  number of samples per window and step comes from the sample spacing.
- Columns: time, frequency, per channel the fundamental magnitude (RMS), with
  `--angles` its angle relative to the reference at the same step, with `--rms`
  the true RMS of all window samples (including harmonics and DC). Per
  three-phase group (as in `phasors`) the positive `1`, negative `2` and zero
  `0` sequence magnitudes and the unbalance `2/1` in %; the column label is the
  common start of the group's channel ids (`I1` for IA, IB, IC), else `G1`, ….
- Frequency: from the turn of the reference phasor between consecutive steps,
  f = f₀ + Δφ / (2π Δt); missing for the first row and where the reference is
  zero or missing. Valid while |f − f₀| · Δt < ½ (±25 Hz at a 1-cycle step).
- A sequence component below 1e-6 of the group's largest is shown as 0 (residue
  of integer samples). Missing values: `-` (text), empty (CSV), `null` (JSON).
- Text: header lines (window, step, rows, reference, side) and an aligned
  table. CSV: header `time_ms,frequency_hz,IA_A,IA_deg,IA_rms_A,…,I1_A,…,
  I_unbalance_pct`. JSON: `channels`, `groups`, `steps[]` with `time_s`,
  `frequency`, `phasors`, `rms`, `sequences`.
- Library: `analysis::trend::trend`.

### `phasors` command

Phasors of the fundamental frequency and symmetrical components at one moment
(alias `phasor`).

- Arguments: `<FILE>`, `--at <ms>` (window end relative to the trigger, default
  0), `--cycles <N>` (window length, default 1), `--channels <LIST>` (analog;
  naming a binary channel is an error), `--reference <CH>` (angle reference,
  default the first selected channel; may be outside the selection),
  `--frequency <Hz>` (default the line frequency of the `.cfg`; required when
  it is 0), `--json`; the global `--secondary` selects secondary values.
- Method: DFT over the samples of `N` cycles ending at the sample nearest
  `--at`, as a protection relay sees the signals at that moment. The number of
  samples comes from the sample spacing there (at least 4); the DFT uses the
  sample times, so angles are right with any samples per cycle. DC is rejected
  over whole cycles. Magnitudes are RMS (peak / √2), angles in degrees
  relative to the reference, in (-180, 180].
- Symmetrical components for every group of three selected channels with
  phases A, B, C (also L1/L2/L3, R/S/T) of the same circuit and unit:
  zero (A+B+C)/3, positive (A+aB+a²C)/3, negative (A+a²B+aC)/3 with a = 1∠120°,
  and the unbalance negative / positive in %.
- Text output: window, reference and side, a table `channel | unit | magnitude
  | angle` (angle `-` for a signal that is zero), and a table of symmetrical
  components (a component that is zero shows `0` without angle).
- A channel with a missing value in the window shows the reason instead of a
  value, and its group gets no symmetrical components. A window that starts
  before the recording, too few samples, or a reference that can't be computed
  are errors (exit 1).
- JSON: `file`, `side`, `frequency`, `at_s`, `from_s`, `samples`, `reference`,
  `phasors[]` (`index`, `id`, `unit`, `magnitude`, `angle` or `error`) and
  `sequences[]` (`circuit`, `unit`, `channels`, `zero`, `positive`, `negative`,
  `unbalance`).
- Library: `analysis::phasor` (`Window::ending_at`, `phasor`, `Polar`) and
  `analysis::sequence` (`symmetrical`, `groups`, `sequences`).

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
- Library: `analysis::stats` (`analog`, `binary`).

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

Tab completion for your shell, computed by `sprcmtrd` itself (dynamic
completion of `clap_complete`, feature `unstable-dynamic`).

- On Tab the shell script runs `COMPLETE=<shell> sprcmtrd -- <words>`; `main`
  answers with `CompleteEnv` before anything else runs and exits.
- Subcommands, options and option values (`--theme`, `--sort`, `--format`,
  `--rev`, …) come from the clap definitions, so they are always in sync.
- Recording arguments (`FILE` of `info`, `view`, `events`, `stats`, `export`,
  `cut`, `convert`, `check`, `phasors`, `trend`, `harmonics`, `impedance`, and the TUI `PATH`) use `complete::recordings`:
  - the typed text up to the last `/` is the directory to search (relative,
    absolute or `~/`), the rest the start of a name in it;
  - offered: subdirectories whose name continues the typed text (ending with
    `/`, so the shell steps into them), then the recordings (`.cfg` / `.cff`,
    the main file of each recording) directly in the directory, then those in
    the matching subdirectories and below, all whose path starts with the typed
    text. `.dat`, `.hdr` and other files are not offered;
  - same rules as `list`: hidden directories are skipped unless a name starting
    with `.` is typed, symbolic links to directories are not followed;
  - at most 500 recordings and 200 ms per Tab, so completion in a large tree
    (`~`) stays responsive.
- Output files (`-o`) and `list DIR` keep the plain file / directory completion.
- Shells: bash, zsh, fish, PowerShell, elvish. Recommended setup loads the script
  on shell start (`source <(COMPLETE=bash sprcmtrd)` in `~/.bashrc`,
  `source <(COMPLETE=zsh sprcmtrd)` in `~/.zshrc`,
  `COMPLETE=fish sprcmtrd | source` in fish), so it matches the installed
  binary.
- `completions <SHELL> [--output <FILE>]` prints the same script (calling
  `sprcmtrd` from `PATH`, or the absolute path when started by path), for
  installing it as a file; it has to be written again after an update.
- `--output` errors name the path; a missing directory exits with code 3 (file
  not found), other write errors with 1. On success a confirmation is printed to
  stderr and stdout stays empty.
 

## Architecture

```
src/
  main.rs            entry point: parse args, set up logging, dispatch
  lib.rs             library root, re-exports public API
  args.rs            clap definitions (Cli, Command, one XxxArgs per subcommand,
                     global options); each XxxArgs::run lives in command/xxx.rs
  error.rs           error types (thiserror)
  config.rs          application configuration (parsing, loading, defaults)
  logging.rs          logging setup (env_logger, log levels)
  context.rs         Context: global options and configuration (commands, TUI)
  format/            text formatting shared by the commands and the TUI
    mod.rs           si, time, rate, bytes, table, key_values
    summary.rs       recording summary (info, view, TUI info overlay)
  term/              terminal output shared by the commands and the TUI
    mod.rs           terminal_width, fit
    theme.rs         light / dark background (ThemeKind)
    spinner.rs       progress line on stderr (list, check)
    table.rs         table printed row by row (list)
    style.rs         colors (Palette: check)
  analysis/          signal analysis of loaded recordings (uses comtrade)
    mod.rs           overview, re-exports
    complex.rs       complex numbers for phasors
    phasor.rs        Window, DFT phasor, Polar (phasors)
    sequence.rs      symmetrical components, three-phase groups (phasors)
    stats.rs         channel statistics (stats)
    events.rs        binary channel changes (events)
    trend.rs         values over time, step by step (trend)
    harmonics.rs     harmonic spectrum, THD (harmonics)
    impedance.rs     fault loop impedances (impedance)
  comtrade/
    mod.rs           Comtrade, AnalogChannel, BinaryChannel, public API
    cfg.rs           .cfg parser (all revisions)
    dat.rs           .dat reader (ASCII, BINARY, BINARY32, FLOAT32)
    cff.rs           .cff splitter (2013)
    write.rs         .cfg / .dat / .cff writer (convert)
    export.rs        samples as CSV / JSON (export)
    check.rs         validation of recordings (check)
    discover.rs      find sibling files / recordings in a directory
  command/
    mod.rs           Command::run dispatch
    info.rs
    list.rs
    view.rs
    check.rs        Validate recordings
    events.rs       Timeline of binary channel changes
    stats.rs        Channel statistics
    phasors.rs      Phasors and symmetrical components
    trend.rs        Values over time, cycle by cycle
    harmonics.rs    Harmonic spectrum and THD
    impedance.rs    Apparent impedance of the fault loops
    export.rs       Samples as CSV / JSON
    cut.rs          Time window / channels as a new recording
    convert.rs      Write a recording as another revision / format
    completions.rs  Generate tab-completion scripts for your shell
  tui/
    mod.rs           run(): terminal setup/teardown, event loop
    app.rs           App state, key and action dispatch
    keys.rs          Action, BINDINGS table, KeySpec, KeyMap (configured keys)
    picker.rs        file picker state and background directory scan
    recording.rs     loaded recording: visible channels, zoom, pan, cursor
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

Layering rule: `comtrade` knows nothing about `analysis`, `command` or `tui`. It
is a plain, synchronous library and could be published as its own crate later.
`analysis` builds on `comtrade` only; signal analysis logic lives there and the
commands (and later the TUI) only format its results.

The commands (`command`) and the TUI (`tui`) never use each other: both build on
the shared modules (`comtrade`, `analysis`, `format`, `term`, `context`), and
`main.rs` chooses between them. `tests/layering.rs` checks these rules from the
`crate::…` paths in the sources.

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
`Ctrl+P`. Key bindings are defined in one table in `keys.rs` so footer hints and
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

* Supports bash, zsh, fish, PowerShell and elvish; see the `completions`
  command for the dynamic completion of recordings.


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
