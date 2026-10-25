# `check`: Validate recordings

Check recordings for problems (for scripts and CI). Errors: the recording can't be read correctly. Warnings: missing or clipped values, times out of order, missing units, unusable ratios, ... Exit code 4 when a recording has an error (or a warning with --strict).

```
sprcmtrd check [OPTIONS] <PATH>
```

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

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<PATH>` | A recording file, or a directory to check all recordings in |
| `--no-recursive` | Only check DIR itself, not its subdirectories |
| `--strict` | Fail (exit code 4) on warnings too |
| `--json` | Print JSON instead of text |

The [global options](index.md#global-options) apply too. `sprcmtrd check --help`
shows this list with examples.
