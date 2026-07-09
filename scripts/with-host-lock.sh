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

lock_path="${RSQJS_HOST_LOCK_PATH:-/run/lock/rsqjs/host-performance.lock}"
metadata_path="${lock_path}.owner"
lock_dir="$(dirname "${lock_path}")"

if [[ -L "${lock_dir}" ]]; then
  printf 'with-host-lock: lock directory must not be a symlink: %s\n' "${lock_dir}" >&2
  exit 1
fi
if [[ ! -d "${lock_dir}" ]] && ! mkdir -m 0777 "${lock_dir}" 2>/dev/null; then
  if [[ ! -d "${lock_dir}" ]]; then
    printf 'with-host-lock: failed to create shared lock directory: %s\n' "${lock_dir}" >&2
    exit 1
  fi
fi
if [[ ! -w "${lock_dir}" ]]; then
  printf 'with-host-lock: lock directory is not writable: %s\n' "${lock_dir}" >&2
  exit 1
fi
if [[ -L "${lock_path}" || -L "${metadata_path}" ]]; then
  printf 'with-host-lock: lock and owner metadata must be regular paths\n' >&2
  exit 1
fi

previous_umask="$(umask)"
umask 000
if ! exec {lock_fd}>>"${lock_path}"; then
  printf 'with-host-lock: failed to open shared lock: %s\n' "${lock_path}" >&2
  exit 1
fi
umask "${previous_umask}"

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
if ! rm -f "${metadata_path}"; then
  printf 'with-host-lock: failed to clear stale owner metadata %s\n' "${metadata_path}" >&2
  exit 1
fi
trap cleanup_metadata EXIT
{
  printf 'pid=%s\n' "$$"
  printf 'uid=%s\n' "$(id -u)"
  printf 'host=%s\n' "$(hostname)"
  printf 'started_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'cwd=%q\n' "${PWD}"
  printf 'command='
  printf '%q ' "$@"
  printf '\n'
} > "${metadata_path}"
printf 'with-host-lock: acquired exclusive benchmark slot; owner metadata: %s\n' "${metadata_path}"
"$@"
