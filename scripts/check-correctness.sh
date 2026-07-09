#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export RSQJS_CORRECTNESS_ONLY=1
export RSQJS_TEST_JOBS="${RSQJS_TEST_JOBS:-4}"

exec "${script_dir}/with-host-lock.sh" shared -- "${script_dir}/test-all.sh"
