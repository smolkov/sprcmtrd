# sprcmtrd

`sprcmtrd` is a terminal viewer and analysis tool for COMTRADE recordings
(IEEE C37.111 / IEC 60255-24), the standard format for oscillography and status
data recorded by protection relays and disturbance recorders during power
system faults.

It has two faces:

- **An interactive viewer** to browse recordings and inspect analog channels as
  line plots and binary channels as status strips, with zoom, pan and a cursor.
  See [Interactive viewer](viewer/index.md).
- **Commands** that print to stdout and work in scripts and over SSH: listing
  and checking recordings, statistics and binary events, phasors, trends,
  system frequency, harmonics and fault loop impedances, power, export, cutting and converting. See
  [Commands](commands/index.md).

## Features

- COMTRADE revisions **1991, 1999 and 2013**
- Data formats **ASCII, BINARY, BINARY32 and FLOAT32**, and the 2013 single-file
  **`.cff`** format
- `.hdr` and `.inf` content shown with the recording
- Scaling (`a * raw + b`) with primary / secondary conversion
- Multiple sample rates, timestamp based recordings, missing samples shown as gaps
- Time relative to the trigger (or absolute time of day)
- Large files: a 100 MB `.dat` opens in well under a second (memory mapped,
  min/max decimation for plotting so peaks are never lost)
- Dark, light and no-color themes; configurable key bindings

## About this book

The book describes version 0.6.0. Example outputs come from the
synthetic sample recordings in `tests/data/` of the repository, so you can run
every example yourself in a checkout.
