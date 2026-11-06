# Faster movement in the channel list and overlays

- **Issue:** [smo/sprcmtrd#7](https://gitlab.fe.sprecher-automation.com/smo/sprcmtrd/-/issues/7), "Shortcut für das schnellere Bewegen in der Channelliste"
- **Branch:** `7-shortcut-fur-das-schnellere-bewegen-in-der-channelliste`
- **Status:** done; committed in `70f8674` and pushed to `gitlab`

## Goal

In recordings with many binary channels, moving through the channel list with
the arrow keys is slow. `Ctrl+↓` / `Ctrl+↑` should move the selection faster.

The issue says "Maustasten"; this is read as arrow keys, because the TUI has no
mouse support yet.

This task also adds, beyond the issue:

- `Home` / `End` for the first and last channel.
- Jump, `Home` and `End` keys in the help overlay and file picker. The recording
  info overlay gets them too, because it shares the help overlay's scrolling.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Jump size | Fixed 10 rows everywhere, the same step `PgUp`/`PgDn` use in the file picker | One visible page: adapts to terminal height, but needs extra state set during drawing |
| Fallback keys for the jumps | `J` / `K`: free, match `j` / `k`, and meet the `agent.md` rule for combinations terminals may not send (macOS Mission Control takes `Ctrl+↑/↓` by default). In the file picker, `J` / `K` type into the filter, so `PgUp`/`PgDn` remain the fallback there | None |
| Plot has focus | Jump, `Home` and `End` do nothing: `↑/↓` zoom there, `PgUp`/`PgDn` already scroll the plots, and an invisible selection doesn't move when the sidebar is hidden | Move the selection regardless of focus |
| File picker keys | Hard-coded like its other keys, so the picker's help text stays correct and `J` / `K` can't clash with typing. Rebinding `jump_up`, `jump_down`, `first` or `last` doesn't affect the picker, as with `up` and `down` today | Look them up in the key map |

## Implementation plan

The code below assumes the proposed decisions and hasn't been compiled. Re-read
`spec.md` before starting, as `agent.md` asks.

### 1. Add the actions and bindings (`src/tui/keys.rs`)

- [x] Add the variants at the end of `Action`, after `Close`:

  ```rust
  Close,
  JumpUp,
  JumpDown,
  First,
  Last,
  ```

  `KeyMap::action` returns the first match in enum order, so a key already bound
  to another action in a user's `config.toml` keeps working.

- [x] Add the bindings to `BINDINGS` after `Action::Down`, so the help and
      configuration overlays list them next to `up` and `down`:

  ```rust
  Binding {
      action: Action::JumpUp,
      name: "jump_up",
      keys: &["ctrl+up", "K"],
      help: "10 channels up (sidebar) / scroll up 10 lines",
      hint: None,
  },
  Binding {
      action: Action::JumpDown,
      name: "jump_down",
      keys: &["ctrl+down", "J"],
      help: "10 channels down (sidebar) / scroll down 10 lines",
      hint: None,
  },
  Binding {
      action: Action::First,
      name: "first",
      keys: &["home"],
      help: "First channel (sidebar) / scroll to top",
      hint: None,
  },
  Binding {
      action: Action::Last,
      name: "last",
      keys: &["end"],
      help: "Last channel (sidebar) / scroll to bottom",
      hint: None,
  },
  ```

No other change is needed for the configuration overlay or `config.toml`: they
are generated from `BINDINGS`, and `KeyMap::new` gives existing configs the new
default keys.

### 2. Add the shared movement helpers (`src/tui/app.rs`)

- [x] Add these next to `STATUS_TIME`. `First` and `Last` use the largest steps,
      which stop at the ends of the list:

  ```rust
  /// Rows moved by one jump.
  const JUMP: isize = 10;

  /// How far `action` moves a selection or scroll position, if it does.
  fn step(action: Action) -> Option<isize> {
      match action {
          Action::Up => Some(-1),
          Action::Down => Some(1),
          Action::JumpUp => Some(-JUMP),
          Action::JumpDown => Some(JUMP),
          Action::First => Some(isize::MIN),
          Action::Last => Some(isize::MAX),
          _ => None,
      }
  }

  /// Position after moving `by` rows, kept within the `n` rows.
  fn moved(position: usize, by: isize, n: usize) -> usize {
      position.saturating_add_signed(by).min(n.saturating_sub(1))
  }
  ```

### 3. Move the channel selection (`src/tui/app.rs`)

- [x] In `recording_action`, remove the sidebar `Up` and `Down` arms and handle
      all movement before the `match`:

  ```rust
  if focus == Focus::Sidebar && let Some(by) = step(action) {
      self.selected = moved(self.selected, by, n);
      self.plot_follow.set(true);
      return;
  }
  ```

`plot_follow` scrolls the plot to the selected channel. With the plot focused, the
new actions reach the existing `_ => {}` arm and do nothing, while the `Up` and
`Down` zoom arms stay. `App::action` already forwards the new actions to
`recording_action`, so without a recording they show the usual "no recording
loaded" message.

### 4. Scroll the help and info overlays

Today `overlay_action` stores a scroll offset that only the drawing code clamps.
Pressing `↓` past the end keeps increasing it, so `↑` then seems to do nothing
until the offset is back in range. `End` and jumps would make this much worse, so
the stored offset has to be clamped.

- [x] Add a field to `App`, following the `plot_page` pattern, and start it at 0
      in `App::new`:

  ```rust
  /// Largest scroll offset of the open text overlay at the last draw.
  pub overlay_scroll_max: Cell<u16>,
  ```

- [x] Store the value in `scrolled_text` (`src/tui/widgets/mod.rs`), after it
      computes `max`:

  ```rust
  app.overlay_scroll_max.set(max);
  ```

- [x] In `overlay_action`, remove the `Up` and `Down` arms and handle all movement
      before the `match`:

  ```rust
  if let Some(by) = step(action) {
      let rows = usize::from(self.overlay_scroll_max.get()) + 1;
      *scroll = moved(usize::from(*scroll), by, rows) as u16;
      return;
  }
  ```

### 5. Move in the file picker (`picker_key` in `src/tui/app.rs`)

- [x] Add the arms below and replace the existing `PageUp` and `PageDown` arms,
      which keep their behaviour. The `Ctrl` arms must come before the existing
      `KeyCode::Up` and `KeyCode::Down` arms, which match any modifier and would
      otherwise catch `Ctrl+↑/↓`:

  ```rust
  KeyCode::Up if key.modifiers.contains(KeyModifiers::CONTROL) => {
      p.selected = moved(p.selected, -JUMP, p.filtered.len());
  }
  KeyCode::Down if key.modifiers.contains(KeyModifiers::CONTROL) => {
      p.selected = moved(p.selected, JUMP, p.filtered.len());
  }
  KeyCode::PageUp => p.selected = moved(p.selected, -JUMP, p.filtered.len()),
  KeyCode::PageDown => p.selected = moved(p.selected, JUMP, p.filtered.len()),
  KeyCode::Home => p.selected = 0,
  KeyCode::End => p.selected = p.filtered.len().saturating_sub(1),
  ```

- [x] Update the picker's help line in `src/tui/widgets/help.rs`:

  ```rust
  "↑/↓ select, Ctrl+↑/↓ or PgUp/PgDn move 10, Home/End first and last",
  "Enter opens a file or directory",
  ```

### 6. Add the tests

- [x] Add the tests listed under [Tests](#tests). `moved()` and `step()` go in a
      new `#[cfg(test)]` module in `app.rs`.

### 7. Update the documentation

- [x] Add these rows after the `↑`/`↓` row in `README.md`,
      `docs/src/viewer/index.md` and `docs/src/viewer/keys.md`, and the same keys
      and actions in the `spec.md` key table, which has its own columns:

  ```markdown
  | `Ctrl+↑`/`Ctrl+↓`, `K`/`J` | Move 10 channels (sidebar), scroll 10 lines (help, info) |
  | `Home`/`End`        | First / last channel (sidebar), top / bottom (help, info) |
  ```

- [x] Add the picker keys to the `spec.md` file picker section (`↑`/`↓` line), the
      file picker section in `README.md` and `docs/src/viewer/index.md`, and
      `docs/src/viewer/file-picker.md`.
- [x] Note in `spec.md` that jump and `Home`/`End` scroll the plot like any other
      selection change.
- [x] Record the task as done in `todo.md`.

### 8. Verify

- [x] Run the real-terminal check and all commands under [Tests](#tests).
- [x] Inspect `git status` and `git diff`, then commit only when asked. The empty,
      untracked `note.md` stays out of the commit.

## Tests

- [x] Unit, `moved()`: steps of 10, stopping at both ends, `isize::MIN` and
      `isize::MAX`, empty list. The sample recordings look too small to show a
      full 10-channel step through the UI.
- [x] Unit, `keys.rs`: `ctrl+down`, `home` and `end` parse, survive a config round
      trip and map to the right actions; `J` still differs from `j`.
- [x] TUI, sidebar (`tui/widgets/mod.rs`):
  - A jump and `End` select the expected channel and cross the analog/binary
    separator; `Home` returns to the first channel.
  - The plot scrolls to the selected channel.
  - Nothing changes when the plot has focus.
- [x] TUI, overlays: at a small terminal size, `End` shows the last help line and
      `Home` the first. After `End`, one `↑` moves the view at once, and a jump
      past either end stays in range. Each key press needs a draw first, because
      the draw sets `overlay_scroll_max`.
- [x] TUI, file picker: build a `Picker` with `Picker::new` and `push` instead of
      running a scan. Check `Ctrl+↑/↓`, `PgUp`/`PgDn`, `Home`, `End`, an empty
      list, and that `J` types into the filter.
- [x] Real terminal: in `tmux`, send `C-Down`, `J`, `Home` and `End` in the sidebar
      with `long1_t1_anstatt_t2.cfg`/`.dat`, attached to the issue, then in the
      help overlay and file picker. Don't add the recording to `tests/data/`.
- [x] Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`,
      `cargo test --locked`, `cargo +1.88 check --locked --all-targets` and
      `mdbook build docs`.

## Results

- All listed commands pass: 100 unit and 35 integration tests.
- Real terminal, `long1_t1_anstatt_t2` (9 analog, 253 binary channels): `Ctrl+↓`,
  `J`, `K`, `Ctrl+↑`, `Home` and `End` select the expected channels, checked against
  the `.cfg` order.
- The `End` test for the help overlay runs at 140×24, not the minimum terminal size:
  at 80×24 long lines wrap, and `End` stops short (see "Wrapped overlay lines").
- Hidden channels: most binary channels in the sample start hidden. A jump can land
  on one, and the plot then stays where it is because there's no plot to scroll to.

## Not included

- Changes to the configuration overlay, which already has its own `Home` and `End`
  handling.
- The sidebar position indicator already listed in `todo.md`.
- Sidebar scroll position: `channels.rs` creates a new `ListState` on every draw,
  so the list doesn't remember its position. The selection stays on the bottom row
  once the list scrolls, so moving up doesn't show the channels below it. Possible
  follow-up.
- Wrapped overlay lines: `scrolled_text` counts lines before wrapping, so at 80×24
  neither `End` nor `↓` reaches the last lines of the help overlay. It already
  limited scrolling before this task. Possible follow-up.
