#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s shared|exclusive -- command [args...]\n' "${0##*/}" >&2
  exit 2
}

mode="${1:-}"
if [[ "${mode}" != "shared" && "${mode}" != "exclusive" ]]; then
  usage
fi
shift

if [[ "${1:-}" != "--" ]]; then
  usage
fi
shift

if (( $# == 0 )); then
  usage
fi

if ! command -v flock >/dev/null 2>&1; then
  printf 'with-host-lock: missing required command: flock\n' >&2
  exit 1
fi

lock_path="${RSQJS_HOST_LOCK_PATH:-/tmp/rsqjs-host-performance.lock}"
metadata_path="${lock_path}.owner"
mkdir -p "$(dirname "${lock_path}")"
exec {lock_fd}>"${lock_path}"

if [[ "${mode}" == "shared" ]]; then
  printf 'with-host-lock: waiting for shared correctness slot on %s\n' "${lock_path}"
  flock --shared "${lock_fd}"
  printf 'with-host-lock: acquired shared correctness slot\n'
  export RSQJS_HOST_LOCK_HELD=shared
  exec "$@"
fi

cleanup_metadata() {
  if ! rm -f "${metadata_path}"; then
    printf 'with-host-lock: failed to remove owner metadata %s\n' "${metadata_path}" >&2
  fi
}

printf 'with-host-lock: waiting for exclusive benchmark slot on %s\n' "${lock_path}"
flock --exclusive "${lock_fd}"
export RSQJS_HOST_LOCK_HELD=exclusive
trap cleanup_metadata EXIT
{
  printf 'pid=%s\n' "$$"
  printf 'started_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'cwd=%q\n' "${PWD}"
  printf 'command='
  printf '%q ' "$@"
  printf '\n'
} > "${metadata_path}"
printf 'with-host-lock: acquired exclusive benchmark slot; owner metadata: %s\n' "${metadata_path}"
"$@"
