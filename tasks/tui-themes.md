# TUI: named color themes from TOML files, text styles

- **Issue:** none, from the user ("improve themes and font; themes can be
  defined with a config file, e.g. in the config directory in
  `themes/theme.toml`; implement atom-one-dark, nord, solarized-dark,
  nord-night, tokyonight")
- **Status:** done, 1.0.0. Built as proposed,
  except:
  - The theme files, `Catalog` and `ThemeSpec` are in `src/term/themes/`
    (built-in files `src/term/themes/*.toml`), not in `src/tui/theme/`. The
    `themes` command, the command output colors and shell completion need
    them too, and `command` and `tui` must not use each other
    (`tests/layering.rs`). `tui/theme.rs` stays one file: `Theme` built from a
    `ThemeSpec`, with `block()` and `title(focused)`.
  - `nord-night` uses the proposed colors (Nord on `#1b1e25`). The question
    of which "Nord Night" was meant is still open.
  - The commands never fail on an unknown or broken theme: their colors fall
    back to dark (a warning in the log). Only the TUI and `themes --print`
    report it as an error.
  - `sprcmtrd themes` lists in the `T` order (built-in, then user themes by
    name), not alphabetically. A broken file is listed as `invalid` and its
    error goes to stderr.

## Goal

Besides dark and light, the TUI offers well-known color schemes: Atom One Dark,
Nord, Nord Night, Solarized Dark and Tokyo Night. A theme is a TOML file, so a
user can add their own in `~/.config/sprcmtrd/themes/<name>.toml` or change a
built-in one, without building the program again. A theme also sets text
styles (bold, italic, dim) and the border style. This is the part of "font"
a terminal program can control.

## What exists

- `tui/theme.rs`: `Theme` has named roles (`background`, `text`, `muted`,
  `border`, `accent`, `selection`, `plot_shades[2]`, `warning`, `error`,
  `trigger`, `cursor`, `phase[4]`, `palette[8]`, `plain`) and the constructors
  `dark()`, `light()`, `plain()`. All of them use the 16 ANSI colors (and
  `Indexed(234/254)` for the plot shades). Widgets use only these roles and the
  style helpers (`base`, `muted`, `border`, `selection`, `accent`, `warning`,
  `error`). No widget has a hard-coded color.
- Style helpers hard-code the modifiers: `accent`, `warning` and `error` are
  bold. Borders are always `BorderType::Plain`.
- `term/theme.rs`: `ThemeKind` (`Dark` / `Light`) with `resolve(ThemeChoice)`
  (`auto` through `COLORFGBG`) and `toggled()`. `term/style.rs::Palette` (command
  output) uses only the `ThemeKind`.
- `args.rs`: `ThemeChoice` is a `ValueEnum` (`dark | light | auto`) used by
  `--theme` and `config.toml` (`theme = "dark"`).
- `tui/app.rs`: `T` (`Action::Theme`) toggles dark / light and doesn't save. The
  theme row of the configuration overlay toggles too, and `save_config`
  writes `theme = "dark" | "light"`.
- `config.rs`: `AppConfig::default_path()` gives `~/.config/sprcmtrd/config.toml`
  (or `$SPRCMTRD_CONFIG`). The themes directory sits next to it.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| "Font" | A terminal program can't choose the font or its size; the terminal does that. A theme's `[style]` sets the modifiers per role (`title`, `accent`, `selection`, `warning`, `error`, `muted`: any of `bold`, `italic`, `dim`, `underlined`, `reversed`) and `border = "plain" \| "rounded" \| "thick" \| "double"`. Built-in themes: rounded borders, italic muted text in the RGB themes | Also the plot marker (`braille`, `dot`, `half-block`, `block`) per theme. It is a resolution choice, not a style, so a separate `plot_marker` setting would fit better |
| Where | Built-in themes are TOML files in the source (`src/tui/themes/*.toml`, `include_str!`), so they use the same parser and format as user files. User themes are in `<config dir>/sprcmtrd/themes/<name>.toml`, the name is the file stem. A user file with a built-in name replaces that theme | Every theme in `config.toml` (`[themes.nord]`): one file, but it gets long and the overlay would rewrite it |
| Format | `kind = "dark" \| "light"`, an optional `extends = "<theme>"` and `[colors]` with the role names of `Theme`. A color is `"#rrggbb"`, an ANSI name (`"light-red"`), an index of the 256 colors (`"234"`) or `"reset"` (the terminal's own color). `ratatui::style::Color::from_str` parses all of them. Missing roles come from `extends`, and without it from `dark` or `light` by `kind`. Unknown keys are an error (`deny_unknown_fields`, as in `config.toml`) | Every role required: no surprises, but a small change means copying the whole file |
| Built-in names | `dark`, `light` (unchanged, ANSI), `atom-one-dark`, `nord`, `nord-night`, `solarized-dark`, `tokyonight` (the "night" variant). Colors in the table below | Also `solarized-light` and `tokyonight-day`, so a light background has more than one choice (cheap to add later) |
| `nord-night` | Nord's colors on a darker background (`#1b1e25`, shades `#242933` / `#1b1e25`, selection `#2e3440`). **Please confirm which "Nord Night" you mean.** If there is a scheme you use (editor or terminal), I'll take its colors | Leave it out until the source is clear |
| `--theme` / `config.toml` | `ThemeChoice` becomes a name: `dark`, `light`, `auto` or any theme name, so `theme = "dark"` in existing files stays valid. An unknown name is an error that lists the names. Shell completion offers the built-in and user names (dynamic completion, as for recordings) | Keep `--theme dark\|light\|auto` and add `--color-scheme <name>`: two options for one thing |
| `auto` | Picks `dark` or `light` as today (`COLORFGBG`) | Two settings `theme_dark` / `theme_light` for `auto`: more setup for little gain |
| Truecolor | RGB colors are sent as they are when `COLORTERM` is `truecolor` or `24bit`. Otherwise each one is mapped once, at load time, to the nearest of the 256 colors, so tmux without RGB and the Linux console stay readable | Always RGB: wrong colors in terminals without truecolor |
| Background | The RGB themes paint their background, as `dark` / `light` do now. `background = "reset"` keeps the terminal's (also transparent) background | Never paint the background: then the plot shades don't fit every terminal |
| `T` at runtime | Goes to the next theme in a fixed order (built-in, then user themes by name) and the footer shows `theme: nord`. As today it isn't saved | Keep the dark / light toggle, and pick a named theme only in the overlay |
| Configuration overlay | The theme row shows `◀ nord ▶`; `←`/`→` go through all themes, `Enter`/`Space` go forward. The hint names the file of a user theme. It is saved as `theme = "<name>"` | A list overlay with a preview: nicer, more code, could come later |
| Broken theme file | Only the selected theme is checked when it loads. At startup a broken file is an error with the path and the TOML position, like a broken `config.toml`. If it is reached with `T` or in the overlay, it is skipped and the footer shows the error. Other files in `themes/` are not read until they are chosen (the names come from the file names) | Read all files at startup and refuse to start on any error: one broken file blocks the program |
| Command output | `Palette` uses only the theme's `kind` (16 ANSI colors as today), so `sprcmtrd check` doesn't change color with the TUI theme | Use the theme's `error` / `warning` / `accent` / `muted` in the command output too |
| Theme command | `sprcmtrd themes` lists the names, built-in or the user file. `sprcmtrd themes --print <name>` writes the theme as complete TOML to stdout as a template for a custom theme | Only documentation with an example file |

### Colors of the new themes

Roles as in `Theme`. Phase A/B/C/N stays red/green/blue/neutral, as in
`dark`. Shades: `[lighter, darker]`.

| Role | atom-one-dark | nord | nord-night | solarized-dark | tokyonight |
|---|---|---|---|---|---|
| background | `#282c34` | `#2e3440` | `#1b1e25` | `#002b36` | `#1a1b26` |
| text | `#abb2bf` | `#d8dee9` | `#d8dee9` | `#839496` | `#c0caf5` |
| muted | `#5c6370` | `#616e88` | `#616e88` | `#586e75` | `#565f89` |
| border | `#4b5263` | `#4c566a` | `#3b4252` | `#586e75` | `#414868` |
| accent | `#61afef` | `#88c0d0` | `#88c0d0` | `#268bd2` | `#7aa2f7` |
| selection | `#3e4451` | `#434c5e` | `#2e3440` | `#073642` | `#283457` |
| plot_shades | `#2c313a` / `#282c34` | `#3b4252` / `#2e3440` | `#242933` / `#1b1e25` | `#073642` / `#002b36` | `#1f2335` / `#1a1b26` |
| warning | `#e5c07b` | `#ebcb8b` | `#ebcb8b` | `#b58900` | `#e0af68` |
| error | `#e06c75` | `#bf616a` | `#bf616a` | `#dc322f` | `#f7768e` |
| trigger | `#c678dd` | `#b48ead` | `#b48ead` | `#d33682` | `#bb9af7` |
| cursor | `#d19a66` | `#d08770` | `#d08770` | `#cb4b16` | `#ff9e64` |
| phase A B C N | `#e06c75` `#98c379` `#61afef` `#abb2bf` | `#bf616a` `#a3be8c` `#81a1c1` `#d8dee9` | as nord | `#dc322f` `#859900` `#268bd2` `#93a1a1` | `#f7768e` `#9ece6a` `#7aa2f7` `#a9b1d6` |
| palette | `#56b6c2` `#c678dd` `#e5c07b` `#98c379` `#61afef` `#be5046` `#d19a66` `#abb2bf` | `#8fbcbb` `#b48ead` `#ebcb8b` `#a3be8c` `#5e81ac` `#d08770` `#88c0d0` `#eceff4` | as nord | `#2aa198` `#6c71c4` `#b58900` `#859900` `#268bd2` `#cb4b16` `#d33682` `#93a1a1` | `#7dcfff` `#bb9af7` `#e0af68` `#9ece6a` `#2ac3de` `#ff9e64` `#1abc9c` `#c0caf5` |

## Output (examples)

`~/.config/sprcmtrd/themes/my-nord.toml`, a small change to a built-in theme:

```toml
kind = "dark"
extends = "nord"

[colors]
accent = "#ebcb8b"
background = "reset"        # keep the terminal background

[style]
border = "thick"
muted = ["dim"]
```

`src/tui/themes/tokyonight.toml` (complete, also the output of
`sprcmtrd themes --print tokyonight`):

```toml
kind = "dark"

[colors]
background = "#1a1b26"
text = "#c0caf5"
muted = "#565f89"
border = "#414868"
accent = "#7aa2f7"
selection = "#283457"
plot_shades = ["#1f2335", "#1a1b26"]
warning = "#e0af68"
error = "#f7768e"
trigger = "#bb9af7"
cursor = "#ff9e64"
phase = ["#f7768e", "#9ece6a", "#7aa2f7", "#a9b1d6"]
palette = ["#7dcfff", "#bb9af7", "#e0af68", "#9ece6a", "#2ac3de", "#ff9e64", "#1abc9c", "#c0caf5"]

[style]
border = "rounded"
title = ["bold"]
accent = ["bold"]
selection = ["bold"]
warning = ["bold"]
error = ["bold"]
muted = ["italic"]
```

`sprcmtrd themes`:

```
NAME            KIND   SOURCE
atom-one-dark   dark   built-in
dark            dark   built-in
light           light  built-in
my-nord         dark   ~/.config/sprcmtrd/themes/my-nord.toml
nord            dark   built-in
nord-night      dark   built-in
solarized-dark  dark   built-in
tokyonight      dark   built-in
```

Configuration overlay:

```
│Appearance                                                               │
│  Theme           ◀ tokyonight ▶      T cycles; files in themes/         │
```

## Implementation plan

1. **Theme files** (`src/tui/theme/`, from `tui/theme.rs`): `mod.rs` keeps
   `Theme` and its helpers. It gets `kind: ThemeKind`, `styles: Styles` (the
   modifiers per role) and `border: BorderType`. The helpers use `styles`
   instead of the hard-coded `BOLD`, and `Theme::block()` gives a `Block` with
   the theme's border type. `file.rs` has `ThemeFile` (serde,
   `deny_unknown_fields`), the color parsing through `Color::from_str`, `extends`
   (cycles are an error), the 256-color mapping when there is no truecolor, and
   `to_toml()` for `--print`. `catalog.rs`
   lists the names (built-in in `themes/*.toml`, plus the user directory),
   `load(name)` and `next(name)`. `dark` and `light` become TOML files too.
   Tests check that they give exactly the `Theme` values of today.
2. **Widgets:** every `Block::bordered()` / `Block::default().borders(..)`
   goes through `theme.block()`, and titles use `theme.title()`. Check with
   `grep` that no `BorderType` or modifier for a theme role is left in the
   widgets.
3. **Options and configuration** (`args.rs`, `context.rs`, `config.rs`):
   `ThemeChoice` becomes `Auto | Named(String)` with `FromStr` / serde from a
   string. `--theme` gets a dynamic completion of the catalog names.
   `AppConfig::themes_dir()` is next to `default_path()` (with
   `$SPRCMTRD_CONFIG` it is the `themes/` directory next to that file).
   `Palette` resolves to the `kind` of the named theme.
4. **App** (`tui/app.rs`, `tui/keys.rs`, `widgets/config.rs`): `theme_name:
   String` instead of `theme_kind`. `T` and the overlay row call `catalog.next`.
   The footer shows the name or the load error. `save_config` writes the name.
   The help text of `theme` becomes "Next color theme".
5. **Command** (`command/themes.rs`, `args.rs`): `themes` with `--print
   <name>`, list output as in the other list commands (`term::LiveTable` not
   needed, it is short).
6. **Tests:** parse every built-in file and compare it with the table above;
   `extends` and the fallback by `kind`; unknown key, bad color and `extends`
   cycle are errors with the file name; a user file replaces a built-in; the
   256-color mapping (`#1a1b26` → nearest index, without `COLORTERM`);
   `--theme nord` and `theme = "nord"`, and `theme = "dark"` still valid; an
   unknown name lists the names; `T` cycling and skipping a broken user file
   with a footer message; the overlay row and saving the name; render tests
   (`TestBackend`) with `nord`: background and accent of the focused border
   from the theme, rounded border characters; `Theme::plain` unchanged (no
   colors, `--no-color` beats a named theme); CLI test for `themes` and
   `themes --print tokyonight` (parses again into the same theme).
7. **Docs:** `spec.md` (Theme section, configuration overlay, `themes`
   command, `--theme`), `README.md` (options, `T`, configuration, a theme
   file example), `docs/src/viewer/configuration.md` (theme format reference
   with all roles and style keys), `docs/src/commands/` (`themes`),
   `tasks/README.md` status, `todo.md`.
8. **Checks:** the four commands from `agent.md`, `mdbook build docs`, and a
   look in `tmux` at every built-in theme, with `COLORTERM=truecolor` and
   without it (256-color mapping) and with `--no-color`.
