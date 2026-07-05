#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

test262_commit="64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1"
cache_dir="${RSQJS_TEST262_CACHE_DIR:-${repo_root}/target/test262}"
test262_dir="${cache_dir}/test262-${test262_commit}"
tmp_dir="${cache_dir}/.test262-${test262_commit}.tmp"
test262_url="https://github.com/tc39/test262.git"

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

if [[ -n "${RSQJS_TEST262_DIR:-}" ]]; then
  if [[ -d "${RSQJS_TEST262_DIR}" ]]; then
    printf '%s\n' "${RSQJS_TEST262_DIR}"
    exit 0
  fi
  log "RSQJS_TEST262_DIR is set but is not a directory: ${RSQJS_TEST262_DIR}"
  exit 1
fi

if [[ "${RSQJS_TEST262_AUTO_SETUP:-1}" == "0" ]]; then
  log "Test262 auto setup is disabled; full corpus rows will be skipped."
  exit 0
fi

require_tool git

if [[ -d "${test262_dir}/test" && -d "${test262_dir}/harness" ]]; then
  printf '%s\n' "${test262_dir}"
  exit 0
fi

mkdir -p "${test262_dir}"
rm -rf "${tmp_dir}"
mkdir -p "${tmp_dir}"

log "Fetching Test262 ${test262_commit} from ${test262_url}"
git -C "${tmp_dir}" init -q
git -C "${tmp_dir}" remote add origin "${test262_url}"
git -C "${tmp_dir}" fetch --depth 1 origin "${test262_commit}" >/dev/null
git -C "${tmp_dir}" checkout --detach FETCH_HEAD >/dev/null

if [[ ! -d "${tmp_dir}/test" || ! -d "${tmp_dir}/harness" ]]; then
  log "Test262 checkout is incomplete: ${tmp_dir}"
  exit 1
fi

rm -rf "${test262_dir}"
mv "${tmp_dir}" "${test262_dir}"

printf '%s\n' "${test262_dir}"
