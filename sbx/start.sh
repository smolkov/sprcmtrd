#!/usr/bin/env bash
# Starts Claude Code in the sandbox container; see sbx/README.md.
#
#   sbx/start.sh [--rebuild] [--pull] [shell [bash-args...] | claude-args...]
set -euo pipefail

sbx_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$sbx_dir/.." && pwd)
image=sprcmtrd-sbx
env_file=$sbx_dir/.env

die() {
    echo "sbx: $*" >&2
    exit 1
}

usage() {
    sed -n '2,4s/^# \{0,1\}//p' "${BASH_SOURCE[0]}"
    cat <<'EOF'

  --rebuild   build the image again (newer Claude Code, Rust, packages)
  --pull      also fetch a newer debian:trixie-slim (implies --rebuild)
  shell       open bash in the sandbox instead of Claude (`shell -c CMD` runs CMD)
  other args  passed to claude, e.g. `sbx/start.sh --resume`
EOF
}

rebuild=false
pull=false
while [[ $# -gt 0 ]]; do
    case $1 in
        --rebuild) rebuild=true ;;
        --pull) rebuild=true pull=true ;;
        -h | --help)
            usage
            exit 0
            ;;
        *) break ;;
    esac
    shift
done
if [[ ${1:-} == shell ]]; then
    shift
    cmd=(bash "$@")
else
    cmd=(claude --dangerously-skip-permissions "$@")
fi

command -v podman >/dev/null || die "podman is not installed (apt install podman)"
[[ -f $env_file ]] || die "missing $env_file: copy sbx/env.example to sbx/.env and fill in the tokens"
grep -Eq '^(CLAUDE_CODE_OAUTH_TOKEN|ANTHROPIC_API_KEY)=.+' "$env_file" ||
    die "$env_file needs CLAUDE_CODE_OAUTH_TOKEN or ANTHROPIC_API_KEY"
if [[ $(stat -c %a "$env_file") != 600 ]]; then
    echo "sbx: warning: $env_file is readable by others; run chmod 600 $env_file" >&2
fi

# Build when the image is missing or its inputs changed since it was built.
hash=$(cat "$sbx_dir/Containerfile" "$sbx_dir/settings.json" "$sbx_dir/entrypoint.sh" | sha256sum | cut -c1-16)
built=$(podman image inspect --format '{{ index .Labels "sbx.hash" }}' "$image" 2>/dev/null || true)
if $rebuild || [[ $built != "$hash" ]]; then
    echo "sbx: building image $image ..." >&2
    # One working container, committed once: with podman's vfs storage every
    # committed step would be a full copy of the image.
    build=(podman build --layers=false --label "sbx.hash=$hash" --tag "$image" --file "$sbx_dir/Containerfile")
    $pull && build+=(--pull=always)
    "${build[@]}" "$sbx_dir"
fi

run=(
    podman run --rm --interactive
    --hostname "$image"
    # The host user becomes `agent` (uid 1000) inside, so files keep the host owner.
    --userns=keep-id:uid=1000,gid=1000
    # The project at its host path; nothing else from the host.
    --volume "$repo:$repo:Z"
    --workdir "$repo"
    --volume "$image-claude:/home/agent/.claude"
    --volume "$image-cargo:/home/agent/.cargo/registry"
    --env-file "$env_file"
    --env "SBX_GIT_NAME=$(git -C "$repo" config user.name || true)"
    --env "SBX_GIT_EMAIL=$(git -C "$repo" config user.email || true)"
    --env "TERM=${TERM:-xterm-256color}"
    --env "COLORTERM=${COLORTERM:-}"
)
[[ -t 0 ]] && run+=(--tty)
exec "${run[@]}" "$image" "${cmd[@]}"
