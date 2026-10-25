# `list`: Recordings in a directory

List recordings in a directory

```
sprcmtrd list [OPTIONS] [DIR]
```

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

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `--no-recursive` | Only search DIR itself, not its subdirectories |
| `--filter <FILTER>` | Only show recordings whose file name matches this regular expression |
| `--valid` | Only show recordings that load without errors (configuration and data) |
| `--station <STATION>` | Only show recordings whose station name matches this regular expression (case-insensitive) |
| `--since <SINCE>` | Only recordings that start at or after this time (`2024-03-15`, `2024-03-15 08:30`, `2024-03-15T08:30:00`) |
| `--until <UNTIL>` | Only recordings that start at or before this time; a date alone includes the whole day |
| `--min-duration <MIN_DURATION>` | Only recordings at least this long (`500ms`, `2s`, `1min`; a bare number is ms) |
| `--max-duration <MAX_DURATION>` | Only recordings at most this long |
| `--sort <SORT>` | Sort order. Default: in a terminal, rows are shown as they are found; otherwise sorted by start time. Values: `name` (file name), `time` (start time of the recording), `size` (size of the data file) |

The [global options](index.md#global-options) apply too. `sprcmtrd list --help`
shows this list with examples.
