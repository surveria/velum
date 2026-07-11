#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export RSQJS_CORRECTNESS_ONLY=1
default_test_jobs=4
if [[ "${GITHUB_ACTIONS:-false}" == "true" ]]; then
  default_test_jobs=30
fi
export RSQJS_TEST_JOBS="${RSQJS_TEST_JOBS:-${default_test_jobs}}"
export RSQJS_TEST262_RUN_ALL=1
unset RSQJS_TEST262_PATH_FILTER RSQJS_TEST262_FLAG_FILTER
unset RSQJS_BENCH_FILTER

exec "${script_dir}/test-all.sh"
