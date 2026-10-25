# `export`: Samples as CSV or JSON

Export samples as CSV or JSON (for Excel, Python, gnuplot, ...). Analog values are scaled (primary side unless --secondary), binary channels are 0 or 1, missing values are empty (CSV) or null (JSON).

```
sprcmtrd export [OPTIONS] <FILE>
```

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

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--format <FORMAT>` | Output format (default: json for a .json output file, otherwise csv). Values: `csv` (comma separated values with a header row), `json` (one JSON object with an array per column) |
| `--channels <CHANNELS>` | Channels to export: comma separated 1-based indices or ids (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all |
| `--from <FROM>` | Start of the time window in ms relative to the trigger |
| `--to <TO>` | End of the time window in ms relative to the trigger |
| `--every <EVERY>` | Export only every Nth sample (no filtering) [default: 1] |
| `--absolute` | Time as date and time of day instead of ms relative to the trigger |
| `-o, --output <OUTPUT>` | Write to this file instead of stdout |

The [global options](index.md#global-options) apply too. `sprcmtrd export --help`
shows this list with examples.
