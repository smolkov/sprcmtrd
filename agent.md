# Agent information file

Guidance for coding agents working on `sprcmtrd`, a terminal viewer for
COMTRADE (IEEE C37.111) recordings: CLI commands plus a ratatui TUI.

- `spec.md` is the source of truth for behaviour. The user edits it directly;
  re-read it before starting a task ("reload spec"), implement what it says, and
  update it in the same change when behaviour changes or a decision is made.
- `README.md` and the book in `docs/` (mdBook, `docs/src/`) are the user
  documentation. Update both when commands, options, keys, configuration or the
  release process change: a new command gets a page in `docs/src/commands/`
  and a line in `docs/src/SUMMARY.md`. Check with `mdbook build docs`.

## Workflow

- Plan first: before changing files for a task, write a plan (what changes,
  in which files, which decisions the spec leaves open, how it is tested) and
  ask the user to approve it. Build only after the approval, and only what
  the plan says; a change of the plan needs a new approval. Questions and
  read-only investigation need no plan.
- Update `readme.md` when commands, options, keys, configuration or the release
  process change.
- Update `todo.md` when tasks are identified or completed.

## Commands

Run all of these before declaring work done. CI runs exactly these:

```sh
cargo fmt --check                                   # rustfmt.toml: max_width = 120
cargo clippy --locked --all-targets -- -D warnings  # no warnings allowed
cargo test --locked                                 # unit, TUI render, CLI tests
cargo +1.88 check --locked --all-targets            # minimum supported Rust
```

- Run `cargo fmt` after edits; CI fails on unformatted code.
- `--locked`: a dependency or version change must update `Cargo.lock` in the same
  commit (`cargo check` without `--locked` updates it).
- MSRV is 1.88 (`rust-version` in `Cargo.toml`; let-chains need it). New
  dependencies must build with 1.88; `cargo add` picks compatible versions.
- To reproduce CI failures that pass locally, run the tests in a clean
  environment: `env -i HOME=$HOME PATH=$HOME/.cargo/bin:/usr/bin:/bin CI=true cargo test --locked`.

## Architecture

```
src/comtrade/   COMTRADE library: cfg.rs, dat.rs, cff.rs, check.rs, discover.rs, export.rs, write.rs, mod.rs
src/analysis/   signal analysis: phasor.rs (Window, DFT), sequence.rs, complex.rs, stats.rs, events.rs, trend.rs, harmonics.rs, impedance.rs
src/command/    info, list, check, view, events, stats, phasors, trend, harmonics, impedance, export, cut, convert, completions + shared formatting (si, time, table)
src/tui/        app.rs (state, key and action dispatch), keys.rs (Action, BINDINGS, KeyMap),
                picker.rs (file picker, directory scan), recording.rs (channels, zoom, cursor),
                event.rs, theme.rs, widgets/
src/args.rs     clap definitions      src/config.rs   config.toml load/save
src/context.rs  Context (global options, config) for commands and TUI
src/format/     shared text formatting: si, time, rate, bytes, table, key_values, summary
src/complete.rs dynamic shell completion of recordings (COMPLETE=<shell> sprcmtrd)
src/term/       terminal output: Spinner, LiveTable, Palette colors (only on a terminal, never color
                alone), ThemeKind (light / dark), fit, terminal_width
src/error.rs    comtrade::Error       src/logging.rs  tracing setup
tests/cli.rs    CLI integration tests  tests/data/     sample recordings
```

- **Layering:** `comtrade` is a plain synchronous library and must not depend on
  `analysis`, `command` or `tui`. `analysis` builds on `comtrade` only: put
  signal analysis there (with tests) and let commands only format results.
  `command` (CLI) and `tui` never use each other; shared code goes into
  `format`, `term`, `context` or `analysis`, and `main.rs` picks the side.
  `tests/layering.rs` fails when a layer uses a module it must not.
- **Async:** only the TUI event loop uses Tokio. Blocking work (loading a `.dat`,
  directory scans) runs in `spawn_blocking` and reports back through
  `event::Message`.
- **Commands:** each subcommand has an `XxxArgs` struct in `args.rs`
  (`Command::Stats(StatsArgs)`) and its behaviour in `command/xxx.rs` as
  `impl StatsArgs { pub fn run(self, ctx: &Context) }`; `Command::run` only
  dispatches. A new command adds a variant, its `Args` struct and the module.
- **Key bindings:** one table, `BINDINGS` in `tui/keys.rs`. Help overlay, footer
  hints, configuration overlay and `config.toml` names are generated from it.
  Add new actions there; never hard-code key hints elsewhere.
- **Errors and exit codes:** library errors are `comtrade::Error`; commands use
  `anyhow`. `main.rs` maps file errors (including I/O `NotFound`) to exit 3,
  invalid content to 4, everything else to 1; clap uses 2.
- **Large files:** `.dat` is memory mapped; plots use min/max decimation
  (`AnalogChannel::plot_segments`, `BinaryChannel::strip`). Keep rendering
  O(columns) per channel, not O(samples).

## Code style

- Match the surrounding code: naming, comment density, idioms. Doc comments on
  all public items; comments explain *why*, not what.
- Keep functions reasonably short; prefer small helpers over long match arms.
- No `unwrap()` on user input or I/O outside tests; `expect("reason")` only for
  real invariants.
- Theme colors come from `tui/theme.rs` roles (16 ANSI colors so the terminal
  scheme applies). The only 256-color exception is `plot_shades`. Everything
  must stay readable with `--no-color` (`Theme::plain`, modifiers only).

## Testing

- Add tests with every change: unit tests next to the code, TUI rendering with
  ratatui `TestBackend` (see `tui/widgets/mod.rs` tests), CLI behaviour and exit
  codes in `tests/cli.rs` with `assert_cmd`.
- `tests/data/` holds small **synthetic** recordings (1991 ASCII, 1999 BINARY,
  1999 ASCII 4 s at 1 kHz, 2013 BINARY32, 2013 FLOAT32 `.cff`, a broken `.cfg`). They are binary for git
  (`.gitattributes`); never let tools rewrite line endings. Adding a file there
  changes `list` output, so update `list_shows_recordings_and_errors`.
- Tests must never touch the user's real files: TUI test apps use
  `config_path: None` or a temp dir, CLI tests set `SPRCMTRD_CONFIG` to a
  non-existent or temp path.
- Tests run on Linux, macOS and Windows in CI: build expected paths with
  `Path::join(...).display()`, never hard-code `/` in displayed paths.
- For TUI changes, also check a real terminal: run the binary in `tmux`
  (`tmux new-session -d -s t -x 110 -y 30 "./target/debug/sprcmtrd tests/data"`,
  `tmux send-keys`, `tmux capture-pane -p`; `-e` shows colors). Send `Esc` and the
  next key in separate `send-keys` calls, otherwise they arrive as `Alt+key`.

## Pitfalls

- **Terminal keys:** many combinations are not distinguishable in the legacy key
  encoding (`Ctrl+Shift+P`, `Ctrl+,`). The TUI enables the kitty keyboard
  protocol when supported; every such binding needs a plain fallback key.
  Character keys match ignoring Shift (`T`, `?` arrive with SHIFT set).
- **textplots** (`view`) panics for width < 32 or height < 3 braille pixels.
- **Cursor** always sits on a sample time (`Recording::set_cursor`); the time
  axis shows it with as many ms decimals as the sample period needs.
- **Files change on disk:** the user's editor may reformat or edit files while
  you work. Re-read a file before editing it, and run `git diff` before every
  commit to make sure only intended changes are included.

## Sandbox

`sbx/start.sh` runs Claude Code in a podman container (spec: `sbx/sandbox.md`,
usage: `sbx/README.md`). Inside it the agent has no permission prompts; only
the repository is mounted, git pushes over HTTPS with the tokens from
`sbx/.env`, and force pushes and deletions are denied in `sbx/settings.json`.
Never commit `sbx/.env`. Changes to `sbx/` should keep the image small: the
host's podman uses `vfs` storage on a nearly full disk.

## Git workflow

- Commit or push only when the user asks. Never amend or force-push.
- Before committing, show/inspect `git status` and `git diff`. Commit the user's
  own unrelated edits (e.g. to `spec.md` or the header banner) as a separate
  commit and say so.
- Commit messages: short imperative subject, body with the why and a bullet
  list for larger changes.
- Remotes: `origin` (GitHub, public) and `gitlab` (company GitLab, `main`
  tracks `gitlab/main`). "Push" means both unless the user says otherwise:
  `git push origin main && git push gitlab main`.
- After pushing to GitHub, check the CI run (public API, no token needed):
  `curl -s "https://api.github.com/repos/smolkov/sprcmtrd/actions/runs?per_page=1"`.
  The GitLab API needs a login; ask the user for pipeline logs.

## Releases

1. Bump `version` in `Cargo.toml`, run `cargo check` to update `Cargo.lock`,
   run all checks, commit as `Release X.Y.Z`.
2. Push `main` to both remotes, then an annotated tag to both:
   `git tag -a vX.Y.Z -m "sprcmtrd X.Y.Z" && git push origin vX.Y.Z && git push gitlab vX.Y.Z`.
3. GitHub (`.github/workflows/release.yml`) builds Linux x86_64/aarch64, macOS
   x86_64/aarch64 and Windows MSVC (archives with binary, README, licenses),
   Debian packages for both Linux targets (`cargo deb`), and publishes a release
   with `SHA256SUMS`. GitLab (`.gitlab-ci.yml`, `ci/*.sh`) builds Linux
   x86_64/aarch64 (with `.deb`) and Windows MinGW on Linux runners and creates a
   release from the package registry.
4. Verify: download an archive, `sha256sum --check --ignore-missing SHA256SUMS`,
   run `--version`; install the `.deb` in a clean container
   (`podman run --rm -v ./dist:/deb:ro debian:trixie-slim …`).

Packaging settings are in `Cargo.toml` (license `MIT OR Apache-2.0`, metadata,
`include`, release profile, `[package.metadata.deb]`); completion stubs for the
`.deb` are in `packaging/completions/`. Before publishing to crates.io run
`cargo publish --dry-run --locked`; it also reports yanked dependencies.

The tag must match `Cargo.toml` or both release pipelines stop. Test changes to
`ci/*.sh` locally in the CI images with podman before pushing a tag.
