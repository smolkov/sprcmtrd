# Sandbox for Claude Code

Runs the Claude Code agent in a rootless podman container, so it can work on
this project without permission prompts while it only reaches the project
directory. The container is the security boundary; the specification is in
[`sandbox.md`](sandbox.md).

## Requirements

- podman (rootless): `sudo apt install podman`
- git with `user.name` and `user.email` set (commits in the sandbox use them)
- Tokens (see below)

## First start

```sh
cp sbx/env.example sbx/.env
chmod 600 sbx/.env
$EDITOR sbx/.env        # fill in the tokens
sbx/start.sh
```

The first start builds the image `sprcmtrd-sbx` (Debian trixie, Rust, Claude
Code; a few minutes). Claude asks once to confirm the bypass-permissions mode;
the answer is kept.

## Tokens (`sbx/.env`)

| Variable                  | What for                                                            |
|---------------------------|---------------------------------------------------------------------|
| `CLAUDE_CODE_OAUTH_TOKEN` | Claude subscription: run `claude setup-token` on the host and paste |
| `ANTHROPIC_API_KEY`       | Instead of the above: an API key (billed per use)                    |
| `GH_TOKEN`                | GitHub: fine-grained token, only this repository (contents read/write, pull requests, actions read) |
| `GITLAB_TOKEN`            | GitLab: project access token (`api`, `write_repository`)            |
| `GITLAB_HOST`             | `gitlab.fe.sprecher-automation.com`                                 |

`sbx/.env` is gitignored and never copied into the image. Give the tokens the
smallest scope and an expiry date: they are what the agent can push and
change with.

## Daily use

```sh
sbx/start.sh                    # Claude in the sandbox
sbx/start.sh --resume           # arguments go to claude
sbx/start.sh shell              # bash in the sandbox
sbx/start.sh shell -c 'cargo test --locked'
sbx/start.sh --rebuild          # newer Claude Code, Rust and packages
sbx/start.sh --pull             # also a newer debian:trixie-slim
```

The image is rebuilt automatically when `Containerfile`, `settings.json` or
`entrypoint.sh` change. Every start creates a fresh container, removed on exit.

## What the agent can reach

| Can                                                        | Cannot                                      |
|------------------------------------------------------------|---------------------------------------------|
| Read and write the repository (at its host path)           | Your home directory, `~/.ssh`, other projects |
| Build and test (shares `target/` with the host)            | Your host `~/.claude` (it has its own)      |
| The network (Claude API, GitHub, crates.io, GitLab on VPN) | Listen for connections (no ports published) |
| Push to GitHub / GitLab with the tokens in `sbx/.env`      | Use your SSH keys                           |
|                                                            | Force-push or delete branches, repositories and releases (denied in `settings.json`) |

- Git remotes stay as they are (`git@github.com:…`); inside the sandbox they
  are rewritten to HTTPS and authenticated with the tokens.
- Files the agent writes belong to you (the host user is mapped to `agent`).
- Claude's sessions, login state and memory are kept in the volume
  `sprcmtrd-sbx-claude`, separate from your own Claude Code. Downloaded crates
  are kept in `sprcmtrd-sbx-cargo`.

## Reset

```sh
podman volume rm sprcmtrd-sbx-claude    # forget Claude sessions, login, memory
podman volume rm sprcmtrd-sbx-cargo     # crate download cache
podman rmi sprcmtrd-sbx                 # the image; rebuilt on the next start
```

## Troubleshooting

- `User-selected graph driver "overlay" overwritten by graph driver "vfs"`:
  podman's storage was created with `vfs` and the configuration now asks for
  `overlay`. It works, but `vfs` uses much more disk space and builds slowly.
  `podman system reset` switches to `overlay`; it deletes **all** local podman
  images, containers and volumes.
- GitLab push fails: the host must be on the company VPN; the container uses
  the host's network routes.
