#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

timestamp="${RSQJS_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${RSQJS_TEST_REPORT_PATH:-reports/test-runs/rsqjs-test-report-${timestamp}.md}"

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo run --quiet --bin rsqjs-test-runner -- --report "${report_path}"

printf 'test report: %s\n' "${report_path}"
