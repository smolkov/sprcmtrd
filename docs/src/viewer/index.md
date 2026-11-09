# Interactive viewer

Start it without a command: `sprcmtrd` opens the file picker in the current
directory, `sprcmtrd PATH` opens a recording or the picker in a directory.

The screen has a header, the channel list on the left, the plots with a shared
time axis on the right, and a footer with the most used keys:

```
┌ Channels ──────┐┌ Plot ──────────────────────── cursor 2.0 ms · window 2.000 s ┐
│● IA           A││IA [A]         ...analog line plots...                       │
│● IB           A││▸ 2.828k                                                     │
│  ...           ││...                                                          │
│──── binary ────││TRIP_B        1 ▁▁▁▁▁▁▌██▌▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁ │
│● TRIP_B        ││CB_CLOSED     0 ██████▌▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▌██████████████████████ │
│● CB_CLOSED     ││               -0.500 s      T ▲ 2.0 ms     0.500 s   1.500 s│
└────────────────┘└─────────────────────────────────────────────────────────────┘
q quit  o open  i info  ? help  Space toggle  + zoom  ← cursor  Shift+← pan  c config
```

- **Header** (above the panels, not shown): program name and version, the
  file, station and device, revision, data format, sample rate and number of
  samples.
- **Sidebar:** channel list; `●` shown, `○` hidden. The first six analog channels
  and up to eight binary channels that change state are shown after loading.
- **Plots:** one shared time axis for all channels. The gutter shows the channel,
  its value at the cursor and the visible value range. Binary states that differ
  from the channel's normal state are highlighted.
- **RMS view:** `r` shows the sliding RMS instead of the samples, over one cycle
  ending at each sample, so a step shows up over the following cycle. The cycle is
  fitted to the measured frequency of the strongest voltage or current. `d` removes
  the DC part (title `RMS-DC`). There is a gap until a whole window fits.
- **Harmonics view:** `f` shows the harmonics over time: one row per channel and
  order, at first only the fundamental (`IA h1 [A]`, RMS). In this view `1` … `9`
  show and hide the orders 1 … 9; orders 2 and up are in % of the fundamental at
  the same moment (`IA h3 [%]`). The window is one cycle of the measured
  frequency, as in the RMS view, so the harmonics stay at whole multiples when
  the system runs off nominal (with a fixed 50 Hz window, a 55 Hz signal shows a
  false 2nd harmonic). Orders at or above half the sample rate can't be shown.
- **Scrolling:** analog plots are at least 5 rows high. When the shown channels
  don't fit, the plots scroll (`PgUp` / `PgDn`, or select a channel in the
  sidebar); a scrollbar on the right border and `channels 4–9 of 18` in the
  title show the position. The time axis and the title always stay visible.
- **Time axis:** tick labels, the trigger `T` and the cursor `▲` with its exact
  time. The cursor always sits on a sample.
- Minimum terminal size is 80×24.

### Keys

| Key                 | Action                                        |
|---------------------|-----------------------------------------------|
| `q`, `Ctrl+c`       | Quit (closes an open overlay first)           |
| `o`, `Ctrl+p`       | Open file picker                              |
| `i`                 | Recording info overlay                        |
| `?`                 | Help overlay                                  |
| `c`, `Ctrl+,`       | Configuration overlay                         |
| `Tab`               | Switch focus sidebar / plot                   |
| `↑`/`↓`, `k`/`j`    | Select channel (sidebar), zoom (plot)         |
| `Ctrl+↑`/`Ctrl+↓`, `K`/`J` | Move 10 channels (sidebar), scroll 10 lines (help, info) |
| `Home`/`End`        | First / last channel (sidebar), top / bottom (help, info) |
| `PgUp`/`PgDn`       | Scroll plots one page up / down               |
| `Space`             | Show / hide selected channel                  |
| `a`                 | Show all / hide all channels                  |
| `+`/`=`, `-`        | Zoom in / out (time, around the cursor)       |
| `←`/`→`, `h`/`l`    | Move cursor                                   |
| `Shift+←`/`→`, `H`/`L` | Pan the view                               |
| `0`                 | Reset zoom (whole recording)                  |
| `t`                 | Jump cursor to trigger                        |
| `p`                 | Primary / secondary values                    |
| `r`                 | Waveform / sliding RMS                        |
| `d`                 | RMS with / without DC                         |
| `f`                 | Waveform / harmonics over time                |
| `1` … `9`           | Harmonics view: show / hide order 1 … 9       |
| `m`                 | Relative / absolute time                      |
| `s`                 | Show / hide sidebar                           |
| `T`                 | Dark / light theme (not saved)                |
| `Esc`               | Close overlay                                 |

`Ctrl+,` works in terminals with the kitty keyboard protocol (kitty, WezTerm,
foot, Ghostty, recent Alacritty); `c` works everywhere.

### File picker

Opens at start when no file is given, and with `o`.

- Lists the subdirectories of the current directory, then all recordings in it
  **and its subdirectories**, with paths relative to it. `Ctrl+r` switches to
  this directory only.
- `Ctrl+b` shows only valid recordings, hiding those whose configuration or data
  fails to load (like `list --valid`); press it again to show all.
- Type to filter (case-insensitive substring); start with `/` for a regular
  expression, e.g. `/^station_b/.*2026`.
- `↑`/`↓` select, `Ctrl+↑`/`Ctrl+↓` or `PgUp`/`PgDn` move 10 entries, `Home`/`End`
  go to the first and last entry.
- `Enter` opens a recording or directory, `Backspace` on an empty filter goes to
  the parent directory, `Esc` closes.
- The search runs in the background and results appear as they are found.

### Configuration overlay

`c` (or `Ctrl+,`) shows the theme and every key binding. Changes take effect
immediately and are saved to the configuration file:

- Theme row: `Enter` or `←`/`→` switches dark / light.
- Binding row: `Enter`, then press the new key (`Esc` cancels). Keys already
  used by another action are refused. `Backspace` restores the default.

## Pages

- [Keys](keys.md): every key and what it does.
- [File picker](file-picker.md): finding and opening recordings.
- [Configuration](configuration.md): theme and key bindings.
