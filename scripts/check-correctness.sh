#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export VELUM_CORRECTNESS_ONLY=1
default_test_jobs=4
if [[ "${GITHUB_ACTIONS:-false}" == "true" ]]; then
  default_test_jobs=30
fi
export VELUM_TEST_JOBS="${VELUM_TEST_JOBS:-${default_test_jobs}}"
export VELUM_TEST262_RUN_ALL=1
unset VELUM_TEST262_PATH_FILTER VELUM_TEST262_FLAG_FILTER
unset VELUM_BENCH_FILTER

exec "${script_dir}/test-all.sh"
