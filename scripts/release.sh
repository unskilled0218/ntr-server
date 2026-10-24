#!/usr/bin/env bash
# Adapted from container-release-template 3c73528231ac1cbdd43600a5ca723390db804bac.
set -Eeuo pipefail
set +x
export PYTHONDONTWRITEBYTECODE=1
usage() {
  echo 'Usage: ./scripts/release.sh [--validate | --help]'
  echo '       ./scripts/release.sh --associate-existing <tag> --expected-tag-oid <oid> --expected-source <commit>'
}
mode=publish
case "$#:${1:-}" in
  1:--help)
    usage
    echo 'No arguments: build and publish the configured project. Setup: scripts/release.json and a private token file.'
    exit 0 ;;
  1:--validate|0:) ;;
  6:--associate-existing)
    [[ ${#2} -le 17 && "$2" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ &&
       "$3" == --expected-tag-oid && "$4" =~ ^[0-9a-f]{40}$ &&
       "$5" == --expected-source && "$6" =~ ^[0-9a-f]{40}$ ]] || { usage >&2; exit 2; }
    mode=associate ;;
  *) echo 'Unsupported arguments.' >&2; usage >&2; exit 2 ;;
esac
for deprecated in "${!FORGEJO_@}"; do
  echo 'Deprecated FORGEJO_* release overrides are refused; use scripts/release.json and a private token file.' >&2
  exit 2
done
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
source "$root/scripts/release-common.sh"
command -v python3 >/dev/null || stop 'Install Python 3.11 or later (no jq required)'
if [[ "${1:-}" == --validate ]]; then
  data settings "$root" /dev/null
  echo 'Configuration valid; no credentials or remote operations used'
  exit 0
fi
tools=(git curl)
[[ "$mode" != publish ]] || tools+=(docker)
for tool in "${tools[@]}"; do
  command -v "$tool" >/dev/null || stop "Required command unavailable: $tool"
done
for marker in release-template.lock.json scripts/release-template .template-transaction; do
  [[ ! -e "$root/$marker" && ! -L "$root/$marker" ]] || stop 'Old installation state; use its preserved original checkout/pin, not this command'
done
if [[ -z "${RELEASE_LOCK_FD:-}" ]]; then
  exec python3 -B "$root/scripts/release-data.py" locked "$root" "$root/scripts/release.sh" "$@"
fi
data lock-check "$root"
umask 077
work=$(mktemp -d "${TMPDIR:-/tmp}/bash-release.XXXXXXXX")
phase='local setup'
cleanup() { rm -rf -- "$work"; }
trap cleanup EXIT
trap 'echo "Publication incomplete/uncertain during $phase; inspect exact remaining objects without rollback" >&2' ERR
trap 'exit 130' INT
trap 'exit 143' TERM
started=$(data clock)
export RELEASE_PROGRESS_START=$started
stage 'local setup'
data settings "$root" "$work/settings.json"
tag=$(field "$work/settings.json" tag)
version=$(field "$work/settings.json" version)
origin=$(field "$work/settings.json" gitea)
repository=$(field "$work/settings.json" repository)
image=$(field "$work/settings.json" image)
if [[ "$mode" == associate ]]; then
  printf 'Association target: %s / %s (%s)\n' "$repository" "$image" "$2"
else
  printf 'Release target: %s / %s (%s)\n' "$repository" "$image" "$tag"
fi
[[ ! -e "$root/.release-template/$tag.json" && ! -L "$root/.release-template/$tag.json" ]] || stop 'Legacy checkpoint exists; preserve its original checkout/pin'
if [[ "$mode" == associate ]]; then
  stage 'existing recovery'
  data source "$root" >/dev/null || stop 'Cannot verify recovery tool source'
  data auth-api "$root" "$work"
  data recover-association "$root" "$work" "$2" "$4" "$6"
  exit 0
fi
stage 'source and builder preflight'
original_docker_context=${DOCKER_CONTEXT-}
original_docker_config=${DOCKER_CONFIG:-$HOME/.docker}
run 1048576 60 docker buildx inspect --bootstrap
data platforms "$work/command.out"
# Capture the CLI's actual selected plugin, not a guessed installation path.
run 1048576 60 docker info --format '{{json .ClientInfo.Plugins}}'
data plugin "$work/command.out" "$work/plugin.json"
commit=$(data source "$root") || stop 'Cannot verify release source'
data payload "$root" "$commit" "$work/payload.json"
data auth "$root" "$work"
registry_host=${image%%/*}
export BUILDX_CONFIG=${BUILDX_CONFIG:-$original_docker_config/buildx}
run 65536 60 docker context show
context=$(cat "$work/command.out") || stop 'Cannot read selected Docker context'
[[ -n "$context" && "$context" != *$'\n'* && "$context" != *$'\r'* ]] || stop 'Invalid selected Docker context'
export DOCKER_CONFIG="$work/docker"
if [[ "$context" != default || -n "$original_docker_context" ]]; then
  export DOCKER_CONTEXT="$context"
else
  unset DOCKER_CONTEXT
fi
run 1048576 60 docker info --format '{{json .ClientInfo.Plugins}}'
data plugin-check "$work/command.out" "$work/plugin.json"
stage 'read-only startup classification'
data inventory "$root" "$work" "$work/inventory.json"
tag_state
if [[ -n "$remote_oid" || -n "$release_record" ]]; then
  [[ -n "$remote_oid" && -n "$release_record" && "$remote_commit" == "$commit" ]] || stop 'Ambiguous tag-only/release-only or conflicting startup state; inspect without repair'
  stage 'completed-publication verification'
  run 1048576 60 git -C "$root" fetch --no-tags --no-write-fetch-head origin "$remote_oid"
  data inspect-tag "$root" "$commit" "$remote_oid" "$work/identity.json"
  prepared=$(field "$work/identity.json" digest)
  printf '%s\n' "$release_record" > "$work/release.json"
  data release-check "$root" "$commit" "$work/release.json"
  [[ $(data image "$root" "$work" "$prepared" "$commit") == "$prepared" ]] || stop 'Bound image differs'
  [[ $(data resolve "$root" "$work" latest) == "$prepared" ]] || stop 'Completed latest is missing or foreign; no repair authority'
  scan
  [[ $(field "$work/inventory.json" object) == "$remote_oid" && $(field "$work/inventory.json" commit) == "$commit" ]] || stop 'Completed tag changed'
  field "$work/inventory.json" release > "$work/release.json"
  data release-check "$root" "$commit" "$work/release.json"
  [[ $(data resolve "$root" "$work" latest) == "$prepared" ]] || stop 'Completed latest changed during verification'
  associate_package "$remote_oid"
  success
  exit 0
fi
stage 'fresh-publication preparation'
tag_confirmed=false
promotion_confirmed=false
run 65536 60 git -C "$root" tag --list "$tag"
[[ ! -s "$work/command.out" ]] || stop 'Local tag exists without a remote publication; inspect before building'
predecessor=$(data resolve "$root" "$work" latest) || stop 'Cannot capture predecessor alias'
config_hash=$(field "$work/settings.json" config_hash)
staging="rt-${version}-${commit}-${config_hash#sha256:}"
prepared=$(data image "$root" "$work" "$staging" "$commit" missing) || stop 'Cannot verify staging evidence'
if [[ -z "$prepared" ]]; then
  fresh_guard
  login
  data snapshot "$root" "$commit" "$work/source"
  stage 'staging build and immutable verification'
  run 16777216 7200 docker buildx build --platform linux/amd64,linux/arm64 --provenance=false \
    --metadata-file "$work/build.json" --push --tag "$image:$staging" \
    --label "org.opencontainers.image.version=$version" --label "org.opencontainers.image.revision=$commit" \
    --label "org.opencontainers.image.source=$origin/$repository" --file "$work/source/Dockerfile" "$work/source"
  prepared=$(data image "$root" "$work" "$staging" "$commit") || stop 'Cannot verify built image'
  data metadata "$work/build.json" "$prepared"
else
  data progress 'staging reuse'
  login
fi
oid=$(data make-tag "$root" "$commit" "$prepared" "$predecessor" "$work/identity.json") || stop 'Cannot create exact intended tag'
stage 'tag publication'
fresh_guard
attempt run 1048576 60 git -C "$root" push --no-follow-tags origin "$oid:refs/tags/$tag"
scan
tag_state
[[ "$remote_oid" == "$oid" && "$remote_commit" == "$commit" && -z "$release_record" ]] || stop 'Tag push not confirmed or release appeared'
tag_confirmed=true
stage 'digest promotion'
fresh_guard
promotion_latest=$(data resolve "$root" "$work" latest) || stop 'Cannot read latest before promotion'
[[ "$promotion_latest" == "$predecessor" || "$promotion_latest" == "$prepared" ]] || stop 'Latest changed before promotion'
if [[ "$promotion_latest" != "$prepared" ]]; then
  attempt run 1048576 60 docker buildx imagetools create --tag "$image:latest" "$image@$prepared"
fi
[[ $(data resolve "$root" "$work" latest) == "$prepared" ]] || stop 'Promotion not confirmed'
promotion_confirmed=true
stage 'release creation'
fresh_guard
attempt data http "$work" POST "$origin/api/v1/repos/$repository/releases" "$work/post.json" "$work/payload.json" > "$work/post.status"
stage 'final publication verification'
scan
tag_state
[[ "$remote_oid" == "$oid" && "$remote_commit" == "$commit" && -n "$release_record" ]] || stop 'Release creation not confirmed'
printf '%s\n' "$release_record" > "$work/release.json"
data release-check "$root" "$commit" "$work/release.json"
[[ $(data image "$root" "$work" "$prepared" "$commit") == "$prepared" && $(data resolve "$root" "$work" latest) == "$prepared" ]] || stop 'Final image verification failed'
associate_package "$oid"
success
