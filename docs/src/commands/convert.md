# `convert`: Other revision, format or layout

Convert a recording to another revision, data format or file layout. The output extension selects the layout: `.cfg` writes .cfg + .dat (and .hdr / .inf when the source has them), `.cff` one combined file.

```
sprcmtrd convert [OPTIONS] --output <OUTPUT> <FILE>
```

```sh
sprcmtrd convert tests/data/combined_2013.cff -o fault.cfg --rev 1999 --format binary
```

```
tests/data/combined_2013.cff (2013 FLOAT32) -> fault.cfg (1999 BINARY)
  wrote fault.cfg  440 B
  wrote fault.dat  14.4 kB
  wrote fault.hdr  31 B
  wrote fault.inf  47 B
  note: revision 1999 has no time codes or leap second field; they are left out
  note: IA: rescaled to 16-bit integers, resolution 1.2207e-1 A
  ...
  read back: 800 samples, 4 analog, 4 binary channels
```

- The output name selects the layout: `NAME.cfg` writes `.cfg` + `.dat` (and
  `.hdr` / `.inf`), `NAME.cff` one combined file.
- `--format ascii|binary|binary32|float32` and `--rev 1991|1999|2013` default
  to the source's. BINARY32, FLOAT32 and `.cff` need revision 2013, which is
  then chosen automatically.
- Anything that loses information is printed as a `note:`: channels rescaled to
  fit 16- or 32-bit integers (with the new resolution), values rounded to
  32-bit floats, and fields an older revision cannot store.
- Existing files are only replaced with `--force`, and never the source
  recording. The result is read back before the command reports success.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<FILE>` | Any file of the recording (.cfg, .dat, .cff, ...) |
| `-o, --output <OUTPUT>` | Output file: NAME.cfg (with NAME.dat) or NAME.cff |
| `--format <FORMAT>` | Data format of the output (default: the source format). Values: `ascii` (comma separated text), `binary` (16-bit integers), `binary32` (32-bit integers (revision 2013)), `float32` (32-bit floats (revision 2013)) |
| `--rev <REV>` | Standard revision of the output (default: the source revision, or 2013 when the format or .cff output needs it). Values: `1991` (IEEE C37.111-1991), `1999` (IEEE C37.111-1999), `2013` (IEEE C37.111-2013 / IEC 60255-24:2013) |
| `--force` | Overwrite existing output files |

The [global options](index.md#global-options) apply too. `sprcmtrd convert --help`
shows this list with examples.
