# TUI: show the plot views as tabs

- **Issue:** none, from the user ("it is not visible which different views the plot has")
- **Status:** done, unreleased. Built as planned, except:
  - Two more fallback levels after the planned ones: first the inactive
    tabs go (only the current view and the group key), then the group
    toggle. At 80 columns with the sidebar the planned levels don't fit
    beside the right title, which would cut off the details. On the last
    level the title reads as before (` Harmonics 1 3 · channels …`, no
    brackets).
  - The harmonic orders follow the tabs after ` · ` (`┃ g · [1] 2 [3]`).
  - At 110 columns with the sidebar and a scroll position, the title shows
    the short form (`w r [Harmonics] z ┃ g · …`). The sketch at 110 columns
    assumed the whole width for the plot.

## Problem

The plot area has four views: waveform, RMS (`r`), harmonics (`f`) and
impedance (`z`). The block title names only the current one (` Plot `,
` RMS `, …), so a user can't tell that the others exist or how to reach them.
The keys are only in the help overlay (`?`), not in the footer, whose hints
already run past 110 columns. Going back to the waveform means pressing the
key of the current view again, which is also nowhere to be seen. Grouping
(`g`) shows only as ` · grouped` in the title once it is on; while it is off,
nothing hints at it.

## What exists

- `PlotView` (`tui/app.rs`): `Waveform`, `Rms`, `Harmonics`, `Impedance`;
  `toggle_rms_view` / `toggle_harmonics_view` / `toggle_impedance_view` switch
  to a view or back to the waveform, and say in the footer why a view can't be
  opened (no line frequency, no voltage and current groups).
- `widgets/plot.rs::render` builds the left title from the view name,
  ` · grouped`, the harmonic orders (` Harmonics [1] 2 [3] …`), the impedance
  window (` · -20.0 ms … 80.0 ms · primary Ω`) and the scroll position
  (` · channels 1–4 of 10`); the right title has the cursor and the window. The
  orders fall back to the shown ones when the long form doesn't fit.
- Key labels come from the keymap (`app.keymap.label(action)`), so a rebound key
  shows correctly (`agent.md`: never hard-code key hints).

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Where | The plot block's left title becomes a tab bar: ` Plot │ RMS │ Harmonics │ Impedance `, then the details of the current view (orders, grouped, scroll position, impedance window) after ` · `. No extra row | A tab row of its own above the plots: one plot row less |
| Current view | Accent and reversed when the plot is focused, bold otherwise; in brackets with `--no-color` (`[RMS]`), so it never depends on color alone. `RMS-DC` with DC removed | Underline: hard to see in some terminals |
| Keys | Each tab shows its key from the keymap before the name, muted: `w Plot │ r RMS │ f Harmonics │ z Impedance` | Keys only in the help overlay as today |
| Back to the waveform | New binding `waveform_view`, key `w` (free), opens the waveform from any view. `r` / `f` / `z` still toggle back as before | Only toggling: then the `Plot` tab has no key to show |
| Cycling | New binding `next_view`, key `v` (free): the next view that can be opened, skipping unavailable ones, wrapping to the waveform | No cycling key: four keys suffice |
| Unavailable views | Shown muted and crossed out (`crossed_out` modifier) when the recording has no line frequency (RMS, harmonics, impedance) or no voltage and current groups (impedance); without color the strike remains. Pressing the key keeps the footer message as today | Hide them: then the user doesn't learn that they exist |
| Narrow plot area | Dropped in this order until the title fits beside the right title: the keys, then the inactive tab names shortened to their key (`w r [Harmonics] z`), then the harmonic orders to the shown ones (as today). The current view's name always stays | Wrap the title: a block has only one title row |
| Grouping | A toggle after the view tabs, set apart by ` ┃ `: `g Group`, in brackets and highlighted when on (`[g Group]`), muted when off. It replaces the ` · grouped` detail. In the impedance view, where grouping doesn't apply, it is crossed out like an unavailable view; `g` still switches it, as today. In a narrow title it shrinks to `g` / `[g]` together with the tabs | Keep ` · grouped` only when on: then grouping stays invisible while it is off |
| Help overlay | `w` and `v` appear in the existing view group of `?`; the help line of `r` / `f` / `z` stays | |

## Output (sketch)

110 columns, waveform focused, line frequency and groups present:

```
┌ [w Plot] │ r RMS │ f Harmonics │ z Impedance ┃ g Group · channels 1–4 of 10 ── cursor 0.0 ms · window 2.000 s ┐
```

Harmonics view, grouped, narrower (80 columns of plot area):

```
┌ w r [Harmonics] z ┃ [g] · [1] 2 [3] 4 5 · channels 1–2 of 8 ── cursor 0.0 ms · window 2.000 s ┐
```

No line frequency in the configuration (RMS, harmonics and impedance crossed out):

```
┌ [w Plot] │ r R̶M̶S̶ │ f H̶a̶r̶m̶o̶n̶i̶c̶s̶ │ z I̶m̶p̶e̶d̶a̶n̶c̶e̶ ┃ g Group ──── cursor 0.0 ms · window 1.000 s ┐
```

## Implementation plan

1. **Keys** (`tui/keys.rs`): `Action::WaveformView` (`waveform_view`, `w`) and
   `Action::NextView` (`next_view`, `v`) in `BINDINGS`, next to `rms_view`; the
   configuration overlay and `config.toml` names follow from the table.
2. **App** (`tui/app.rs`): `PlotView::ALL`, `PlotView::available(&Recording)`
   (one place for the line frequency and loop checks the toggles use now),
   `show_waveform`, `next_view`; the toggles call `available` for their checks.
3. **Title** (`widgets/plot.rs`): a `view_tabs(app, rec, width) -> Vec<Span>`
   helper that builds the tab spans, the group toggle and the fallback levels; `render` puts the
   tabs and the current view's details into the left title as a `Line` of
   spans instead of one string. The empty state (no recording) shows the tabs
   without details, all muted.
4. **Tests** (`widgets/mod.rs`, `TestBackend`): the tabs and keys at 110
   columns; the current view in brackets with `Theme::plain`; the fallback at
   80 and 60 columns; crossed-out tabs without a line frequency (style check on
   the buffer); `w` from each view; `v` cycling and skipping unavailable views;
   a rebound key shown in its tab; the group toggle off, on (brackets with
   `Theme::plain`) and crossed out in the impedance view. Existing title assertions (` Harmonics [1] 2
   …`, ` Impedance · -20.0 ms …`) updated to the new title.
5. **Docs:** `spec.md` (TUI layout: plot title, keys table with `w` and `v`),
   `README.md` keys, `docs/src/` TUI page, `tasks/README.md` status.
6. **Checks:** the four commands from `agent.md`, `mdbook build docs`, and a
   look in `tmux` at 110 and 80 columns with and without `--no-color`.
