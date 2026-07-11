#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# Cheap PR/local gate: keep source quality high without materializing external
# corpora or running sequential QuickJS benchmark/report generation.
"${script_dir}/check-vendored-regress.sh"
"${script_dir}/check-touched-file-sizes.sh" "${RSQJS_BASE_REF:-origin/main}"
"${script_dir}/check-architecture-boundaries.sh" --self-test
"${script_dir}/test-report-artifact-metadata.sh"
"${script_dir}/test-jetstream-artifact-metadata.sh"

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

if [[ "${RSQJS_FAST_RUNNER:-0}" == "1" ]]; then
  cargo fmt --manifest-path runner/Cargo.toml --all -- --check
  cargo clippy --manifest-path runner/Cargo.toml --all-targets --all-features -- -D warnings
  cargo test --manifest-path runner/Cargo.toml --all-targets --all-features
  RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path runner/Cargo.toml --no-deps --all-features
fi
