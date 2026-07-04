#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

quickjs_version="2026-06-04"
quickjs_archive="quickjs-${quickjs_version}.tar.xz"
quickjs_url="https://bellard.org/quickjs/${quickjs_archive}"
quickjs_sha256="b376e839b322978313d929fd20663b11ba58b75df5a46c126dd19ea2fa70ad2a"

cache_dir="${RSQJS_QUICKJS_CACHE_DIR:-${repo_root}/target/quickjs}"
archive_path="${cache_dir}/${quickjs_archive}"
source_dir="${cache_dir}/quickjs-${quickjs_version}"
quickjs_binary="${source_dir}/qjs"

log() {
  printf '%s\n' "$*" >&2
}

require_tool() {
  local tool="$1"
  if command -v "${tool}" >/dev/null 2>&1; then
    return 0
  fi
  log "Missing required tool: ${tool}"
  return 1
}

if [[ -n "${RSQJS_QUICKJS:-}" ]]; then
  if [[ -x "${RSQJS_QUICKJS}" ]]; then
    printf '%s\n' "${RSQJS_QUICKJS}"
    exit 0
  fi
  log "RSQJS_QUICKJS is set but is not executable: ${RSQJS_QUICKJS}"
  exit 1
fi

if command -v qjs >/dev/null 2>&1; then
  command -v qjs
  exit 0
fi

if [[ "${RSQJS_QUICKJS_AUTO_SETUP:-1}" == "0" ]]; then
  log "QuickJS auto setup is disabled; reference checks will be skipped."
  exit 0
fi

require_tool curl
require_tool sha256sum
require_tool tar
require_tool make
require_tool gcc

mkdir -p "${cache_dir}"

if [[ ! -f "${archive_path}" ]]; then
  log "Downloading QuickJS ${quickjs_version} from ${quickjs_url}"
  curl -fsSL "${quickjs_url}" -o "${archive_path}"
fi

read -r actual_sha256 _ < <(sha256sum "${archive_path}")
if [[ "${actual_sha256}" != "${quickjs_sha256}" ]]; then
  log "QuickJS archive checksum mismatch."
  log "Expected: ${quickjs_sha256}"
  log "Actual:   ${actual_sha256}"
  exit 1
fi

if [[ ! -x "${quickjs_binary}" ]]; then
  rm -rf "${source_dir}"
  tar -xJf "${archive_path}" -C "${cache_dir}"
  log "Building QuickJS ${quickjs_version}"
  make -C "${source_dir}" qjs CONFIG_LTO= >/dev/null
fi

if [[ ! -x "${quickjs_binary}" ]]; then
  log "QuickJS binary was not produced: ${quickjs_binary}"
  exit 1
fi

printf '%s\n' "${quickjs_binary}"
