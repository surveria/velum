#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

test262_commit="64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1"
manifest_path="${repo_root}/tests/corpora/test262/manifest.tsv"
cache_dir="${RSQJS_TEST262_CACHE_DIR:-${repo_root}/target/test262}"
test262_dir="${cache_dir}/test262-${test262_commit}"
raw_base_url="https://raw.githubusercontent.com/tc39/test262/${test262_commit}"

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
  log "Test262 auto setup is disabled; upstream manifest rows will be skipped."
  exit 0
fi

require_tool curl

if [[ ! -f "${manifest_path}" ]]; then
  log "Test262 manifest is missing: ${manifest_path}"
  exit 1
fi

mkdir -p "${test262_dir}"

while IFS=$'\t' read -r case_id relative_path mode reason; do
  if [[ -z "${case_id}" || "${case_id}" == \#* ]]; then
    continue
  fi
  if [[ -z "${relative_path}" || -z "${mode}" || -z "${reason}" ]]; then
    log "Invalid Test262 manifest row for case: ${case_id}"
    exit 1
  fi

  target_path="${test262_dir}/${relative_path}"
  if [[ -f "${target_path}" ]]; then
    continue
  fi

  mkdir -p "$(dirname "${target_path}")"
  curl -fsSL "${raw_base_url}/${relative_path}" -o "${target_path}"
done <"${manifest_path}"

printf '%s\n' "${test262_dir}"
