#!/usr/bin/env bash
# Prepares the agent's home on every start of the sandbox, then runs the
# command. The home directory is not kept between runs; the Claude config
# directory is a volume, so the settings are copied over it each time.
set -euo pipefail

mkdir -p "$CLAUDE_CONFIG_DIR"
cp /etc/sbx/settings.json "$CLAUDE_CONFIG_DIR/settings.json"

# Git: commit as the host user, push over HTTPS with the tokens from the
# environment. The repository's SSH remotes are rewritten here only; the
# repository's own config is not changed.
git config --global init.defaultBranch main
[[ -n "${SBX_GIT_NAME:-}" ]] && git config --global user.name "$SBX_GIT_NAME"
[[ -n "${SBX_GIT_EMAIL:-}" ]] && git config --global user.email "$SBX_GIT_EMAIL"
if [[ -n "${GH_TOKEN:-}" ]]; then
    git config --global url."https://github.com/".insteadOf "git@github.com:"
    # Reads the token when git asks, so it is never written to a file.
    git config --global credential."https://github.com".helper \
        '!f() { echo username=x-access-token; echo "password=$GH_TOKEN"; }; f'
fi
if [[ -n "${GITLAB_TOKEN:-}" && -n "${GITLAB_HOST:-}" ]]; then
    git config --global url."https://$GITLAB_HOST/".insteadOf "git@$GITLAB_HOST:"
    git config --global credential."https://$GITLAB_HOST".helper \
        '!f() { echo username=oauth2; echo "password=$GITLAB_TOKEN"; }; f'
fi

exec "$@"
