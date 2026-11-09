# TUI: impedance trajectories

- **Issue:** [smo/sprcmtrd#6](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/6), "Darstellung der Impedanztrajektorien"
- **Status:** done, released in 0.8.0. `impedance::clean` (float residue) moved from the command to the library, so the TUI shows the same values. The trajectory end is labelled in lower case, the cursor point in reversed bold. The configuration test rebinds zoom to `x` instead of `z`, now taken.

## Goal

A separate view in the TUI that draws the apparent impedance of the fault
loops as trajectories in the R-X plane, as a distance relay sees them:

- one diagram for the phase-to-ground loops A-N, B-N, C-N;
- one diagram for the phase-to-phase loops A-B, B-C, C-A.

The trajectories cover a time range that can be set, so they don't get
cluttered. A movable time cursor marks the point of each loop at that moment,
and the values are shown next to the diagrams.

## What exists

- `sprcmtrd impedance` computes the six loops at one moment or with
  `--trend` over time (`analysis::impedance`: `Loop`, `loop_quantities`,
  `impedance`, `find_pair`), from 1-cycle DFT phasors at the line frequency,
  ground loops compensated with `--k0` (default 0).
- The TUI has a time window (`+` / `-` zoom, `H` / `L` pan, `0` the whole
  recording) and a cursor on a sample (`←` / `→`, `t` to the trigger). The RMS
  (`r`) and harmonics (`f`) views replace the plots of the plot area.
- The voltage / current group helper (`VoltageCurrent`) is in
  `command/mod.rs`, which the TUI may not use (layering test). The group search
  (`find_pair`) and the unit factors (`unit_factor`) are in
  `analysis::impedance`.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Opening it | Key `z` (free), binding `impedance_view`, title `Impedance`. It replaces the plots of the plot area like `r` and `f`, and `z` goes back to the waveform. The sidebar stays | An overlay (`?`, `i` style): the cursor keys would then scroll the overlay instead of moving the cursor |
| Layout | Two R-X diagrams side by side, `L-N` left and `L-L` right, each with a table of its three loops under it. Below 100 columns of plot area they are stacked | One diagram with all six loops: unreadable |
| Time range | The visible time window of the plot area: zoom and pan set it as for the waveform (`+` / `-`, `H` / `L`, `0`). The title shows it (`-20.0 ms … 80.0 ms`). No new keys | Two range markers (start / end) with their own keys: more keys, and the zoom already does the same |
| Time cursor | The TUI cursor (`←` / `→`, `t`). Its point on each trajectory is drawn as the loop's name (`A`, `B`, `C` in L-N; `AB`, `BC`, `CA` in L-L). The table shows R, X, \|Z\| and φ of each loop at the cursor. The time axis on the bottom row stays, with the cursor | |
| Telling loops apart | By phase color, and without color by the name at the cursor point and at the trajectory's end (`agent.md`: color is never the only cue) | Different line styles: braille lines all look the same |
| Points | Phasors as in `impedance --trend`: 1-cycle DFT at the line frequency, a window ending at each point. The cursor values then equal `sprcmtrd impedance --at <cursor>`. Points: every sample in the window, at most 2 per column of the diagram (as in the harmonics view) | Windows fitted to the measured frequency: off nominal more accurate, but the numbers would differ from `impedance` |
| No loop current | A point is left out where the loop current is below 5 % of the largest loop current in the window (breaker open, unfaulted loop with no load): there the impedance goes to infinity and would set the scale. The table shows `-` | Draw everything: one point at 10⁵ Ω flattens the rest |
| Scale | Same Ω per dot on both axes (a braille dot is about square), so angles look right. The range covers the drawn points and the origin, with 5 % margin. R and X axes through the origin. The values are shown at the axis ends (`R 0 … 12.3 Ω`) | Separate R and X scales: the line angle looks wrong |
| Groups | Found by unit as in `impedance` (`find_pair`: the first voltage group and a current group of the same circuit). None: the view says `no voltage and current A/B/C groups` and can't be opened (footer, like `r` without a line frequency) | A key to choose the groups: later, if needed |
| k0 | 0 (no compensation) as in `impedance`, said in the L-N title (`L-N, k0 = 0`). A `k0` setting in the configuration file (`[impedance] k0 = "0.8@-5"`) is a follow-up in `todo.md` | Ask for k0 in the TUI: needs an input field |
| Side | Primary or secondary as the TUI's `p` key sets it (Ω primary / secondary) | |

## Output (sketch, 110 columns of plot area)

```
┌ Impedance · -20.0 ms … 80.0 ms · primary Ω ─────────── cursor 40.0 ms · window 100.0 ms ┐
│ L-N (k0 = 0)                            │ L-L                                          │
│ X ↑ 14.2                                │ X ↑ 38.0                                     │
│        ⣀⡠B                              │                 ⢀⣀AB                         │
│      ⡠⠊                                 │              ⡠⠊⠁                             │
│ ───⠐⠁────────A──C──────────── R → 22.5  │ ─────────────────BC─CA────────── R → 61.0    │
│                                         │                                              │
│ loop  R [Ω]   X [Ω]   |Z| [Ω]   φ       │ loop  R [Ω]   X [Ω]   |Z| [Ω]   φ            │
│ A-N   18.31   4.02    18.75     12.4°   │ A-B   7.81    35.20   36.06     77.5°        │
│ B-N   1.72    9.73    9.88      79.9°   │ B-C   …                                      │
│ C-N   20.05   3.11    20.29     8.8°    │ C-A   …                                      │
│                ▲ 40.0 ms                time axis                                      │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

(Layout illustrative.)

## Implementation plan

### 1. Library: `src/analysis/impedance.rs`

- `pub struct Loops<'a>`: the six channels (V A, B, C, I A, B, C) with their
  factors to V and A, from a `Pair` (`Loops::new(c, &pair)`). This moves the
  core of `command::VoltageCurrent` into the library, and `VoltageCurrent`
  uses it, so `impedance` and the TUI compute alike.
- `pub fn trajectory(loops: &Loops, c: &Comtrade, points: &[usize], samples, frequency, k0, side) -> Vec<[Option<Complex>; 6]>`:
  the six impedances at each point (window of `samples` ending there).
- Unit tests:
  - A synthetic fault with known R and X (V = Z·I) gives the impedance.
  - A point without current gives `None`.
  - The values equal `impedance --at` for `fault_2013`.

### 2. TUI

- `tui/app.rs`: `PlotView::Impedance`, `Action::ImpedanceView` (`z`).
  Opening it needs a line frequency and a voltage / current pair, otherwise a
  footer message. The `Loops` are found once per recording (`Recording`, like
  `track`).
- `tui/keys.rs`: binding `impedance_view` (`z`), help text.
- `tui/widgets/impedance.rs` (new): the two diagrams (ratatui `Canvas`, braille)
  with axes, trajectories, the names at the cursor and at the end, and the
  tables; side by side or stacked by width. `widgets/plot.rs` calls it in
  this view instead of the channel rows, and keeps the time axis.
- No change to zoom, pan or the cursor keys.

### 3. Tests (TUI, `tui/widgets/mod.rs`)

- `z` opens the view with `fault_2013`: both diagram titles, the tables with
  six loops. The values at the cursor (`t`, then `→` to 40 ms) equal
  `sprcmtrd impedance tests/data/fault_2013.cfg --at 40`. `z` again gives the
  waveform.
- Zoom changes the time range in the title.
- `fault_1999` (one voltage only, no voltage group) can't open the view:
  footer message.
- Narrow terminal (80×24): the diagrams are stacked and readable.
- No colors: the loop names are drawn.

### 4. Docs

- `docs/src/viewer/index.md` (an "Impedance view" item, keys), `keys.md`,
  `spec.md` TUI section, README viewer keys.
- `todo.md`: tick the item; add "k0 for the impedance view in the
  configuration".
- `tasks/README.md`: the status of this task.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`. `list_content_filters` needs `55Hz.cfg` out of
`tests/data/`.
