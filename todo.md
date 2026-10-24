# Todo

* [x] Improve command 'list' with option to show only not broken comtrade files can be parsed successfully. 
* [x] Improve file picker to switch between show only working comtrade files and show all files. Use keyboard `ctrl`+ `b` shortcuts for toggling. 

## Command ideas

* [x] `check <FILE|DIR>`: full validation of recordings for scripts and CI. Reads `.cfg` and `.dat` and reports problems: sample count vs `.cfg` end sample, record size, non-monotonic timestamps, missing values (`99999` / `0xFFFF`), clipped values at min/max, unknown units. Exit code 0 when all valid, 4 otherwise; `--json` for machine output.
* [x] `events <FILE>`: timeline of binary channel changes (trip, breaker open/close) with time relative to trigger and absolute time, e.g. `+12.5 ms  TRIP_A  0 → 1`. Options `--channels`, `--from`/`--to`, `--json`.
* [x] `stats <FILE>`: per analog channel min, max, mean, RMS, peak value and its time, in primary or secondary values (`--secondary`); per binary channel number of changes. Options `--channels`, `--from`/`--to`, `--json`.
* [x] `export <FILE>`: write samples as CSV (or JSON) for Excel, Python or gnuplot. Options `--channels`, `--from`/`--to`, `--secondary`, `--every N` (decimation), `--output FILE` (default stdout).
* [ ] `diff <A> <B>`: compare two recordings: header fields, sample rates, channel lists (added, removed, changed scaling or units). Useful when relay settings or a recorder changed.
* [ ] `watch <DIR>`: watch a directory (fault recorder output folder) and print a `list` row for each new recording as it appears; `--exec CMD` runs a command for it (e.g. `sprcmtrd view`).
* [x] `list` filters: `--station REGEX`, `--since DATE`, `--until DATE`, `--min-duration`, so large archives can be searched by recording content, not only by file name.
* [ ] `phasors <FILE> --at <ms>`: fundamental phasors (magnitude and angle) of the analog channels at a time, plus symmetrical components (positive, negative, zero sequence) for three-phase groups. Signal analysis is a non-goal for v1, so this needs a spec update first.

## Commands that write files (non-goal for v1, needs spec decision)

* [x] `cut <FILE> --from <ms> --to <ms> --output <NAME>`: extract a time window (and optionally `--channels`) into a new, smaller recording.
* [x] `convert <FILE>`: convert between data formats (ASCII ↔ BINARY/BINARY32/FLOAT32), revisions (1991/1999 → 2013), and `.cfg`+`.dat` ↔ `.cff`.
* [ ] `rename <DIR>`: rename recordings to a pattern such as `{station}_{start:%Y%m%d_%H%M%S}`, with `--dry-run`.

## TUI ideas

* [ ] Write log to a file in TUI mode (already in spec, not implemented).
* [ ] Two measurement cursors: show Δt and the value difference of each channel between them.
* [ ] Allow more than one key per action in the configuration overlay (rebinding currently replaces all keys).

## Webserver ui ideas

## Tauri ui ideas


## Refactoring ideas

* [ ] Separate TUI and CLI logic more cleanly to reduce coupling and improve testability.
* [ ] App module is too large separate into smaller modules for better maintainability. Move key handling into its own module.
  `tui/app.rs` has 1500 lines. Natural cuts: `Action`, `Binding`, `BINDINGS`, `KeySpec`, `KeyMap` → `tui/keys.rs`; `Picker`, `PickerEntry`, `Matcher` and `scan` → `tui/picker.rs`; `Recording` (channel visibility, zoom, cursor) → `tui/recording.rs`. `App` keeps state and dispatch.

### Command line

* [ ] Shared clap argument groups: `--channels`, `--from` / `--to` are defined five times (view, stats, events, export, cut). One `#[command(flatten)] Selection` struct with `channels()` and `window()` methods instead of passing `select_channels` / `time_window` results around.
* [ ] One `Args` struct per subcommand (`Stats(StatsArgs)`) with its own `run(self, ctx)`, instead of the big `match` in `command/mod.rs` that destructures every variant into separate parameters.
* [ ] Small helpers for repeated code: `GlobalOpts::side()` (the `if secondary { Side::Secondary } else { Side::Primary }` in view, stats, export and the TUI), `Comtrade::name()` (main file name, in view, stats, events), `Comtrade::absolute_time(t)` (trigger time + offset, in export, events, cut and the plot widget), and a serde name for `Side` (`primary` / `secondary`) instead of hand-written matches.
* [ ] Overwrite protection exists twice (`convert::check_outputs`, `export::refuse_source`); move "is this path part of the recording" into the library (`RecordingPath::contains`) and use it in both.
* [ ] Exit code policy: commands mix `bail!` strings (exit 1) with library `Unsupported` errors (exit 4), and `convert` strips the `unsupported:` prefix by hand. Decide which errors are usage errors and give them a type.
* [ ] JSON output: every command has its own structs and naming (`time` vs `time_s`, `from_s`). One documented schema style (units in names, times always in seconds) and a `version` field so scripts can rely on it.
* [x] `list.rs` mixes search, sorting and terminal drawing (spinner, fixed-width live rows). Move the spinner and live table into a small output module that `check` and `watch` could use too.
* [x] Move terminal style and widgets (spinner, live table, colors) into their own module: `src/term/` (`Spinner`, `LiveTable`, `Palette`).

### Library

* [ ] Make `comtrade` its own crate in a workspace (`sprcmtrd-comtrade`). The spec already says it could be published; a crate boundary enforces the layering rule at compile time and is a first step for the web / Tauri UIs.
* [ ] Large files (spec goal: ≥ 100 MB `.dat`): `Comtrade::open` copies all samples into vectors, `write::encode` builds a `Vec<Option<f64>>` per channel and `export --format json` builds the whole document in memory. Measure with a generated large file first, then stream or work on the memory-mapped data where it matters.
* [ ] `round12` formats and parses a string for every value (`format!("{v:.11e}")`); fine for stats, slow for `export` of large files. Replace with numeric rounding and benchmark.

### Tests

* [ ] `tests/cli.rs` has 700 lines; split per command (`tests/cli/list.rs`, …) with a shared helper module.
* [ ] Tests that list all of `tests/data` break whenever a sample file is added (happened with the `*_uppercase` files). Give them their own temporary directory with the files they need.
* [ ] The scripts that generated the synthetic recordings were lost. Generate them reproducibly instead, e.g. a small `xtask` or test helper that builds them with `comtrade::write`, so they can be changed and recreated.


### Sandbox

* [x] Create podman container for sandboxing claude agent. 