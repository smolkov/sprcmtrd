# TUI: optional grouping of the analog traces

- **Issue:** [smo/sprcmtrd#5](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/5), "optionale Gruppierung der Analogspuren"
- **Status:** done, unreleased. The gutter name is the common start of the ids and the phases (`I ABCN [A]`) instead of all ids, which don't fit the gutter. Trace names that would share a cell are moved sideways. `layout` takes a minimum height per analog row.

## Goal

Several analog channels in one plot row, for example the phase currents IA,
IB and IC in one trace, so their size and phase relation can be compared at
a glance and more fits on the screen. Grouping is optional: without it the
plot area stays as it is (one row per channel).

## What exists

- The plot area draws one row per shown analog channel (at least 5 rows high,
  the area scrolls), the gutter shows the name, the value at the cursor and
  the value range. Colors come from the phase (`Theme::channel`).
- The RMS (`r`) and harmonics (`f`) views use the same rows; the harmonics view
  already draws several rows per channel (one per order).
- `analysis::sequence::groups` finds three-phase groups: phases A, B, C of
  the same circuit and unit (also `L1`/`L2`/`L3`, `R`/`S`/`T`).
- The selected channel of the sidebar is marked with `▶` in its rows.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Switching it | Key `g` (free), binding `group_view`: groups on / off. Off after start; the setting stays when another file is opened (like `d`). Footer `grouped: 2 groups` / `one row per channel` | Always grouped: the issue asks for it as an option |
| Which channels form a group | The three-phase groups found by unit (`sequence::groups`), plus a neutral channel of the same circuit and unit (phase `N`, `E` or `G`, for example IN), so `IA IB IC IN` is one row. Every other channel keeps its own row | Groups from the configuration file: more flexible, but needs a format and an editor. Could come later (`todo.md`) |
| Hidden channels | Only the shown channels of a group (sidebar `●`) are in its row; with one shown it is a normal row | Always all channels of a group |
| Order | The group's row is where its first shown channel would be | Groups first, then the rest |
| Drawing | All traces of the group in one plot on a common y range, each in its phase color. Without colors the traces are told apart by the phase name at the right edge of each trace and at the cursor (`A`, `B`, `C`, `N`), as in the impedance view. `agent.md` says color must never be the only cue | Dashed or dotted lines: braille has no line styles |
| Gutter | First line the members (`IA IB IC IN [A]`, cut when too long), then one line per member with its value at the cursor (`A ▸ 318.2`), then the range (`↑` / `↓`). A group row is at least 3 + number of members rows high (7 for four traces) | Only the name: the values at the cursor are what the gutter is for |
| RMS and harmonics views | Grouping works there too: the RMS row draws the members' RMS, the harmonics view one row per group and order (`IA IB IC h3 [%]`) | Waveform only |
| Impedance view | Not affected (it draws R-X diagrams, no channel rows) | |
| Selected channel | The group's row gets the `▶` when a member is selected in the sidebar | |
| Units | A group has one unit (`sequence::groups` checks it), so the common y axis is in that unit | |

## Output (sketch)

```
┌ Plot · grouped ────────────────────────────── cursor 40.0 ms · window 199.5 ms ┐
│▶ IA IB IC IN [A]  ⡠⠒⢄    ⡠⠒⢄    ⡠⠒⢄       A  │
│A ▸ 318.2        ⡜ ⡠⠤⡀⢣  ⡜⢀⠤⡀⢣ ⡜          B  │
│B ▸ 3.458k      ⡸ ⡸   ⢱⠈⡆⡸⡜   ⢣⢱          C  │
│C ▸ 318.2       ⠁⡰⠁   ⠈⢆⠃⡰⠁    ⢇⠁         N  │
│N ▸ 3.140k      ⠜     ⠈⢆⡰     ⠈⢆             │
│↑ 4.980k                                       │
│↓ -4.980k                                      │
│VA VB VC VN [kV]  …                            │
│…                                              │
│START_A   ▁▁▁▁▁▁▁▁████████████▁▁▁▁▁▁▁▁▁▁▁       │
└───────────────────────────────────────────────┘
```

(Layout illustrative.)

## Implementation plan

### 1. Library: `src/analysis/sequence.rs`

- `pub fn neutral(channels, group: &Group) -> Option<usize>`: a channel with
  phase `N` / `E` / `G` of the group's circuit and unit.
- Unit tests: IN joins IA IB IC; a neutral of another unit or circuit
  doesn't; no neutral gives `None`.

### 2. TUI

- `tui/app.rs`: `App::grouped: bool`, `Action::GroupView` (`g`), footer message.
- `tui/keys.rs`: binding `group_view` (`g`), help text.
- `tui/recording.rs`: the groups of the recording (three-phase plus
  neutral), found once (`OnceCell`, as `track`), and a function that turns
  the shown analog channels into rows: a single channel or the shown members
  of a group.
- `tui/widgets/plot.rs`:
  - The analog rows become "a set of channels" (one or a group) times the
    harmonic order; `layout` gets each row's minimum height (5, or 3 +
    members).
  - `analog_plot` draws all members with a common y range, the trace names
    at the right edge and at the cursor, and the gutter lines.
  - The title says ` · grouped` when grouping is on.
- The sidebar doesn't change: channels are still shown and hidden one by one.

### 3. Tests (TUI, `tui/widgets/mod.rs`)

- `fault_2013` (IA IB IC IN, VA VB VC VN):
  - `g` puts IA IB IC IN in one row: gutter `IA IB IC IN [A]` and four value
    lines at the cursor, the same values as in the ungrouped rows.
  - `g` again gives the old layout.
- Hiding IB in the sidebar leaves `IA IC IN` in the row.
- The RMS view grouped: the four RMS values.
- The harmonics view grouped: one row per group and order.
- `▶` on the group row when IB is selected.
- No colors: the names `A` `B` `C` `N` are drawn in the plot.
- `fault_1999` (IA IB IC and one voltage): only the currents are grouped.
- The layout with the larger minimum height scrolls correctly at 80×24.

### 4. Docs

- `docs/src/viewer/index.md` (plots: grouping, key `g`), `keys.md`,
  `spec.md` TUI section, README viewer keys.
- `todo.md`: tick the item; add "groups from the configuration file" as an
  idea.
- `tasks/README.md`: the status of this task.

### 5. Checks

`cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
`cargo test --locked`, `cargo +1.88 check --locked --all-targets`,
`mdbook build docs`.
