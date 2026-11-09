# `themes`: Color themes

List the color themes of the viewer, or print one as a template for your own

```
sprcmtrd themes [--print NAME]
```

```
$ sprcmtrd themes
NAME            KIND   SOURCE
dark            dark   built-in
light           light  built-in
atom-one-dark   dark   built-in
nord            dark   built-in
nord-night      dark   built-in
solarized-dark  dark   built-in
tokyonight      dark   built-in
my-nord         dark   /home/me/.config/sprcmtrd/themes/my-nord.toml
```

- The built-in themes come first, then your files in the `themes` directory
  next to the configuration file, by name. A file with a built-in name
  replaces that theme; its row shows the file.
- A theme file that doesn't load is listed as `invalid`, and the error (file
  and position) is printed on stderr.
- `--print NAME` writes the theme as a complete file with every entry, with
  `extends` already applied:

```sh
sprcmtrd themes --print tokyonight > ~/.config/sprcmtrd/themes/mine.toml
sprcmtrd --theme mine
```

The file format is described in [Configuration](../viewer/configuration.md#color-themes).
