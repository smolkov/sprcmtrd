# Desktop GUI with Tauri and Leptos, workspace with shared crates

- **Issue:** none, from `todo.md` ("Tauri ui ideas", and "Make `comtrade` its
  own crate in a workspace" under Refactoring / Library)
- **Status:** plan, waiting for approval

## Goal

A desktop viewer for COMTRADE recordings next to the CLI and the TUI: the same
recordings, channels, plots and analysis, with a mouse, resizable windows and
real graphics. It's built with Tauri 2 (native window, Rust backend) and
Leptos (Rust UI compiled to WebAssembly), so all code stays Rust.

To share code, the project becomes a Cargo workspace: the COMTRADE library and
the signal analysis become their own crates, used by the CLI/TUI binary and by
the GUI backend. Crate boundaries also enforce the layering that
`tests/layering.rs` checks by text today.

The work has four stages. Each stage ends with all checks green and gets its
own commit(s), so it can be stopped after any stage. Stage 1 is useful on its
own and changes no behaviour.

## Findings that shape the plan

- `comtrade` and `analysis` are clean already: they use only `chrono`,
  `serde`, `memmap2`, `thiserror` and `crate::error`. Nothing in them knows
  about clap, ratatui or tokio, so they move without code changes.
- `format` uses only `comtrade` (the summary needs `Config`), so it can move
  too and the GUI can show the same numbers (`si`, `time`) as the CLI.
- `args`, `config`, `context`, `term`, `complete`, `logging` belong to the
  CLI/TUI binary.
- **The frontend can't use `comtrade`:** Leptos runs as `wasm32` in the web
  view, and `memmap2` and file access don't exist there. So the frontend talks
  to the Tauri backend through commands (`invoke`), and the data types they
  exchange need a small crate of their own that builds for both targets.
- **MSRV:** Tauri 2.12 needs Rust 1.90, while the project's MSRV is 1.88.
  Leptos 0.8.23 needs 1.88 and trunk 0.21 needs 1.81.
- **Local setup:** the installed `cargo-tauri` is 1.0.3 (Tauri 1), `trunk`
  isn't installed, and `libwebkit2gtk-4.1-dev` is missing; the
  `wasm32-unknown-unknown` target is installed. The GUI build needs WebKitGTK
  and GTK dev packages on Linux, which is a lot for the podman sandbox on the
  nearly full disk (`agent.md`, Sandbox).
- `Comtrade::open` copies all samples into vectors (`todo.md`, Large files).
  That doesn't block the GUI, because the backend holds the recording either
  way, but it's the same limit as in the TUI.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Workspace root | The root stays the `sprcmtrd` package (CLI + TUI) and becomes the workspace root too; new crates go in `crates/`, the GUI in `gui/`. `tests/`, `packaging/`, `cargo deb`, the release scripts and `target/release/sprcmtrd` keep working | A virtual root with everything in `crates/`: tidier, but it moves the binary, its tests and the packaging, and touches every CI and release script |
| Library crates | Two: `sprcmtrd-comtrade` (file format: `comtrade/` + `error.rs`, publishable as the spec suggests) and `sprcmtrd-core` (`analysis/` + `format/`) | One crate for all of it: fewer manifests, but the format library can't be published without the analysis. Or three (format separate): too fine for 1 000 lines |
| Paths in the binary | `lib.rs` re-exports the crates under the old names (`pub use sprcmtrd_comtrade as comtrade;` and so on), so `crate::comtrade::…` stays and the diff in `command/` and `tui/` is close to zero | Replace every path with `sprcmtrd_comtrade::…`: about 150 changed `use` lines without a functional change |
| Versions | `[workspace.package]` with one shared version, edition, license, authors; each crate inherits them. Workspace `[workspace.dependencies]` for shared ones (`chrono`, `serde`, `thiserror`) | Independent versions per crate: only needed once a crate is published |
| GUI MSRV | `rust-version = "1.90"` for the two GUI crates only; the CLI, TUI and libraries stay at 1.88. CI's MSRV job checks the non-GUI packages | Raise the whole project to 1.90 |
| Default build | `default-members` leaves out the GUI, so `cargo build`, `cargo test`, `cargo clippy` and CI behave as today and don't need WebKitGTK. The GUI builds with `cargo tauri dev` / `build` and its own CI job | GUI in every build: every developer and CI runner needs the WebKit packages |
| GUI app | Its own binary `sprcmtrd-gui` with its own package (`.deb`, AppImage, `.msi`, `.dmg` from `cargo tauri build`) | A `sprcmtrd gui` subcommand: one binary, but the CLI `.deb` would depend on WebKitGTK |
| Data flow | The backend holds the open recordings and does all the work, the frontend only gets what it draws: decimated min/max per pixel column (`AnalogChannel::plot_segments`, `BinaryChannel::strip`), as the TUI does. O(columns) per channel, works for large files | Send the samples to the frontend once: simpler drawing, but megabytes per channel over IPC and in wasm memory |
| Plot drawing | Canvas 2D through `web-sys`, our own code like the TUI widgets: axes, min/max bands, binary strips, cursor | SVG elements from Leptos: easy to style, slow with many channels × 2 000 columns. Or a plotting crate (`plotters-canvas`): more dependencies, less control |
| Frontend build | Leptos 0.8 in CSR mode (client side only, Tauri has no server) built by `trunk`, which `tauri.conf.json` runs (`beforeDevCommand: trunk serve`) | Leptos 0.9 (only rc now) |
| Stage 2 first screen | The viewer: open a file (dialog) or a directory (list like `list`), sidebar with channels, analog plots and binary strips on one time axis, wheel zoom, drag pan, cursor with values, primary/secondary, light/dark from the OS | The analysis first (phasors, fault summary): less to draw, but there is no context to see them in |
| CI and release | Stage 3: a GitHub job that builds and tests the GUI on Linux (WebKit packages from apt). Releases of the GUI only after a separate approval (stage 4, GitHub only first; Tauri doesn't cross-build Windows on the GitLab Linux runners) | GUI artefacts in the same release pipelines now |
| Sandbox | Develop the GUI on the host; the sandbox image stays small. The Rust parts (backend commands, API types, frontend logic) still build and test there with `-p` | Add WebKitGTK to the image: hundreds of MB on `vfs` storage |

## Workspace layout after stage 1 and 2

```
Cargo.toml                 [workspace] + package sprcmtrd (CLI + TUI), as today
src/                       args, command, tui, config, context, term, complete, logging, main
tests/                     cli.rs, layering.rs (command vs tui only), data/
crates/
  comtrade/                sprcmtrd-comtrade: cfg, dat, cff, check, discover, export, write, error
  core/                    sprcmtrd-core: analysis/, format/
  api/                     sprcmtrd-api: serde types between GUI backend and frontend (native + wasm32)
gui/
  src-tauri/               sprcmtrd-gui: Tauri 2 backend, commands, tauri.conf.json, icons
  ui/                      sprcmtrd-gui-ui: Leptos 0.8 CSR frontend, index.html, Trunk.toml, style.css
```

Dependencies: `comtrade` ← `core` ← `sprcmtrd`, `sprcmtrd-gui`; `api` ←
`sprcmtrd-gui`, `sprcmtrd-gui-ui`. `api` depends only on `serde` (no
`comtrade`), so it builds for wasm32.

## Implementation plan

### Stage 1: workspace and library crates (no behaviour change)

1. **Workspace manifest:** `[workspace]` with `members = [".", "crates/*"]`,
   `default-members`, `[workspace.package]` (version 0.6.0, edition 2024,
   rust-version 1.88, license, authors, repository) and
   `[workspace.dependencies]`. Release profile stays at the root (it applies
   to the workspace).
2. **`crates/comtrade`:** move `src/comtrade/*` and `src/error.rs`
   (`git mv`, so history follows). `crate::error` becomes `crate::` paths
   inside the crate; its `lib.rs` keeps today's public API (`Comtrade`,
   `Error`, `RecordingPath`, …). Its own `README.md` and crate docs, `include`.
3. **`crates/core`:** move `src/analysis/*` and `src/format/*`; `use
   crate::comtrade::…` becomes `use sprcmtrd_comtrade::…`.
4. **Binary crate:** `src/lib.rs` re-exports `comtrade`, `analysis`, `format`
   and `Error` under the old names; `main.rs` maps exit codes as before
   (`comtrade::Error` unchanged).
5. **Tests:** unit tests move with their code. The ones that open
   `tests/data/…` (`comtrade/check.rs`, `export.rs`, `write.rs`) use a path
   built from `CARGO_MANIFEST_DIR` (`../../tests/data`), so the sample files
   stay in one place. `tests/layering.rs` keeps the command ↔ tui rule; the
   library rules are now enforced by Cargo, and the test says so.
6. **Docs:** `agent.md` (Architecture, Commands: `--workspace` where needed,
   MSRV per package), `spec.md` project structure, `docs/src/development.md`,
   `README.md` development section. `todo.md`: tick the workspace item.
7. **CI/release:** check that `.github/workflows/*.yml`, `.gitlab-ci.yml`,
   `ci/*.sh` and `cargo deb` still find everything; `cargo publish --dry-run`
   for `sprcmtrd-comtrade` (not published without your go).
8. **Checks:** the four CI commands, plus `cargo test --workspace --locked`
   and `mdbook build docs`. Output of all commands must be unchanged: compare
   `info`, `list`, `fault`, `trend` on `tests/data` before and after.

### Stage 2: GUI skeleton and the viewer

1. **Tools (on the host, your approval for installs):**
   `cargo install tauri-cli --version "^2" --locked`, `cargo install trunk
   --locked`, `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev
   librsvg2-dev libayatana-appindicator3-dev`. 
2. **`crates/api`:** `RecordingId`, `Summary` (header, channels, time span,
   sample rates, line frequency), `ListRow`, `PlotRequest` (`id`, time window,
   columns, channel ids, side), `PlotData` (per analog channel: min/max per
   column, y range, unit; per binary channel: strip cells), `CursorValues`.
   Serde only; unit tests for round trips.
3. **`gui/src-tauri`:** Tauri 2 app, `tauri-plugin-dialog` for open dialogs.
   State: `Mutex<HashMap<RecordingId, Comtrade>>`. Commands: `scan_dir`,
   `open_recording` (in `spawn_blocking`), `close_recording`, `plot`,
   `values_at`. Thin: each command converts between `core` / `comtrade` and
   `api`; tests call the command functions directly without a window.
4. **`gui/ui`:** Leptos app with a file / directory picker, channel sidebar
   (visibility, as in the TUI), plot area with canvas per channel, a shared
   time axis, wheel zoom around the mouse, drag to pan, click for the cursor
   and a value readout, primary / secondary switch, light / dark from
   `prefers-color-scheme`. A redraw asks the backend for the new window at
   the canvas width in device pixels; requests are debounced while zooming.
5. **Docs:** `spec.md` section "Desktop GUI" with these decisions,
   `docs/src/gui/index.md` + `SUMMARY.md`, README (install / run),
   `agent.md` (GUI layout, `cargo tauri dev`, the frontend-can't-use-comtrade
   rule).
6. **Checks:** the CI commands (unchanged scope), plus `cargo clippy -p
   sprcmtrd-gui -p sprcmtrd-gui-ui --target …`, `cargo test -p sprcmtrd-api
   -p sprcmtrd-gui`, `trunk build`, and a manual run of `cargo tauri dev`
   with `tests/data` (open each sample, zoom, pan, cursor).

### Stage 3: analysis panels and CI

- Panels that reuse the command logic from `core`: events timeline, phasors
  at the cursor with a phasor diagram, fault summary (`analysis::fault`),
  RMS view (`analysis::rms`), harmonics at the cursor.
- To share the result types with the CLI, the JSON structs that today live
  in `command/*.rs` move to `core` where the GUI needs them (one per panel,
  when the panel is built), so CLI JSON and GUI show the same numbers.
- GitHub CI job `gui`: Ubuntu, apt packages, `cargo clippy` / `test` for the
  GUI packages, `trunk build`, `cargo tauri build --no-bundle`.

### Stage 4: GUI releases (separate approval)

- Bundles from `cargo tauri build` in `release.yml`: `.deb` and AppImage
  (Linux x86_64), `.msi` (Windows), `.dmg` (macOS), next to the CLI archives
  in the same release. Signing / notarization for macOS and Windows needs
  certificates: decide then.

## Open questions

- **Name:** `sprcmtrd-gui` as the binary and app name, or a product name for
  the window title and installers?
- **Scope of stage 2:** are several recordings open at once (tabs), or one
  at a time like the TUI? The plan does one, with the backend ready for more.
- **Web server UI** (`todo.md`, "Webserver ui ideas"): the same Leptos
  frontend could later run in a browser against an HTTP backend instead of
  Tauri commands, if the backend calls go through one small trait. The plan
  keeps that door open but doesn't build it.
