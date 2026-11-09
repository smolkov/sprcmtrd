# TUI: scroll position in the channel list and the file picker

- **Issue:** none, from `todo.md` ("Show the scroll position in the long
  lists too")
- **Status:** planned, waiting for approval

## Problem

The sidebar channel list and the file picker scroll with the selection, but
nothing shows that more entries are above or below, or how far down the
selection is. A recording with 40 channels at 24 rows, or a picker with 340
recordings, looks like a short list until you move through it.

## What exists

- `widgets/channels.rs`: a `List` in a bordered block (title ` Channels `),
  analog channels, a ` binary ` separator row, then binary channels;
  `ListState` with the selected row (the separator skipped). The list
  scrolls by itself; its offset after rendering is in `ListState::offset()`.
- `widgets/filepicker.rs`: an overlay with the filter line, a `Table` of
  directories and recordings (`TableState`), and a bottom line that says
  `340 of 512 entries` (shown of found), an error, or "no recordings here".
- `widgets/plot.rs`: when the plot rows scroll, a `Scrollbar` on the block's
  right border (track `│` in the border style, thumb in the accent, no
  arrows) and `channels 4–9 of 18` in the title. The new indicators follow
  it.
- Tests: `app_with_picker(n)` fills a picker by hand (`widgets/mod.rs`).

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| What | Both: a scrollbar (where more entries are) and a count (where the selection is) | Only one of them; the scrollbar alone gives no number, the count alone no picture |
| When | Only when the list doesn't fit; a short list looks as today | Always: a count on every short list is noise |
| Sidebar scrollbar | On the block's right border, rows inside the block, as in the plot area: the sidebar is narrow (18 to 32 columns), a column inside would cut channel names | A column inside the block |
| Sidebar count | Bottom border, right: ` 12 / 40 `, the selected channel's position among all channels (analog and binary, the separator not counted) | In the title (` Channels 12/40 `): the title is short anyway, but the bottom border is free |
| Picker scrollbar | The rightmost column of the table area (the overlay's border is shared with the filter line and the bottom line) | On the overlay's right border, beside the table rows only |
| Picker count | The bottom line: `12 / 340 · 512 entries` when a filter hides some, `12 / 512 entries` otherwise. Errors and "no recordings" stay as today | Keep `340 of 512 entries` and put the position elsewhere |
| Look | As in the plot area: track `│` in the border style, thumb in the accent; without colors the thumb is the block `█` (ratatui's default), so it shows without color | |
| Focus | The count and the scrollbar show whether the list is focused or not | Only when focused |

## Output (sketch)

Sidebar, 40 channels, 10th selected, at 24 rows:

```
┌ Channels ──────┐
│● IA           A│
│● IB           A█
│● IC           A█
│● IN           A│
│...             │
└──────── 10 / 40 ┘
```

File picker bottom line, filter `2024`:

```
  12 / 340 · 512 entries
```

## Implementation plan

1. **Helper** (`widgets/mod.rs`): `scrollbar(f, track, total, viewport,
   offset, theme, focused)` drawing the plot area's scrollbar; `plot.rs`
   uses it too instead of its own code.
2. **Sidebar** (`widgets/channels.rs`): after `render_stateful_widget`, when
   the rows exceed the inner height: the scrollbar on the right border from
   `state.offset()`, and ` {n} / {total} ` as a right-aligned bottom title.
3. **Picker** (`widgets/filepicker.rs`): the table one column narrower when
   it overflows, the scrollbar in that column from `state.offset()`; the
   bottom line with the position.
4. **Tests** (`widgets/mod.rs`, `TestBackend`):
   - Sidebar: `fault_2013` (20 channels) at 80×20: ` 1 / 20 ` on the bottom
     border and a thumb on the right border; after `End` ` 20 / 20 ` and the
     thumb at the bottom; at 80×40 neither. The separator row isn't counted.
   - Picker: `app_with_picker(100)`: `1 / 100 entries`, a thumb in the last
     column; after `End` the thumb at the bottom; a filter that hides some
     gives `k / m · 100 entries`; `app_with_picker(3)`: no scrollbar.
   - `Theme::plain`: the thumb symbol is there without colors.
5. **Docs:** `spec.md` (sidebar, file picker overlay), `README.md` and
   `docs/src/viewer/` (sidebar and file picker), `todo.md`,
   `tasks/README.md` status.
6. **Checks:** the four commands from `agent.md` (`cargo test` with
   `--no-fail-fast`), `mdbook build docs`, and a look in `tmux` at 80×24
   with `fault_2013` and with the picker on `tests/data`.
