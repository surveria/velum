#!/usr/bin/env bash
set -euo pipefail

if [[ "${1:-}" == --version ]]; then
  printf 'cargo 0.0.0 (campaign fixture)\n'
  exit 0
fi
[[ "${1:-}" == build ]] || exit 64
[[ "$(<workload.txt)" == 'committed workload' ]] || exit 65
[[ "${PWD}" == "${VELUM_BUILD_REPO_ROOT}" ]] || exit 66
[[ "${CARGO_TARGET_DIR}" == */sources/"${VELUM_BUILD_COMMIT_SHA}" ]] || exit 67

# The original checkout changes after the launcher has captured its commit.
printf 'changed during mock build\n' > "${MOCK_ORIGINAL_CHECKOUT}/workload.txt"
mkdir -p "${CARGO_TARGET_DIR}/release"
cp scripts/mock-runner.sh "${CARGO_TARGET_DIR}/release/velum-test-runner"
chmod +x "${CARGO_TARGET_DIR}/release/velum-test-runner"
printf 'mock build source: %s\n' "${PWD}"
