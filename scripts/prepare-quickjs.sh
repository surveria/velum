#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

quickjs_version="2026-06-04"
quickjs_archive="quickjs-${quickjs_version}.tar.xz"
quickjs_url="https://bellard.org/quickjs/${quickjs_archive}"
quickjs_sha256="b376e839b322978313d929fd20663b11ba58b75df5a46c126dd19ea2fa70ad2a"
quickjs_git_url="https://github.com/bellard/quickjs.git"
quickjs_git_commit="04be246001599f5995fa2f2d8c91a0f198d3f34c"

cache_dir="${VELUM_QUICKJS_CACHE_DIR:-${repo_root}/target/quickjs}"
archive_path="${cache_dir}/${quickjs_archive}"
source_mode="${VELUM_QUICKJS_SOURCE:-archive}"
case "${source_mode}" in
  archive)
    source_dir="${cache_dir}/quickjs-${quickjs_version}"
    ;;
  git)
    source_dir="${cache_dir}/quickjs-${quickjs_version}-git"
    ;;
  *)
    source_dir="${cache_dir}/quickjs-${quickjs_version}-${source_mode}"
    ;;
esac
tmp_dir="${cache_dir}/.quickjs-${quickjs_version}-${source_mode}.tmp"
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

build_quickjs() {
  log "Building QuickJS ${quickjs_version}"
  make -C "${source_dir}" qjs CONFIG_LTO= >/dev/null
}

if [[ -n "${VELUM_QUICKJS:-}" ]]; then
  if [[ -x "${VELUM_QUICKJS}" ]]; then
    printf '%s\n' "${VELUM_QUICKJS}"
    exit 0
  fi
  log "VELUM_QUICKJS is set but is not executable: ${VELUM_QUICKJS}"
  exit 1
fi

if command -v qjs >/dev/null 2>&1; then
  command -v qjs
  exit 0
fi

if [[ "${VELUM_QUICKJS_AUTO_SETUP:-1}" == "0" ]]; then
  log "QuickJS auto setup is disabled; reference checks will be skipped."
  exit 0
fi

require_tool make
require_tool gcc

mkdir -p "${cache_dir}"

case "${source_mode}" in
  archive)
    require_tool curl
    require_tool sha256sum
    require_tool tar

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
      build_quickjs
    fi
    ;;
  git)
    require_tool git
    if [[ ! -x "${quickjs_binary}" ]]; then
      rm -rf "${tmp_dir}"
      mkdir -p "${tmp_dir}"
      log "Fetching QuickJS ${quickjs_git_commit} from ${quickjs_git_url}"
      git -C "${tmp_dir}" init -q
      git -C "${tmp_dir}" remote add origin "${quickjs_git_url}"
      git -C "${tmp_dir}" fetch --depth 1 origin "${quickjs_git_commit}" >/dev/null
      git -C "${tmp_dir}" checkout --detach FETCH_HEAD >/dev/null

      if [[ "$(cat "${tmp_dir}/VERSION")" != "${quickjs_version}" ]]; then
        log "QuickJS git version mismatch."
        log "Expected: ${quickjs_version}"
        log "Actual:   $(cat "${tmp_dir}/VERSION")"
        exit 1
      fi

      rm -rf "${source_dir}"
      mv "${tmp_dir}" "${source_dir}"
      build_quickjs
    fi
    ;;
  *)
    log "Unsupported QuickJS source mode: ${source_mode}"
    exit 1
    ;;
esac

if [[ ! -x "${quickjs_binary}" ]]; then
  log "QuickJS binary was not produced: ${quickjs_binary}"
  exit 1
fi

printf '%s\n' "${quickjs_binary}"
