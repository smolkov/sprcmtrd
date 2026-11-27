# Development

```sh
cargo test            # unit, TUI render and CLI integration tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check     # rustfmt.toml: max_width = 120
```

GitHub Actions run these on every push to `main` and every pull request
(`.github/workflows/ci.yml`): format and clippy, tests on Linux, macOS and
Windows, and a build with the minimum Rust version.

### Releasing

1. Set the new version in `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`
3. Approve the deployment: the `Publish release` job of the run waits in
   GitHub Actions ("Review deployments", environment `release`).

`.github/workflows/release.yml` checks that the tag matches the `Cargo.toml`
version, runs the tests, builds the binaries for all platforms and publishes a
GitHub release with the archives (binary, README, licenses), Debian packages
for the Linux targets, `SHA256SUMS`, build provenance attestations and
generated release notes.

Supply chain of a release:

- A pushed tag alone can't publish: the environment `release` needs your
  approval, and accepts only `v*` tags. A ruleset keeps published `v*` tags
  from being moved or deleted; `main` can't be force-pushed or deleted.
- Release builds use no CI caches, and tools are fixed versions: actions by
  commit SHA, `cargo-deb` 3.8.0, GitLab images by digest.
- Dependabot proposes dependency and action updates weekly, after a 7-day
  cooldown. `.github/workflows/deny.yml` runs `cargo deny` (advisories, bans,
  sources; settings in `deny.toml`) on dependency changes and weekly.

Packaging settings live in `Cargo.toml`: the release profile (LTO, one codegen
unit, stripped symbols), the crate metadata for crates.io (`include` keeps CI,
sandbox, notes and your own recordings in `tests/data` out of the package: it
lists the sample recordings by name, so add a new sample there; check with
`cargo package --list`), and
`[package.metadata.deb]` for `cargo deb` (binary, README, licenses and the
completion stubs from `packaging/completions/`). To publish on crates.io:
`cargo publish --dry-run`, then `cargo publish` (needs a crates.io token).

### GitLab

`.gitlab-ci.yml` runs the same checks on Linux Docker runners for merge
requests and the default branch: rustfmt, clippy, tests and the minimum Rust
version. For a version tag pushed to GitLab (`git push gitlab v0.2.0`) it also
checks the tag against `Cargo.toml` and builds release binaries with
`ci/build-release.sh`. `ci/gitlab-release.sh` uploads the binaries and
`SHA256SUMS` to the project's generic package registry and creates a GitLab
release linking to them. `v*` tags are protected: only Maintainers can create
them.

| Platform        | GitHub                    | GitLab                           |
|-----------------|---------------------------|----------------------------------|
| Linux x86_64    | tests + binary            | tests + binary                   |
| Linux aarch64   | binary                    | binary (cross-compiled)          |
| Windows x86_64  | tests + binary (MSVC)     | binary (MinGW, cross-compiled)   |
| macOS           | tests + binaries          | — (needs a macOS runner)         |

The GitLab release needs the package registry enabled for the project
(Settings → General → Visibility → Package registry).

| Path                       | Content                                                              |
|----------------------------|----------------------------------------------------------------------|
| `src/comtrade/`            | COMTRADE library: reading, writing, export, validation, discovery    |
| `src/analysis/`            | Signal analysis: statistics, events, phasors, symmetrical components |
| `src/command/`             | The commands (`info`, `list`, `view`, `stats`, …)                    |
| `src/tui/`                 | TUI: state (`app.rs`), keys, file picker, recording view, widgets    |
| `src/format/`, `src/term/` | Shared by commands and TUI: text formatting, terminal output         |
| `tests/data/`              | Synthetic sample recordings (all revisions and formats)              |
| `spec.md`                  | Specification                                                        |

The `comtrade` module does not depend on the command or TUI code and could be
used as a library on its own.

The recordings in `tests/data/` are synthetic and generated for testing; they
are not real disturbance records.

## Code layout

| Path                       | Content                                                              |
|----------------------------|----------------------------------------------------------------------|
| `src/comtrade/`            | COMTRADE library: reading, writing, export, validation, discovery    |
| `src/analysis/`            | Signal analysis: statistics, events, phasors, symmetrical components |
| `src/command/`             | The commands (`info`, `list`, `view`, `stats`, …)                    |
| `src/tui/`                 | TUI: state (`app.rs`), keys, file picker, recording view, widgets    |
| `src/format/`, `src/term/` | Shared by commands and TUI: text formatting, terminal output         |
| `tests/data/`              | Synthetic sample recordings (all revisions and formats)              |
| `spec.md`                  | Specification                                                        |

The commands (`src/command/`) and the viewer (`src/tui/`) never use each other;
both build on the shared library modules (`comtrade`, `analysis`, `format`,
`term`, `context`). `tests/layering.rs` checks this.

The specification is `spec.md`, guidance for coding agents `agent.md` in the
repository root. `sbx/` holds a podman sandbox for running Claude Code on the
project (`sbx/README.md`).

## Releasing

1. Set the new version in `Cargo.toml` and commit.
2. Tag and push: `git tag v0.2.0 && git push origin v0.2.0`
3. Approve the deployment: the `Publish release` job of the run waits in
   GitHub Actions ("Review deployments", environment `release`).

`.github/workflows/release.yml` checks that the tag matches the `Cargo.toml`
version, runs the tests, builds the binaries for all platforms and publishes a
GitHub release with the archives (binary, README, licenses), Debian packages
for the Linux targets, `SHA256SUMS`, build provenance attestations and
generated release notes.

Supply chain of a release:

- A pushed tag alone can't publish: the environment `release` needs your
  approval, and accepts only `v*` tags. A ruleset keeps published `v*` tags
  from being moved or deleted; `main` can't be force-pushed or deleted.
- Release builds use no CI caches, and tools are fixed versions: actions by
  commit SHA, `cargo-deb` 3.8.0, GitLab images by digest.
- Dependabot proposes dependency and action updates weekly, after a 7-day
  cooldown. `.github/workflows/deny.yml` runs `cargo deny` (advisories, bans,
  sources; settings in `deny.toml`) on dependency changes and weekly.

Packaging settings live in `Cargo.toml`: the release profile (LTO, one codegen
unit, stripped symbols), the crate metadata for crates.io (`include` keeps CI,
sandbox, notes and your own recordings in `tests/data` out of the package: it
lists the sample recordings by name, so add a new sample there; check with
`cargo package --list`), and
`[package.metadata.deb]` for `cargo deb` (binary, README, licenses and the
completion stubs from `packaging/completions/`). To publish on crates.io:
`cargo publish --dry-run`, then `cargo publish` (needs a crates.io token).

## GitLab

`.gitlab-ci.yml` runs the same checks on Linux Docker runners for merge
requests and the default branch: rustfmt, clippy, tests and the minimum Rust
version. For a version tag pushed to GitLab (`git push gitlab v0.2.0`) it also
checks the tag against `Cargo.toml` and builds release binaries with
`ci/build-release.sh`. `ci/gitlab-release.sh` uploads the binaries and
`SHA256SUMS` to the project's generic package registry and creates a GitLab
release linking to them. `v*` tags are protected: only Maintainers can create
them.

| Platform        | GitHub                    | GitLab                           |
|-----------------|---------------------------|----------------------------------|
| Linux x86_64    | tests + binary            | tests + binary                   |
| Linux aarch64   | binary                    | binary (cross-compiled)          |
| Windows x86_64  | tests + binary (MSVC)     | binary (MinGW, cross-compiled)   |
| macOS           | tests + binaries          | — (needs a macOS runner)         |

The GitLab release needs the package registry enabled for the project
(Settings → General → Visibility → Package registry).

| Path                       | Content                                                              |
|----------------------------|----------------------------------------------------------------------|
| `src/comtrade/`            | COMTRADE library: reading, writing, export, validation, discovery    |
| `src/analysis/`            | Signal analysis: statistics, events, phasors, symmetrical components |
| `src/command/`             | The commands (`info`, `list`, `view`, `stats`, …)                    |
| `src/tui/`                 | TUI: state (`app.rs`), keys, file picker, recording view, widgets    |
| `src/format/`, `src/term/` | Shared by commands and TUI: text formatting, terminal output         |
| `tests/data/`              | Synthetic sample recordings (all revisions and formats)              |
| `spec.md`                  | Specification                                                        |

The `comtrade` module does not depend on the command or TUI code and could be
used as a library on its own.

The recordings in `tests/data/` are synthetic and generated for testing; they
are not real disturbance records.

## Documentation

This book lives in `docs/` (mdBook). Build and view it with:

```sh
mdbook serve docs --open
```

`.github/workflows/docs.yml` builds it on every push to `main` that changes
`docs/` and publishes it at <https://smolkov.github.io/sprcmtrd/>; pull
requests only build it. The repository needs Settings → Pages → Source:
"GitHub Actions".
