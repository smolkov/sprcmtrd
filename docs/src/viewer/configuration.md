# Configuration

## Configuration overlay

`c` (or `Ctrl+,`) shows the theme and every key binding. Changes take effect
immediately and are saved to the configuration file:

- Theme row: `←`/`→` go to the previous / next theme, `Enter` to the next.
- Binding row: `Enter`, then press the new key (`Esc` cancels). Keys already
  used by another action are refused. `Backspace` restores the default.

## Configuration file

`~/.config/sprcmtrd/config.toml` (Linux; the platform config directory
elsewhere, or the path in `SPRCMTRD_CONFIG`). It is created by the
configuration overlay when missing and can also be edited by hand:

```toml
theme = "nord"                 # dark | light | auto | a theme name

[keys]                         # action = list of keys, replaces the defaults
zoom_in = ["+", "="]
open = ["o", "ctrl+p"]
pan_left = ["shift+left", "H"]
```

Key names: single characters (`q`, `T`, `?`, `,`), `space`, `tab`, `enter`,
`esc`, `backspace`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`,
`pagedown`, with `ctrl+`, `alt+` or `shift+` prefixes. Action names are the
ones in the configuration overlay (`quit`, `open`, `zoom_in`, …). An invalid
file is reported at start. Saving from the overlay rewrites the whole file, so
comments are not kept. `--theme` and `--no-color` override the file.

## Color themes

Built-in themes: `dark` and `light` use the 16 ANSI colors, so they follow
your terminal's color scheme; `atom-one-dark`, `nord`, `nord-night` (Nord's
colors on a darker background), `solarized-dark` and `tokyonight` (the night
variant) use their own RGB colors, rounded borders and italic secondary text.
Choose one with `--theme NAME`, `theme = "NAME"` in the configuration file,
the theme row of the configuration overlay, or `T` (next theme, not saved).
`sprcmtrd themes` lists them.

RGB colors need a terminal with truecolor (`COLORTERM=truecolor` or `24bit`);
otherwise they are shown as the nearest of the 256 colors. A terminal program
can't change the font or its size; a theme sets the text styles (bold,
italic, …) and the border lines instead.

Your own themes are `<name>.toml` files in the `themes` directory next to the
configuration file (`~/.config/sprcmtrd/themes/`). A file with a built-in name
replaces that theme. Missing entries come from `extends`, or else from the
built-in `dark` or `light` theme of the same `kind`:

```toml
# ~/.config/sprcmtrd/themes/my-nord.toml
kind = "dark"                  # dark | light
extends = "nord"               # optional

[colors]
accent = "#ebcb8b"             # "#rrggbb", ANSI name ("light-red"), 0-255, "reset"
background = "reset"           # keep the terminal's background

[style]
border = "thick"               # plain | rounded | thick | double
muted = ["dim"]                # bold | italic | dim | underlined | reversed
```

`sprcmtrd themes --print NAME` writes a complete theme with every entry, a
good start for your own:

| `[colors]` | Used for |
|---|---|
| `background`, `text` | Every area |
| `muted` | Secondary text, hints |
| `border`, `accent` | Borders; the focused border, titles and highlights |
| `selection` | Background of the selected row |
| `plot_shades` | Two backgrounds that alternate between the analog plots |
| `warning`, `error` | Messages, abnormal binary states |
| `trigger`, `cursor` | Trigger marker, plot cursor |
| `phase` | Phases A, B, C, N (also L1–L3, R/S/T) |
| `palette` | 8 colors for channels without a known phase |

`[style]` has `border` and the modifiers of `title` (focused block title),
`accent`, `selection`, `warning`, `error` and `muted`.

Only the chosen theme is read: a broken file is an error at start (with the
file and position), and skipped with a message when `T` or the overlay reach
it. `--no-color` / `NO_COLOR` show no colors whatever the theme.

## Logging

CLI commands log warnings to stderr. The TUI never logs to the screen: with
`-v` (or `RUST_LOG`) it writes to `~/.local/state/sprcmtrd/sprcmtrd.log`
(the platform state or cache directory elsewhere).
