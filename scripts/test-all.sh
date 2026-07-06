#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

timestamp="${RSQJS_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${RSQJS_TEST_REPORT_PATH:-reports/test-runs/rsqjs-test-report-${timestamp}.md}"
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"

quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
if [[ -n "${quickjs_path}" ]]; then
  export RSQJS_QUICKJS="${quickjs_path}"
fi

test262_path="$("${script_dir}/prepare-test262.sh")"
if [[ -n "${test262_path}" ]]; then
  export RSQJS_TEST262_DIR="${test262_path}"
fi
export RSQJS_TEST262_RUN_ALL="${RSQJS_TEST262_RUN_ALL:-1}"

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
# The runner drives all benchmarks in-process, comparing against an embedded
# QuickJS reference behind the `reference-quickjs` feature.
cargo build --release --features reference-quickjs --bin rsqjs --bin rsqjs-test-runner

"${target_dir}/release/rsqjs-test-runner" --report "${report_path}"

printf 'test report: %s\n' "${report_path}"
