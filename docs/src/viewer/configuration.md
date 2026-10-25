# Configuration

## Configuration overlay

`c` (or `Ctrl+,`) shows the theme and every key binding. Changes take effect
immediately and are saved to the configuration file:

- Theme row: `Enter` or `←`/`→` switches dark / light.
- Binding row: `Enter`, then press the new key (`Esc` cancels). Keys already
  used by another action are refused. `Backspace` restores the default.

## Configuration file

`~/.config/sprcmtrd/config.toml` (Linux; the platform config directory
elsewhere, or the path in `SPRCMTRD_CONFIG`). It is created by the
configuration overlay when missing and can also be edited by hand:

```toml
theme = "dark"                 # dark | light | auto

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

## Logging

CLI commands log warnings to stderr. The TUI never logs to the screen: with
`-v` (or `RUST_LOG`) it writes to `~/.local/state/sprcmtrd/sprcmtrd.log`
(the platform state or cache directory elsewhere).
