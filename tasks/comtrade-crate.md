# `comtrade` as its own crate in a workspace

- **Issue:** none, from `todo.md` ("Make `comtrade` its own crate in a
  workspace (`sprcmtrd-comtrade`)", under Refactoring / Library)
- **Status:** planned, waiting for approval

## Goal

The COMTRADE library (`src/comtrade/` and `src/error.rs`) moves into its own
crate `sprcmtrd-comtrade`. The project becomes a Cargo workspace. The spec
already calls the library "a plain, synchronous library [that] could be
published as its own crate later". A crate boundary turns the layering rule
"`comtrade` knows nothing about the layers above it" from a text check in
`tests/layering.rs` into a compile error. It is also the first step of stage 1
in [the desktop GUI plan](gui-tauri-leptos.md): the GUI backend and a later
web UI use the same crate.

There is no behaviour change. Commands, the TUI, output and exit codes stay as
they are.

## What exists

- `src/comtrade/`: `mod.rs` (`Comtrade`, channels, `Side` and so on, 710
  lines), `cfg`, `dat`, `cff`, `check`, `discover`, `export`, `write`; about
  3 200 lines with their unit tests. All submodules are `pub`, and the main
  types are re-exported in `mod.rs`.
- The library's only `crate::` uses are `crate::error::{Error, Result}`
  (5 files) and `crate::comtrade::cff` (one test). Its dependencies are
  `chrono`, `memmap2`, `serde` (`Serialize` for `--json`) and `thiserror`.
- `src/error.rs` holds the library's `Error` and `Result`. Only `comtrade`
  uses it. `lib.rs` re-exports `Error`, and `main.rs` maps
  `Error::is_file_error()` to exit code 3 or 4.
- There are no `pub(crate)` items and no test-only helpers that other modules
  use. The TUI and `analysis` tests build recordings with the public
  `Comtrade::from_readers`, so nothing breaks at the crate boundary.
- 45 files outside the library use `crate::comtrade::…`. Outside users reach
  into submodules for `check` (2), `discover` (4) and `export` (1).
- Four unit tests open sample files with
  `concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/…")`: `mod.rs`,
  `check.rs`, `export.rs` and `write.rs`.
- CI (`.gitlab-ci.yml`, `.github/workflows/ci.yml`) and `agent.md` run
  `cargo fmt --check`, `clippy`, `test`, and `+1.88 check` at the root.
  `cargo deb` and `ci/build-release.sh` build `target/<target>/release/sprcmtrd`.
- The library changes on the branches `cli-refactoring` (`Comtrade::name`,
  `absolute_time`, `RecordingPath::files` / `contains`) and
  `tui-list-scroll-position`. Neither is on `main` yet.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Workspace root | The root stays the `sprcmtrd` package (CLI and TUI) and also becomes the workspace root. The crate goes in `crates/comtrade/`. `tests/`, `packaging/`, `cargo deb` and `target/release/sprcmtrd` stay where they are (as in the GUI plan) | A virtual root with everything in `crates/`: this moves the binary, its tests and the packaging, and touches every CI and release script |
| What moves | Only `comtrade/` and `error.rs`, into `sprcmtrd-comtrade`. `analysis/` and `format/` (`sprcmtrd-core` in the GUI plan) move when the GUI starts | Both crates now: more work without a user yet |
| Library name | Package `sprcmtrd-comtrade`, Rust name `sprcmtrd_comtrade` (the default) | `[lib] name = "comtrade"`: shorter `use` lines, but it reads as the unrelated `comtrade` crate on crates.io |
| Paths in the binary | `lib.rs` re-exports the crate under the old names: `pub use sprcmtrd_comtrade as comtrade;` and `pub use sprcmtrd_comtrade::error;`. `crate::comtrade::…` and `crate::error` keep working, so `command/`, `tui/`, `analysis/` and `format/` don't change | Rewrite the 45 files to `sprcmtrd_comtrade::…`: a large diff with no functional change. It can follow later, file by file |
| Inside the crate | `mod.rs` becomes `lib.rs`. `error.rs` becomes a module of the crate, so `crate::error::…` stays and only `crate::comtrade::cff` changes to `crate::cff`. The public API stays the same (`pub mod cfg` etc. plus the re-exports) | Flatten or rename modules while moving: that isn't needed and hides the move in the history |
| Moving | `git mv`, one commit with only the moves and the manifests, so `git log --follow` and blame keep the history | Copy and delete |
| Versions | `[workspace.package]` with one shared version (0.8.0), edition, `rust-version` 1.88, license, authors and repository; both packages inherit them. `[workspace.dependencies]` for the shared `chrono`, `serde` and `thiserror` | Independent versions per crate: only worth it once the library is published and its API changes on its own schedule |
| `serde` | A plain dependency, as today (`Serialize` for `--json`) | An optional `serde` feature: the usual choice for a published library, but there is no user without it yet |
| Default build | `default-members = [".", "crates/comtrade"]`, so `cargo build`, `test`, `clippy` and `+1.88 check` at the root cover both. Only `cargo fmt` needs `--all`, in CI and `agent.md` | `--workspace` in every command and CI job |
| Sample files | Stay in `tests/data/`. The four library tests use `../../tests/data`, built from `CARGO_MANIFEST_DIR`. The crate's `include` doesn't ship them (`cargo publish` doesn't run the tests) | Copy samples into `crates/comtrade/tests/data`: duplicate files that get out of sync |
| Layering test | `tests/layering.rs` keeps the rules for `src/`, without the `comtrade` row (Cargo enforces it now), and adds a check: the crate's `[dependencies]` are only `chrono`, `memmap2`, `serde` and `thiserror`. A new dependency such as `ratatui` or `tokio` then needs a deliberate change to the test | Rely on Cargo alone: the crate can't use the binary, but it could still gain a UI or async dependency unnoticed |
| Publishing | Not published in this task. `cargo publish --dry-run -p sprcmtrd-comtrade` must pass. The binary depends on it as `{ path = "crates/comtrade", version = "0.8.0" }`, so a later crates.io release publishes the library first, then `sprcmtrd` (written in `agent.md` and `docs/src/development.md`) | `publish = false` on the library: simpler, but `sprcmtrd` then can't be published to crates.io at all |
| Base branch | Start after `cli-refactoring` and `tui-list-scroll-position` are merged into `main`, and branch from `main`. Moving files while other branches change them causes merge conflicts | Branch from `tui-list-scroll-position` now: it works, but it ties this task to two unmerged branches |

## Layout

```
Cargo.toml              [workspace] + package sprcmtrd (CLI and TUI), as today
Cargo.lock              one for the workspace
src/                    lib.rs re-exports comtrade and error; everything else unchanged
tests/                  cli.rs, layering.rs, data/
crates/
  comtrade/
    Cargo.toml          sprcmtrd-comtrade
    README.md           what it reads and writes, a short example
    src/
      lib.rs            was src/comtrade/mod.rs, plus crate docs
      error.rs          was src/error.rs
      cff.rs cfg.rs check.rs dat.rs discover.rs export.rs write.rs
```

Dependencies: `sprcmtrd-comtrade` ← `sprcmtrd`.

## Implementation plan

1. **Move** (one commit, history kept): `git mv src/comtrade/mod.rs
   crates/comtrade/src/lib.rs`, the other modules next to it, and `git mv
   src/error.rs crates/comtrade/src/error.rs`. Add `crates/comtrade/Cargo.toml`
   and the `[workspace]` sections in the root manifest. In the crate, add
   `pub mod error;` to `lib.rs` and change `crate::comtrade::cff` to
   `crate::cff`. In the binary's `lib.rs`, replace `pub mod comtrade;` and
   `pub mod error;` with the re-exports. Fix the doc links that name
   `crate::comtrade`.
2. **Tests:** change the four `CARGO_MANIFEST_DIR` paths to
   `../../tests/data`. Update `tests/layering.rs`: drop the `comtrade` row,
   add the dependency check for `crates/comtrade/Cargo.toml` (read with
   `toml`), and add a doc comment that Cargo now enforces the library's rule.
   All existing tests pass unchanged. There are no new behaviour tests,
   because nothing behaves differently.
3. **Crate docs:** crate-level docs in `lib.rs` (what it reads: revisions
   1991, 1999, 2013 and 2023; ASCII, BINARY, BINARY32, FLOAT32; CFF; what it
   writes; that it is synchronous and has no I/O beyond the files it is given)
   and a short `README.md` with `Comtrade::open` and a channel loop. The
   `include`, `description`, `keywords` and `categories` fields for the
   crate's own manifest.
4. **CI and release:** use `cargo fmt --all --check` in `.gitlab-ci.yml` and
   `.github/workflows/ci.yml`. Check that `ci/build-release.sh`,
   `.github/workflows/release.yml` and `cargo deb` still find the binary and
   the assets (`cargo deb --no-build` locally after a release build).
5. **Docs:**
   - `spec.md`: crate name table, project structure, layering rule ("Cargo
     enforces it"), and the "Comtrade module" heading now names the crate.
   - `agent.md`: architecture, commands (`fmt --all`) and the crates.io order.
   - `docs/src/development.md`: workspace layout and publishing order.
   - `README.md`: development section.
   - `todo.md`: tick the item.
   - [The GUI plan](gui-tauri-leptos.md): stage 1 says the `comtrade` part is
     done by this task and keeps only `sprcmtrd-core`. Its stale 0.6.0 becomes
     the shared version.
6. **Checks:**
   - The four commands from `agent.md` (`cargo fmt --all --check`, and
     `cargo test` with `--no-fail-fast`), `mdbook build docs`.
   - `cargo publish --dry-run --locked -p sprcmtrd-comtrade`.
   - `cargo doc --no-deps -p sprcmtrd-comtrade` without warnings.
   - Same output before and after: `info`, `list`, `check`, `fault`, `trend`
     and `info --json` on `tests/data`, saved and compared with `diff`.
   - A start of the TUI in `tmux`.
