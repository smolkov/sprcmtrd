# Todo

* [x] Improve command 'list' with option to show only not broken comtrade files can be parsed successfully. 
* [x] Improve file picker to switch between show only working comtrade files and show all files. Use keyboard `ctrl`+ `b` shortcuts for toggling. 

## Command ideas

* [ ] `check <FILE|DIR>`: full validation of recordings for scripts and CI. Reads `.cfg` and `.dat` and reports problems: sample count vs `.cfg` end sample, record size, non-monotonic timestamps, missing values (`99999` / `0xFFFF`), clipped values at min/max, unknown units. Exit code 0 when all valid, 4 otherwise; `--json` for machine output.
* [ ] `events <FILE>`: timeline of binary channel changes (trip, breaker open/close) with time relative to trigger and absolute time, e.g. `+12.5 ms  TRIP_A  0 → 1`. Options `--channels`, `--from`/`--to`, `--json`.
* [ ] `stats <FILE>`: per analog channel min, max, mean, RMS, peak value and its time, in primary or secondary values (`--secondary`); per binary channel number of changes. Options `--channels`, `--from`/`--to`, `--json`.
* [ ] `export <FILE>`: write samples as CSV (or JSON) for Excel, Python or gnuplot. Options `--channels`, `--from`/`--to`, `--secondary`, `--every N` (decimation), `--output FILE` (default stdout).
* [ ] `diff <A> <B>`: compare two recordings: header fields, sample rates, channel lists (added, removed, changed scaling or units). Useful when relay settings or a recorder changed.
* [ ] `watch <DIR>`: watch a directory (fault recorder output folder) and print a `list` row for each new recording as it appears; `--exec CMD` runs a command for it (e.g. `sprcmtrd view`).
* [ ] `list` filters: `--station REGEX`, `--since DATE`, `--until DATE`, `--min-duration`, so large archives can be searched by recording content, not only by file name.
* [ ] `phasors <FILE> --at <ms>`: fundamental phasors (magnitude and angle) of the analog channels at a time, plus symmetrical components (positive, negative, zero sequence) for three-phase groups. Signal analysis is a non-goal for v1, so this needs a spec update first.

## Commands that write files (non-goal for v1, needs spec decision)

* [ ] `cut <FILE> --from <ms> --to <ms> --output <NAME>`: extract a time window (and optionally `--channels`) into a new, smaller recording.
* [x] `convert <FILE>`: convert between data formats (ASCII ↔ BINARY/BINARY32/FLOAT32), revisions (1991/1999 → 2013), and `.cfg`+`.dat` ↔ `.cff`.
* [ ] `rename <DIR>`: rename recordings to a pattern such as `{station}_{start:%Y%m%d_%H%M%S}`, with `--dry-run`.

## TUI ideas

* [ ] Write log to a file in TUI mode (already in spec, not implemented).
* [ ] Two measurement cursors: show Δt and the value difference of each channel between them.
* [ ] Allow more than one key per action in the configuration overlay (rebinding currently replaces all keys).
