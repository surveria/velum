#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

if (($# < 1 || $# > 2)); then
  printf 'usage: %s path-filter [report-path]\n' "$0" >&2
  exit 2
fi

path_filter="$1"
if [[ -z "${path_filter//[[:space:]]/}" ]]; then
  printf 'Test262 path filter must not be empty\n' >&2
  exit 2
fi

timestamp="${RSQJS_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${2:-target/rsqjs-reports/focused/test262-${timestamp}.md}"
quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
test262_path="$("${script_dir}/prepare-test262.sh")"

export RSQJS_BUILD_REPO_ROOT="${RSQJS_BUILD_REPO_ROOT:-${repo_root}}"
export RSQJS_BUILD_COMMIT_SHA="${RSQJS_BUILD_COMMIT_SHA:-$(git rev-parse HEAD)}"
export RSQJS_QUICKJS="${quickjs_path}"
export RSQJS_TEST262_DIR="${test262_path}"
export RSQJS_TEST262_RUN_ALL=1
export RSQJS_TEST262_PATH_FILTER="${path_filter}"
export RSQJS_TEST_JOBS="${RSQJS_TEST_JOBS:-30}"
export RSQJS_REPORT_TIMESTAMP="${RSQJS_REPORT_TIMESTAMP:-${timestamp}}"
export RSQJS_REPORT_COMMIT_SHA="${RSQJS_REPORT_COMMIT_SHA:-$(git rev-parse HEAD)}"
export RSQJS_REPORT_TREE_SHA="${RSQJS_REPORT_TREE_SHA:-$(git rev-parse 'HEAD^{tree}')}"
unset RSQJS_TEST262_UPDATE_PASS_BASELINE RSQJS_TEST262_PASS_CANDIDATE_PATH

cargo run --release --manifest-path runner/Cargo.toml -- --correctness "${report_path}"
printf 'focused Test262 report: %s\n' "${report_path}"
