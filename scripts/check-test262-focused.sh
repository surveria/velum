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

timestamp="${VELUM_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${2:-target/velum-reports/focused/test262-${timestamp}.md}"
quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
test262_path="$("${script_dir}/prepare-test262.sh")"

export VELUM_BUILD_REPO_ROOT="${VELUM_BUILD_REPO_ROOT:-${repo_root}}"
export VELUM_BUILD_COMMIT_SHA="${VELUM_BUILD_COMMIT_SHA:-$(git rev-parse HEAD)}"
export VELUM_QUICKJS="${quickjs_path}"
export VELUM_TEST262_DIR="${test262_path}"
export VELUM_TEST262_RUN_ALL=1
export VELUM_TEST262_PATH_FILTER="${path_filter}"
export VELUM_TEST_JOBS="${VELUM_TEST_JOBS:-30}"
export VELUM_REPORT_TIMESTAMP="${VELUM_REPORT_TIMESTAMP:-${timestamp}}"
export VELUM_REPORT_COMMIT_SHA="${VELUM_REPORT_COMMIT_SHA:-$(git rev-parse HEAD)}"
export VELUM_REPORT_TREE_SHA="${VELUM_REPORT_TREE_SHA:-$(git rev-parse 'HEAD^{tree}')}"
unset VELUM_TEST262_UPDATE_PASS_BASELINE VELUM_TEST262_PASS_CANDIDATE_PATH

cargo run --release --manifest-path runner/Cargo.toml -- --correctness "${report_path}"
printf 'focused Test262 report: %s\n' "${report_path}"
