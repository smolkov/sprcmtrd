# Security fixes from the review

- **Issue:** none, from a security review of 1.0.0
- **Status:** done, unreleased. Built with the proposed decisions. Differences from the plan:
  - 2: `info` also filters `.hdr` / `.inf` text when printing (newlines and
    tabs kept). It isn't filtered in the parser, because `convert` writes it back.
  - 3: `.git` is also bind-mounted onto itself. With only `hooks` and
    `config` read-only, the agent could rename `.git` and put a new one with
    its own config in its place. `start.sh` refuses to start when `.git` is
    not a plain directory. Tested with podman on a scratch repository: commit
    and branch work; writing hooks or config, and renaming `.git`, `hooks` or
    `config`, fail.
  - 5: the guard is also in `csv_name`, which builds the CSV names of
    `trend` and `compare`.
  - 6: `dtolnay/rust-toolchain` was referenced by branch (`@stable`, `@1.88`).
    It is now pinned to a commit of `master`, with `toolchain:` as an input.

## Problem

A review of 1.0.0 found these issues. Each was confirmed with a crafted file
or a tool run.

1. **Crafted files abort the process.** The parser reserves memory using
   counts from the `.cfg` before it checks them. A file of a few bytes makes
   the allocation fail, which aborts the process (exit 134). The TUI is then
   left in raw mode. Medium values (about 1e9 samples) make the OS actually
   hand out gigabytes. Places:
   - `cfg.rs` `Vec::with_capacity(n_analog)` / `(n_digital)`: the channel
     count line
   - `cfg.rs` `Vec::with_capacity(nrates)`: the number of sample rates
   - `dat.rs` `parse_ascii`: `new_columns(cfg, cfg.total_samples())`, the
     last sample number
2. **Escape sequences from the file reach the terminal.** Station, device,
   channel, phase, circuit and unit names are printed unfiltered.
   `info`/`list` on a file with `ESC ] 0;… BEL` in the station name set the
   window title. Other sequences can clear the screen, forge output or write
   the clipboard (OSC 52).
3. **Sandbox: host code execution through `.git`.** The repository is mounted
   read-write with `.git`. The agent can write `.git/hooks/*` or set
   `core.hooksPath`, `core.fsmonitor` or `include.path` in `.git/config`.
   That code then runs on the host at the next `git` command.
4. **Sandbox: the push deny rules are easy to get around.** They match
   prefixes only. `git push origin +main` and `git -C . push --force` are not
   matched.
5. **CSV export: formula injection.** A channel id starting with `=`, `+`
   or `@` becomes a formula when the CSV is opened in a spreadsheet.
6. **CI: actions pinned by tag** (`actions/checkout@v4`). A moved tag runs
   other code with the job's token.
7. **Dependency:** `chrono` is flagged unmaintained (RUSTSEC-2026-0543). It
   has no known vulnerability.

No change needed: the one `unsafe` (mmap; truncation by another process is
documented); CI triggers and token permissions; no process spawning;
symlinked directories are not followed.

## Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| 1: limit allocations | Reserve no more than the input can hold: channels and rates at most the number of remaining cfg lines, ASCII samples at most `dat` lines. The existing sample-count check still reports a mismatch. | Hard upper limits (e.g. 100 000 channels): simpler, but an arbitrary rule |
| 2: where to filter | In the cfg parser: control characters (C0, DEL, C1) in text fields become `�` (U+FFFD). This covers every output (CLI, TUI, export, `cut`/`convert`) in one place. | Only when printing: keeps the bytes for `cut`/`convert`, but many print sites to cover and easy to miss one |
| 2: tell the user | `check` reports a warning "control characters replaced in …" | No message |
| 3: `.git` in the sandbox | Mount `.git/hooks` and `.git/config` read-only over the repository mount. The agent can still commit, branch and push; `git remote add` and `git branch -u` fail inside. | A host check after each session (`git config --get core.hooksPath`, new hooks): no limits inside, but easy to forget |
| 4: push rules | More patterns (`+` refspecs, `-C`), and `sbx/README.md` states that the rules are not a boundary: protect `main` on GitHub and GitLab | Remove the rules and rely on the server only |
| 5: CSV | Prefix a header cell starting with `=`, `+`, `@`, tab or CR with `'` (OWASP). `-` is left alone, since negative values are data. JSON is unchanged. | Leave CSV unchanged and document it |
| 6: actions | Pin every `uses:` to its full commit SHA with the tag as a comment | Keep tags |
| 7: chrono | Don't migrate now. Add `deny.toml` with the advisory ignored and the reason, and a `todo.md` line for a move to `jiff` | Migrate to `jiff` now (7 files, date parsing and formatting) |

## Implementation plan

1. `comtrade/cfg.rs`, `comtrade/dat.rs`: capped reservations. Unit tests
   with inline cfg text (no new files in `tests/data`): a huge channel count,
   rate count and sample count each give an error or a normal load, never an
   abort.
2. `comtrade/cfg.rs`: a `clean_text` helper applied to all text fields; a
   flag on `Config` for `check`; unit test and CLI test (`info` output
   contains no ESC).
3. `sbx/start.sh`: two `:ro` volumes; `sbx/README.md`, `sbx/sandbox.md`.
4. `sbx/settings.json`: patterns; README note.
5. `comtrade/export.rs`: header escaping; unit test; spec and
   `docs/src/commands/export.md`.
6. `.github/workflows/*.yml`: SHAs from `gh api`.
7. `deny.toml`, `todo.md`.

Checks: `cargo fmt --check`, clippy, `cargo test --locked`,
`cargo +1.88 check`, `mdbook build docs`, and the crafted files from the
review run again.

## Follow-up: shell completion inserts file names unescaped

- **Status:** planned, waiting for approval

### Problem

A file name runs as shell code when you tab-complete it in bash. Reproduced
in an interactive bash 5.2: `sprcmtrd info fa<Tab>` in a directory with
`fault$(touch PWNED).cfg` puts `fault$(touch PWNED).cfg` on the line, and
Enter creates `PWNED`. A recording from a mail, share or archive is enough.

- Cause: clap_complete's bash adapter (`Bash::write_complete`) prints the
  candidates raw. Its script puts them into `COMPREPLY` without
  `-o filenames`, so bash inserts them unquoted. This applies to every
  candidate, our `recordings` completer and clap's own path completion
  (`--output`) alike.
- zsh and fish are not affected: tested, both insert
  `fault\$\(touch\ PWNED\).cfg`.
- PowerShell and elvish: the adapters print raw candidates in the same way,
  and both shells evaluate `$(…)` / `(…)` in a bare word. Not tested (neither
  shell is installed here). The Windows release makes PowerShell relevant.

### Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Where to fix | Own adapters in `complete.rs` that wrap clap's `Bash`, `Powershell` and `Elvish`: same registration, but `write_complete` escapes each candidate. They are passed with `CompleteEnv::shells(..)` in `main.rs` and in `completions`. This covers every candidate, including clap's path completion. | Escape only in our `recordings` completer: smaller, but `--output` path completion stays open |
| Bash escaping | Backslash before shell metacharacters (space, `!"#$&'()*;<>?[\]^\`{|}`, and `~` except at the start, so `~/` still completes) | `complete -o filenames` in the patched script: bash quotes itself, but it also changes how directories and options are completed |
| PowerShell / elvish | Single-quote candidates that contain special characters, doubling inner quotes (PowerShell also treats `‘’‚‛` as quotes) | Drop these two shells from `completions` (only bash/zsh/fish are documented) |
| Control characters | Drop candidates with control characters (newline, `\v`, the IFS) entirely | Escape them |
| Report upstream | Open an issue at clap-rs/clap with the bash reproduction, once this is fixed here | Don't |

### Implementation plan

1. `complete.rs`: `SafeBash`, `SafePowershell`, `SafeElvish` (implementing
   `EnvCompleter`, delegating to clap) and `SHELLS: Shells`. Unit tests for
   the escaping functions.
2. `main.rs` and `command/completions.rs`: use `SHELLS`.
3. CLI test: `COMPLETE=bash` with `_CLAP_COMPLETE_INDEX` set, in a temp dir
   with `fault$(x).cfg`, prints `fault\$\(x\).cfg`. Manual check in
   interactive bash, zsh and fish with tmux, as in the reproduction.
4. Spec (completion section), `docs/src/commands/completions.md`.

## Follow-up: supply chain of the release

- **Status:** in progress (approved 2026-11-27 with the proposed decisions).
  The file changes are merged into `main`. GitHub
  settings applied and read back on 2026-11-27: environment `release`
  (required reviewer smolkov, only `v*` tags), rulesets "Release tags" (no
  update or deletion of `refs/tags/v*`) and "Main branch" (no force-push, no
  deletion), Dependabot security updates on. Open:
  - GitLab protected tags `v*`: the `glab` token here has no `api` scope
    (HTTP 403). **You:** Settings → Repository → Protected tags, `v*`,
    allowed to create: Maintainers. Re-create the sandbox's GitLab token
    with role Developer.
  - Required SHA pinning: only after the merge, because `main` still uses
    tag references until then.
  - **You:** check 2FA on crates.io.

  Differences from the plan:
  - The environment was created before the push, not after it. A workflow
    that names an environment that doesn't exist creates it without
    protection.
  - `cargo deny` is a workflow of its own (`deny.yml`), so the weekly
    schedule doesn't run the whole CI. It uses cargo-deny 0.20.2, not the
    newest 0.22.1, which was 3 days old.
  - Dependabot waits 7 days (`cooldown`) before proposing a new version.
  - The sandbox's rustup download was tested in a container: the right
    checksum passes, a wrong one stops the build.

### Problem

A supply-chain review on 2026-11-27 found no sign of an attack. The
dependencies are in good shape: all 269 locked crates come from crates.io and
are pinned by checksum, none is yanked, every locked version is at least 37
days old and has at least 600k downloads, there are no lookalike names, and
both published `sprcmtrd` versions contain only the sample files. The release
path is the weak part:

1. Release tags are protected neither on GitHub nor on GitLab, and `main` on
   GitHub has no protection. A pushed `v*` tag builds and publishes binaries
   and `.deb` packages from whatever commit it points to. Any holder of a
   write token can do that, including the tokens in `sbx/.env`.
2. Release builds restore CI caches (`rust-cache` in `release.yml`,
   `target/` in the GitLab `build` job). A poisoned cache would end up in
   the release binaries.
3. Some build inputs float: `cargo-deb` (newest version on both CIs), the
   GitLab images `rust:1-bookworm`, `rust:1-bullseye` and `alpine:3`, and in
   the sandbox `debian:trixie-slim` and `curl | sh` for rustup.
4. There is no provenance: `SHA256SUMS` is made in the same job and isn't
   signed.
5. Dependabot is off, and no CI job checks advisories or sources.
6. `include = ["/tests/**"]` would put untracked files from `tests/data`
   into a published crate (`cargo package --list` shows `55Hz.*` and
   `long1_t1_anstatt_t2.*`).

### Decisions to confirm

| Decision | Proposed | Alternative |
|---|---|---|
| Gate for a GitHub release | Environment `release` with you as required reviewer, deployments only from `v*` tags; the `release` job uses it. A tag push still builds, but publishing waits for your click in the browser, which a token can't give. | Tag ruleset without bypass: no one can push `v*` tags, so releases need the ruleset switched off for each release |
| GitHub tags | Ruleset for `refs/tags/v*`: no update or deletion (a published tag can't be moved to other code); creation allowed | Also restrict creation (see above) |
| GitHub `main` | Ruleset: no force-push, no deletion; direct pushes stay allowed | Require pull requests |
| GitLab | Protected tags `v*`, created by Maintainers only. **You:** re-create the sandbox's project access token with role Developer, so it can't create release tags (GitLab protected environments need Premium) | Leave GitLab as is |
| Caches in release builds | None: remove `rust-cache` from `release.yml` and set `cache: []` in the GitLab `build` job. Releases build a few minutes longer. | Keep caches with a separate key per tag |
| Tool versions | `cargo-deb@<current>` in `install-action`, `cargo install cargo-deb --version <current> --locked` on GitLab; GitLab and sandbox base images pinned by digest (tag as a comment); rustup in the sandbox from `rustup-init` with a checked SHA-256 | Pin only the CI tools, leave the sandbox |
| Claude Code installer in the sandbox | Keep `curl https://claude.ai/install.sh`, documented as a trusted source | Pin a version |
| Provenance | `actions/attest-build-provenance` (pinned) in the GitHub `release` job for all files in `dist/`; README section "Verify a download" with `gh attestation verify` | Sign `SHA256SUMS` with a key (key management) |
| Dependency updates | `.github/dependabot.yml` for `cargo` and `github-actions` (weekly, minor and patch updates grouped); turn on Dependabot security updates | Only security updates |
| Dependency check in CI | `cargo deny check advisories bans sources` as a CI job, also weekly by schedule, so a new advisory shows up without a push; `deny.toml` gets `[sources]` limited to crates.io | Only locally |
| Required SHA pinning | Turn on "require actions to be pinned to a full-length commit SHA" after the branch is merged | Leave off |
| Package contents | `include` lists `tests/*.rs` and the 15 sample files by name | Move the samples to `tests/data/samples/` |
| crates.io | Not changed now. **You:** check 2FA on your crates.io account; trusted publishing once publishing runs in CI | Add a publish workflow with trusted publishing now |

### Implementation plan

1. Workflows: `release.yml` (environment, no cache, attestation and its
   permissions `id-token: write`, `attestations: write`, pinned `cargo-deb`);
   `ci.yml` (`cargo deny` job, weekly schedule); `.github/dependabot.yml`.
2. GitLab: `.gitlab-ci.yml` (images by digest, `cache: []` in `build`),
   `ci/build-release.sh` (`cargo-deb` version).
3. Sandbox: `Containerfile` (base image digest, checked `rustup-init`);
   `sbx/README.md`: token role Developer, Claude installer.
4. `Cargo.toml` `include`; `cargo package --list` shows only the samples.
5. `deny.toml` `[sources]`, `[bans]`.
6. Settings, applied with `gh api` / `glab api` after the changes are pushed,
   each read back afterwards: the GitHub environment, the two rulesets,
   Dependabot security updates, and required SHA pinning after the merge;
   the GitLab protected tags.
7. Docs: README "Verify a download" and the release process (in `README.md`
   and `docs/src/development.md`).

Checks: the usual four plus `mdbook build docs`, `cargo deny check`,
`actionlint` if available, and YAML parsing. After the push: the CI run on
the branch, and each setting read back through the API. A test tag can't
check the approval step, because `verify` rejects any tag that doesn't
match the `Cargo.toml` version. The first real check is the next release:
its `release` job must wait for your approval.
