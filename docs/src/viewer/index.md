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
  A long list shows a scrollbar and the selected channel's position
  (`12 / 40`) on the bottom border.
- **Plots:** one shared time axis for all channels. The gutter shows the channel,
  its value at the cursor and the visible value range. The channel selected in
  the sidebar is marked with `▶` in its plots. Binary states that differ from the
  channel's normal state are highlighted.
- **View tabs:** the plot's title lists the views as tabs with their keys,
  `w Plot │ r RMS │ f Harmonics │ z Impedance`, then the group toggle
  (`┃ g Group`, `[g Group]` when on) and the current view's details. The
  current view is highlighted (in brackets with `--no-color`); views the
  recording can't show are crossed out. `w` shows the waveform from any view,
  `v` goes to the next view that can be shown. When the title is narrow the
  keys go first, then the names of the other tabs (only their keys), then the
  harmonic orders that aren't shown, then the other tabs.
- **Grouping:** `g` draws the phases of a circuit and its neutral in one plot,
  for example IA, IB, IC and IN as `I ABCN [A]`, with one value line per trace in
  the gutter (`B ▸ -3.226k`). The traces share the y range; besides their phase
  color they are named at their right end and at the cursor. Only shown channels
  are grouped; the sidebar still shows and hides them one by one. Grouping works
  in the RMS and harmonics views too; `g` again gives one plot per channel.
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
  false 2nd harmonic). The title lists the orders, the shown ones in brackets
  (`Harmonics [1] 2 [3] 4 …`; on a narrow terminal only the shown ones,
  `Harmonics 1 3`). Orders at or above half the sample rate can't be shown.
- **Impedance view:** `z` draws the apparent impedance of the fault loops as
  trajectories in the R-X plane: the phase-to-ground loops A-N, B-N, C-N
  (`L-N`, without residual compensation, k0 = 0) and the phase-to-phase loops
  A-B, B-C, C-A (`L-L`), side by side or, on a narrow terminal, stacked. The
  trajectories cover the visible time window, so zoom and pan (`+` / `-`,
  `H` / `L`, `0`) set the range. The cursor (`←` / `→`, `t`) marks each loop's
  point with its name (`A`, `BC`, …; lower case at the end of the trajectory),
  and the tables show R, X, |Z| and φ at the cursor, the same values as
  `sprcmtrd impedance --at`. Points with less than 5 % of the loop's largest
  current in the window (breaker open) are left out. Both axes have the same
  Ω per dot, so angles look right. It needs a voltage and a current A/B/C
  group (found by unit).
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
| `w`                 | Waveform (from any view)                      |
| `v`                 | Next view: waveform, RMS, harmonics, impedance |
| `r`                 | Waveform / sliding RMS                        |
| `d`                 | RMS with / without DC                         |
| `f`                 | Waveform / harmonics over time                |
| `1` … `9`           | Harmonics view: show / hide order 1 … 9       |
| `z`                 | Waveform / impedance trajectories (R-X)       |
| `g`                 | Group phases in one plot / one plot per channel |
| `m`                 | Relative / absolute time                      |
| `s`                 | Show / hide sidebar                           |
| `T`                 | Next color theme (not saved)                  |
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
- The bottom line shows the selected entry's position (`12 / 512 entries`;
  `12 / 340 · 512 entries` when the filter hides some); a long list has a
  scrollbar.
- `Enter` opens a recording or directory, `Backspace` on an empty filter goes to
  the parent directory, `Esc` closes.
- The search runs in the background and results appear as they are found.

### Configuration overlay

`c` (or `Ctrl+,`) shows the theme and every key binding. Changes take effect
immediately and are saved to the configuration file:

- Theme row: `←`/`→` go to the previous / next theme, `Enter` to the next.
- Binding row: `Enter`, then press the new key (`Esc` cancels). Keys already
  used by another action are refused. `Backspace` restores the default.

## Pages

- [Keys](keys.md): every key and what it does.
- [File picker](file-picker.md): finding and opening recordings.
- [Configuration](configuration.md): color themes, key bindings.
