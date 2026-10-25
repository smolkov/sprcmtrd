# `info`: Summary of a recording

Print a summary of a recording

```
sprcmtrd info [OPTIONS] <FILE>
```

```sh
sprcmtrd info tests/data/fault_2013.dat
sprcmtrd info --json tests/data/combined_2013.cff | jq .config.analog
```

Prints station, device, revision, data format, start and trigger time, duration,
sample rates, the analog and binary channel tables, and the `.hdr` / `.inf`
content (`--no-extra` skips it). `--json` prints the same data as JSON. Only the
configuration is read, so `info` is fast even for large recordings.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--json` | Print JSON instead of text |
| `--no-extra` | Do not print .hdr / .inf content |

The [global options](index.md#global-options) apply too. `sprcmtrd info --help`
shows this list with examples.
