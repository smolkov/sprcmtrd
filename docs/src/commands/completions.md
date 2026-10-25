# `completions`: Shell completion script

Print the tab-completion script for your shell. On Tab the shell asks sprcmtrd, which completes subcommands, options and recordings (.cfg / .cff) in the typed directory and its subdirectories. Recommended: load the script on shell start, so it always matches the installed version: bash ~/.bashrc: source <(COMPLETE=bash sprcmtrd) zsh ~/.zshrc (after compinit): source <(COMPLETE=zsh sprcmtrd) fish ~/.config/fish/config.fish: COMPLETE=fish sprcmtrd | source Or write it to a file (again after each update): sprcmtrd completions bash --output ~/.local/share/bash-completion/completions/sprcmtrd

```
sprcmtrd completions [OPTIONS] <SHELL>
```

Tab completion for **bash, zsh, fish, PowerShell** and elvish. On Tab the
shell asks `sprcmtrd`, which completes subcommands, options and option values,
and for the recording argument of `info`, `view`, `events`, `stats`, `export`,
`cut`, `convert` and the viewer: **COMTRADE recordings** (`.cfg` / `.cff`) in
the typed directory **and its subdirectories**, plus the subdirectories to step
into.

```
$ sprcmtrd view sta<Tab><Tab>
station_a/                     station_a/2024/fault_1999.cfg
station_b/                     station_b/combined_2013.cff
```

Load it on shell start (recommended, it then always matches the installed
version):

```sh
# bash: ~/.bashrc
source <(COMPLETE=bash sprcmtrd)
# zsh: ~/.zshrc, after compinit
source <(COMPLETE=zsh sprcmtrd)
# fish: ~/.config/fish/config.fish
COMPLETE=fish sprcmtrd | source
# PowerShell: $PROFILE
$env:COMPLETE = "powershell"; sprcmtrd | Out-String | Invoke-Expression; Remove-Item Env:\COMPLETE
```

Or write the script to a file with `sprcmtrd completions SHELL --output FILE`
(for example `~/.local/share/bash-completion/completions/sprcmtrd`); write it
again after updating `sprcmtrd`. Hidden directories are skipped (type `.` to
see them), and a Tab offers at most 500 recordings, so it stays quick in large
trees.

## Arguments and options

| Argument / option | Meaning |
|---|---|
| `<SHELL>` | Shell to generate the script for [possible values: bash, elvish, fish, powershell, zsh] |
| `-o, --output <OUTPUT>` | Write the script to this file instead of stdout |

The [global options](index.md#global-options) apply too. `sprcmtrd completions --help`
shows this list with examples.
