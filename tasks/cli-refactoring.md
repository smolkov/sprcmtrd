# Command line refactoring: broken pipe, shared options, helpers, exit codes, JSON

- **Issue:** none, from `todo.md` ("Refactoring ideas → Command line", the six
  open items)
- **Status:** in progress. Steps 1–5 are in 1.0.0 (merged into `main`);
  step 6 (JSON) is not started. Built differently:
  - Step 1: the JSON form of `Side` (`"primary"`, lowercase) moves to step 6:
    `Side` is already serialized as `"Primary"` in `info --json`, so it
    changes the output.
  - Step 3: `--channels` reads the same in `view`, `stats`, `export` and
    `cut` ("Channels: …", no longer "to show" / "to export" / "to keep"), and
    `cut` lists it before `--from` / `--to`. The struct is `TimeWindow`, not
    `Window` (the phasor `Window` has that name in `command`).
  - Step 4: no `Out` struct and no `Closed` type: `term::write_stdout` (behind
    `out!` / `outln!`) writes and flushes the locked stdout and returns the
    `io::Error`; `main` ends quietly on a `BrokenPipe` anywhere in the chain,
    which covers `export`, `list` and `completions` too.
  - Step 5: also a usage error: `convert` to `.cff` below revision 2013.

## Goal

Six open items in `todo.md` make the commands harder to change and to use
from scripts:

1. `sprcmtrd trend big.cfg | head` panics with "failed printing to stdout:
   Broken pipe".
2. `--channels`, `--from` and `--to` are defined again and again in
   `args.rs`, with slightly different help texts.
3. Small pieces of code are repeated: the side from `--secondary`, the side's
   name, a recording's file name, a relative time as a time of day.
4. "Is this output path part of the source recording" exists twice.
5. Errors in option combinations exit with 1, not 2 like clap's own usage
   errors, and `convert` strips `unsupported: ` from a library message by
   hand.
6. The JSON output names fields in different ways (`inception` next to
   `inception_s`, `frequency` without its unit), and nothing tells a script
   which form it gets.

Items 1, 5 and 6 change behavior. Items 2, 3 and 4 only change code; the
output stays the same, and the existing tests show it.

## What exists

| Item | Today |
|---|---|
| Broken pipe | 75 `print!` / `println!` in 21 command modules; only `export` (`BufWriter` on `stdout().lock()`), `list` and `completions` write to a locked stdout and return I/O errors. `main.rs` prints `error: {e:#}` and maps errors to exit codes in `exit_code`. |
| Shared options | `--from` / `--to` are in 15 `Args` structs. With the same meaning and help ("time window") in `view`, `stats`, `export`, `cut` and `events`; `--channels` for analog and binary channels in `view`, `stats`, `export` and `cut`. Others mean something else: the search window of `fault`, `dc-offset`, `locate`; the `--trend` range of `impedance`, `power`; A's time in `compare`; analog channels only in `phasors`, `harmonics`, `trend`; binary only in `events`. The commands call `command::select_channels` and `command::time_window` themselves. |
| Helpers | `if ctx.global.secondary { Side::Secondary } else { Side::Primary }` in 14 places (13 commands and the TUI). The side's name (`"primary"` / `"secondary"`) is written out in 8 places (`side_name`, `if side == Side::Secondary { "secondary" } …`). `file_name(c: &Comtrade)` twice (`compare.rs`, `frequency.rs`). `trigger + TimeDelta::nanoseconds((t * 1e9).round() as i64)` in `fault.rs`, `events.rs`, `comtrade/export.rs` and `tui/widgets/time_axis.rs`. |
| Overwrite protection | `export::refuse_source` (one output file) and `convert::check_outputs` (all output files, plus `--force`), each listing `cfg`, `dat`, `cff`, `hdr`, `inf` of `Comtrade::path` and comparing canonical paths. `cut` uses `convert::save`. |
| Exit codes | 0 ok, 1 other, 2 clap usage, 3 file (`Error::is_file_error`, I/O `NotFound`), 4 content (`comtrade::Error`, `check::Failed`). Option combinations checked in the commands are `bail!` → 1: `--plot draws text only; leave out --format`, `--format csv needs --trend`, `--cycles must be above 0` (compare, phasors), `give both --voltage and --current, or neither`, `--method crossings … whole number` (frequency), `leave out --secondary` (locate). `convert` turns `write::check_target`'s `Error::Unsupported` into a string and strips `unsupported: `, so it exits 1. |
| JSON | 18 `serde_json::to_string*` calls, 60 `Serialize` structs in 15 modules. Times mostly `*_s` (`time_s`, `from_s`, `at_s`, `duration_s`), but `fault` has `inception` and `interruption` next to `inception_s` / `interruption_s` in `dc-offset` and `locate`. `frequency` (Hz) and the angles (`angle`, `a_angle`, `k0_angle`, `angle_difference`, degrees) have no unit in their names; `percent`, `distance_km`, `length_km` have one. `check` writes an array at the top, the others an object. No version anywhere. |

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Scope | One task, built in six steps on one branch, one commit per step, in the order of the plan below (the behavior changes last). Each step leaves all tests green | JSON as a task of its own: it is the largest step and the only one that breaks scripts |
| Broken pipe | Commands write through `term::Out` (a locked, buffered stdout; `out!` / `outln!` macros instead of `print!` / `println!`). A `BrokenPipe` from writing or flushing becomes `term::Closed`; `main` ends quietly with exit 0 on it, as `head` expects | Restore the default `SIGPIPE` handler at start (`libc::signal`): one line, but a new dependency, Unix only (Windows still panics), and the process dies with 141 without flushing |
| Shared options | Two `#[command(flatten)]` structs in `args.rs`: `Window { from, to }` ("time window", for `view`, `stats`, `export`, `cut`, `events`) and `Selection { channels, window }` (analog and binary channels, for `view`, `stats`, `export`, `cut`). Methods in `command/mod.rs`: `Window::range(&Comtrade)` and `Selection::channels(&Comtrade)`. The options whose meaning differs (search window, `--trend` range, A's time, analog or binary only) keep their own fields and help texts | One struct for all 15: the help would lose what the window means in each command |
| Option names, help | Unchanged for the user; the help of the five commands reads the same as now (it already does except for `events`' binary-only `--channels`, which stays its own) | |
| Helpers | `GlobalOpts::side()`; `Side::name()` and `#[serde(rename_all = "lowercase")]` on `Side`, so JSON writes `"side": side`; `Comtrade::name()` (the file name of the `.cfg` or `.cff`); `Comtrade::absolute_time(t) -> NaiveDateTime`. All in the library (`comtrade`) except `side()` (`args`) | Leave the TUI out: it has one of each, and `comtrade` is shared anyway |
| Overwrite protection | `RecordingPath::contains(&Path) -> bool` in `comtrade::discover` (canonical paths of all its files); `refuse_source` and `check_outputs` use it, their messages stay | A `write::check_outputs` in the library: but `--force` and the message are command line matters |
| Usage errors | `command::UsageError(String)`, exit 2 like clap, for the option combinations listed above. Errors that depend on the recording ("no line frequency; give it with --frequency", unknown channel, empty window) stay 1 | Exit 1 for all of them, and only document that 2 means "clap": then scripts can't tell a wrong call from a wrong file |
| `--cycles` | Checked by clap (`value_parser` with a range above 0) instead of a `bail!`: exit 2, with clap's message | `UsageError` in the command |
| `convert` target | `write::check_target`'s `Error::Unsupported(msg)` becomes `UsageError(msg)` (exit 2, the option chose an impossible target), without string stripping | Keep exit 4: "unsupported" content, though the recording itself is fine |
| JSON names | Every number carries its unit in the name when it has a fixed one: `_s` times, `_hz` frequencies, `_deg` angles, `_percent` percentages, `_km` lengths. Values in the channel's unit keep plain names next to their `unit`. `inception` → `inception_s` and the like; `frequency` → `frequency_hz`; `angle` → `angle_deg` | Only fix the few real mismatches (`inception`, `interruption`), no `_hz` / `_deg`: less breakage, but the rule stays unwritten |
| JSON version | Every JSON output is an object with `"version": 1` first. `check --format json` changes from an array to `{ "version": 1, "recordings": [...] }`. The version goes up when a field is renamed or removed, not when one is added | A `--json-version` option to ask for the old form: code for a format nobody relies on yet (0.x) |
| JSON docs | A page `docs/src/json.md`: the rules, the version, and one example per command; the command pages link to it. The README says that 0.9.0 renames fields | Only the rules, no examples |
| Release note | The renamed fields and the `check` array are listed for the next release (0.9.0) in the commit message and `docs/src/json.md` ("Changes in version 1") | |

## Implementation plan

Each step: code, tests, docs of that step, the four checks from `agent.md`,
then a commit.

### 1. Helpers (no output change)

- `args.rs`: `GlobalOpts::side()`; replace the 14 `if … secondary` blocks.
- `comtrade`: `Side::name()`, `Serialize` for `Side`
  (`rename_all = "lowercase"`); replace the hand-written names and the
  JSON `side: &'static str` fields by `side: Side`.
- `comtrade`: `Comtrade::name()`, `Comtrade::absolute_time(t)`; replace
  `compare::file_name`, `frequency::file_name` and the four
  `TimeDelta::nanoseconds` sums.
- Tests: unit tests for `name`, `absolute_time` (rounding to the nearest ns,
  negative `t`), `Side` serialization. The CLI tests stay unchanged and must
  pass.

### 2. Overwrite protection (no output change)

- `comtrade::discover`: `RecordingPath::files()` (the existing files) and
  `contains(&Path)`; `export::refuse_source` and `convert::check_outputs`
  use it.
- Tests: `contains` for each file kind, a path that doesn't exist, a
  different spelling of the same file (`./x.cfg`, a symlink on Unix).

### 3. Shared options (no output change)

- `args.rs`: `Window` and `Selection` with the help texts of today;
  `ViewArgs`, `StatsArgs`, `ExportArgs`, `CutArgs` get `selection:
  Selection`, `EventsArgs` gets `window: Window` (its binary `--channels`
  stays).
- `command/mod.rs`: `Window::range`, `Selection::channels` (wrapping
  `time_window` and `select_channels`); the five commands use them.
- Tests: `sprcmtrd view --help` (and the four others) list `--channels`,
  `--from`, `--to` with the same text as before (a CLI test comparing the
  three lines); the existing tests.

### 4. Broken pipe

- `term/out.rs`: `Out` (locked `BufWriter<StdoutLock>`), `out!` / `outln!`,
  `Out::finish()` (flush), `Closed` error; `From<io::Error>` maps
  `BrokenPipe` to `Closed`.
- Every command writes through `Out` (`print!` / `println!` gone from
  `src/command/`, checked by a test that greps the sources like
  `tests/layering.rs`). `export` and `list` keep their writers but map
  `BrokenPipe` the same way.
- `main.rs`: `Closed` anywhere in the chain → exit 0, nothing printed.
- Tests: `tests/cli.rs` runs `trend` and `export` of `fault_2013` with
  stdout a pipe that is closed after the first line (`std::process::Command`,
  read one line, drop the reader): exit 0, stderr empty. Unix only
  (`#[cfg(unix)]`); Windows gets the same code path without the test.

### 5. Exit codes

- `command::UsageError` (thiserror), mapped to exit 2 in `main.rs`; the
  combinations from "What exists" use it; `--cycles` gets a clap range;
  `convert` maps `Error::Unsupported` to it.
- Tests: each combination exits 2 with its message (extend
  `compare_errors` and the like); `convert --rev 1991 --format float32`
  exits 2 and its message has no `unsupported:`.
- Docs: exit code table in `README.md`, `docs/src/commands/index.md`,
  `spec.md` (2: "invalid command line arguments or combinations").

### 6. JSON

- Rename by the rules in every `Serialize` struct (a table of old → new in
  the commit and in `docs/src/json.md`); `version: 1` first in every
  top-level JSON object; `check` wrapped.
- A unit test per command module is too much; instead one CLI test runs
  every command with `--format json` on `fault_2013` and checks: an object,
  `"version": 1` first, and no number field (recursively) without a unit
  suffix except a short allow-list (values in channel units: `rms`, `peak`,
  `min`, `max`, `mean`, `value`, `magnitude`, `re`, `im`, `r`, `x`, `p`, `q`,
  `s`, counts like `samples`, `index`, `order`). Existing JSON tests are
  updated to the new names.
- Docs: `docs/src/json.md` (in `SUMMARY.md`), links from the command pages,
  `spec.md` (the rules), README (a short note and the link).

### 7. Finish

- `todo.md`: tick the six items; `tasks/README.md` and this file's status.
- Checks: the four commands from `agent.md`, `mdbook build docs`, and a
  look at `sprcmtrd trend tests/data/fault_2013.cfg | head -3`.
