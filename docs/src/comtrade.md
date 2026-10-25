# COMTRADE support

## Revisions

| Revision | Support | Notes                                                                 |
|----------|---------|-----------------------------------------------------------------------|
| 1991     | Yes     | No revision year in `.cfg` line 1; shorter analog/digital lines.      |
| 1999     | Yes     | Main target.                                                          |
| 2013     | Yes     | Adds time code / leap second lines, UTF-8, `.cff` single-file format. |

The revision is detected from line 1 of the `.cfg`. Without a year it is read as 1991.

## Files of a recording

| Extension | Content                      | Required | Support                     |
|-----------|------------------------------|----------|--------------------------------|
| `.cfg`    | Configuration (text)         | Yes      | Full parse                     |
| `.dat`    | Sample data                  | Yes      | Full parse                     |
| `.hdr`    | Free-text header             | No       | Show as raw text in `info`     |
| `.inf`    | Extra information (INI-like) | No       | Show as raw text in `info`     |
| `.cff`    | 2013 combined file           | No       | Split into sections, then same as above |

File extensions are matched case-insensitively (`.CFG` and `.cfg` both work).
Given any one file of a recording (for example `fault.dat`), the matching
siblings are found by replacing the extension.

## Data formats

| `ft` in `.cfg` | Analog sample type | Notes                                |
|----------------|--------------------|--------------------------------------|
| `ASCII`        | parsed to `i32`/`f64` | comma separated, one row per sample |
| `BINARY`       | `i16`              | little endian                        |
| `BINARY32`     | `i32`              | little endian (2013)                 |
| `FLOAT32`      | `f32`              | little endian (2013)                 |

Binary record layout: `u32` sample number, `u32` timestamp, analog values,
then binary channels packed in 16-bit words (LSB = first channel).

## Values and time

- **Scaling:** engineering value = `a * raw + b`. Primary/secondary conversion
  uses the `primary`, `secondary` and `PS` fields of each analog channel.
  Press `p` in the viewer or give `--secondary` to see secondary values.
- **Missing values:** `0x8000` for `BINARY`, `0x80000000` for `BINARY32`, an empty
  field (2013) or `99999` (1991, 1999) for `ASCII`. Missing samples are shown as gaps, not as zero.
- **Timestamps:** when the sample rate in `.cfg` is non-zero, time is computed
  from sample number and rate(s). When it is `0`, the timestamp column
  × `timemult` (µs) is used. A missing timestamp (`0xFFFFFFFF`) falls back to the rate.
- **Multiple sample rates** (`nrates > 1`) are supported.
- **Time axis** is shown relative to the trigger time (t = 0 at trigger) by
  default, with an option for absolute time.
