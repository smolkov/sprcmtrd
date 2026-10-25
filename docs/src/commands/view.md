# `view`: Summary and plots on stdout

Print a summary and plots of a recording to stdout

```
sprcmtrd view [OPTIONS] <FILE>
```

```sh
sprcmtrd view tests/data/fault_2013.cfg --channels IB,TRIP_B,CB_CLOSED \
    --from -20 --to 120 --width 72 --height 4
```

```
A2 IB [A]
        ⢈   ⡰⠋⠳⡀     ⡠⠲⡄      ⡠⢄      ⢀⢤⡀      ⢀⣀             7.244k
        ⠠  ⢠⠃  ⢣    ⡰⠁ ⠘⡄    ⡜  ⢣    ⢠⠊ ⠸⡀    ⢰⠁⠈⢣
⠤⠤⠤⠴⠲⠒⠶⠤⢴⠄⡠⠃⠄⠠ ⠄⢧ ⠄⡰⠁⠄⠠ ⠵⡠ ⠄⡸ ⠄⠠ ⡇⠠ ⢄⠮ ⠄⠠⢱⠄⠠ ⢤⠣ ⠄⠨⢆⡤⠤⠤⠤⠤⠤⠤⠤⠤⠄
        ⢈⠑⠁     ⠈⠢⠴⠃     ⠣⣀⡔⠁    ⠘⢆⢀⠎     ⠣⡀⣠⠃    ⠈⡇
                                   ⠁       ⠉⠁                 -4.602k
-20.0 ms                                            120.0 ms

▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▌████████████████████████▌ D5 TRIP_B
███████████████████████████████████████████████████▌▁▁▁▁▁▁▁▁ D7 CB_CLOSED
```

| Option                | Meaning                                                              |
|-----------------------|----------------------------------------------------------------------|
| `--channels LIST`     | Comma separated: analog index or id (`3`, `IA`), binary `d2` or id (`TRIP`). Default: all |
| `--from MS`, `--to MS`| Time window in ms relative to the trigger                            |
| `--width COLS`        | Plot width (default: terminal width, 100 when not a terminal)        |
| `--height ROWS`       | Height of each analog plot (default 10)                              |

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `--channels <CHANNELS>` | Channels to show: comma separated 1-based indices or ids (analog: `3` or `IA`, binary: `d2` or `TRIP`). Default: all |
| `--from <FROM>` | Start of the time window in ms relative to the trigger |
| `--to <TO>` | End of the time window in ms relative to the trigger |
| `--width <WIDTH>` | Plot width in columns (default: terminal width) |
| `--height <HEIGHT>` | Height of each analog plot in rows [default: 10] |

The [global options](index.md#global-options) apply too. `sprcmtrd view --help`
shows this list with examples.
