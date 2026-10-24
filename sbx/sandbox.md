# sandbox - specification

Run the Claude Code agent in a rootless podman container, so it can work
without permission prompts (`--dangerously-skip-permissions`) while only the
project directory and a few caches are reachable from inside. The container is
the security boundary, not the agent's permission settings.

## Goals

- The agent can build, test, commit and push this project without asking for
  every command.
- Nothing outside the mounted project directory (home directory, SSH keys,
  other repositories, the host's `~/.claude`) is visible to the agent.
- One command starts it: `sbx/start.sh`.
- Files the agent writes in the project belong to the host user.

## Files

All sandbox files live in `sbx/`:

| File             | Content                                                        |
|------------------|----------------------------------------------------------------|
| `Containerfile`  | Image definition (Debian trixie)                               |
| `start.sh`       | Builds or updates the image and starts the agent (executable)  |
| `README.md`      | Setup, usage, what is mounted, how to reset                    |
| `settings.json`  | Claude Code settings copied into the image                     |
| `entrypoint.sh`  | Runs on every start: settings and git config, then the command |
| `env.example`    | Template for the token file; the real `sbx/.env` is gitignored |
| `.containerignore` | Keeps `.env` out of the image build context                  |

## Container image

- Base: `debian:trixie-slim` (same Debian release as the host, so binaries
  built inside run on the host and the other way round).
- Tools:
  - Rust via rustup: stable with `clippy` and `rustfmt`, plus `1.88` (MSRV
    check from `agent.md`).
  - `git`, `gh` (GitHub CLI), `glab` (GitLab CLI), `curl`, `jq`, `ca-certificates`.
  - `tmux`, `zsh`, `fish` and `python3`, which the agent uses to check the
    TUI and shell completion.
  - Claude Code (native installer), installed for the container user.
- A non-root user `agent` with home `/home/agent`; podman maps it to the host
  user (`--userns=keep-id`), so files created in the project are owned by you.
- The image carries no secrets; tokens are passed at run time.
- Labels record the hash of `Containerfile` and `settings.json`, so
  `start.sh` can tell when the image is out of date.

## `start.sh`

```
sbx/start.sh [--rebuild] [--pull] [shell | claude-args...]
```

- Builds the image (`sprcmtrd-sbx`) when it doesn't exist, or when the hash of
  `Containerfile` / `settings.json` differs from the image label.
- `--rebuild` rebuilds anyway (new Claude Code / Rust versions); `--pull` also
  fetches a newer `debian:trixie-slim`.
- Starts a fresh container for every run (`podman run --rm -it`); state that
  must survive lives in named volumes (below), so updating the image never
  loses it and a broken container is gone after exit.
- Default command: `claude --dangerously-skip-permissions`, extra arguments are
  passed on (`sbx/start.sh --resume`). `shell` opens bash instead.
- Fails early with a clear message when podman is missing or `sbx/.env` lacks
  a Claude credential.
- Works from any directory (finds the repository root from its own path).

## Mounts and volumes

| Host                          | Container                     | Mode       |
|-------------------------------|-------------------------------|------------|
| repository root               | same absolute path            | read-write |
| volume `sprcmtrd-sbx-claude`  | `/home/agent/.claude`         | read-write |
| volume `sprcmtrd-sbx-cargo`   | `/home/agent/.cargo/registry` | read-write |

- The repository is mounted at the same path as on the host, so paths in
  output, `target/` and Claude's per-project memory match the host.
- `target/` is shared with the host (same OS); no separate build directory.
- Nothing else from the host is mounted: no home directory, no `~/.ssh`, no
  host `~/.claude`, no podman/docker socket.
- SELinux hosts: mount with `:Z`.

## Network

- Default: unrestricted outgoing network (rootless podman with `pasta`), so
  the agent reaches the Claude API, GitHub, crates.io, rustup and the GitLab
  server.
- The container uses the host's routes, so the company GitLab works when the
  host is on the VPN.
- No ports are published; nothing listens for the outside.

## Authentication

Tokens are read from `sbx/.env` (gitignored, mode 600) and passed with
`--env-file`; they never enter the image or the repository.

| Variable                   | Used by     | Notes                                              |
|----------------------------|-------------|----------------------------------------------------|
| `CLAUDE_CODE_OAUTH_TOKEN`  | Claude Code | Claude subscription; create with `claude setup-token` |
| `ANTHROPIC_API_KEY`        | Claude Code | Alternative: API key (billed per use)              |
| `GH_TOKEN`                 | `gh`, git   | Fine-grained token, only this repository: contents, pull requests, actions read |
| `GITLAB_TOKEN`             | `glab`, git | Project access token: `api`, `write_repository`     |
| `GITLAB_HOST`              | `glab`, git | `gitlab.fe.sprecher-automation.com`                |

- Git pushes over HTTPS with the tokens (a credential helper reads them from
  the environment). The repository's SSH remotes are rewritten for the
  container only:
  `url."https://github.com/".insteadOf "git@github.com:"`, and the same for
  the GitLab host. The repository's own config is not changed.
- Commits are made as the host user: `start.sh` passes `user.name` and
  `user.email` from the host's git config.
- Use tokens with the smallest scope and an expiry date: they are the most a
  misbehaving agent can misuse.

## Claude settings in the sandbox

`sbx/settings.json`, copied to `/home/agent/.claude/settings.json` (user
settings; the project's `.claude/settings.json` still applies on top):

- `permissions.defaultMode`: `bypassPermissions`. The container replaces the
  permission prompts.
- `permissions.deny` as a safety net for actions that reach outside the
  project even in a sandbox, e.g. `git push --force`, `git push * --delete`,
  `gh repo delete`, `glab repo delete`.
- No other allow lists needed: shell, file system, write and network access are
  limited by the container, not by the agent.
- The Claude config volume keeps login state, sessions and memory between
  runs; `podman volume rm sprcmtrd-sbx-claude` resets it.
- `CLAUDE_CONFIG_DIR=/home/agent/.claude`, so Claude keeps all its state
  (including onboarding answers) in that volume; the rest of the home
  directory is fresh on every start.
- The settings live in the image (`/etc/sbx/settings.json`) and
  `entrypoint.sh` copies them into the volume on every start, so a changed
  `sbx/settings.json` takes effect without resetting the volume. The entrypoint
  also writes the git config (user, HTTPS rewrite, credential helpers that read
  the tokens from the environment, so they are never written to a file).

## README (`sbx/README.md`)

- Requirements: podman (rootless), the host user's git config.
- First start: copy `env.example` to `.env`, fill in tokens, run `sbx/start.sh`.
- Daily use, updating (`--rebuild`, `--pull`), resetting volumes.
- What the agent can and cannot reach (mounts, network, tokens), and that
  pushes happen with the tokens' rights.

## Decisions

Implemented with the defaults above; each can be changed later:

- Network: unrestricted. An allowlist of hosts (Claude API, GitHub, GitLab,
  crates.io, static.rust-lang.org) would need a filtering proxy container, as
  podman can't filter by host name.
- Pushing: allowed with the tokens in `sbx/.env`. To only commit in the
  sandbox, leave `GH_TOKEN` / `GITLAB_TOKEN` empty and push from the host.
- `target/` is shared with the host (same OS, faster). A separate volume would
  keep builds inside from touching the host's build output.
