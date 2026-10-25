# `cut`: Time window as a new recording

Cut a time window and/or some channels into a new, smaller recording. The output keeps the revision and data format of the source (`.cff` output needs revision 2013, see `convert`). Times relative to the trigger stay the same.

```
sprcmtrd cut [OPTIONS] --output <OUTPUT> <FILE>
```

```sh
sprcmtrd cut tests/data/fault_2013.cfg --from -20 --to 100 --channels IA,IB,IC,TRIP_A,CB_CLOSED -o short.cfg
```

```
tests/data/fault_2013.cfg -> short.cfg (2013 BINARY32)
  kept samples 961..=1201 of 4001 (-20.0 ms .. 100.0 ms), 3 of 8 analog and 2 of 10 binary channels
  wrote short.cfg  362 B
  wrote short.dat  5.3 kB
  read back: 241 samples, 3 analog, 2 binary channels
```

- `--from` / `--to` (ms relative to the trigger) and `--channels` work as in
  `view`; without them everything is kept.
- The new recording has the source's revision and data format; times relative
  to the trigger stay the same. `NAME.cff` writes a combined 2013 file.
- Existing files are only replaced with `--force`, and never the source
  recording. To change revision or format too, run `convert` on the result.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `-o, --output <OUTPUT>` | Output file: NAME.cfg (with NAME.dat) or NAME.cff |
| `--from <FROM>` | Start of the time window in ms relative to the trigger |
| `--to <TO>` | End of the time window in ms relative to the trigger |
| `--channels <CHANNELS>` | Channels to keep: comma separated 1-based indices or ids (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all |
| `--force` | Overwrite existing output files |

The [global options](index.md#global-options) apply too. `sprcmtrd cut --help`
shows this list with examples.
