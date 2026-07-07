#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# Cheap PR/local gate: keep source quality high without materializing external
# corpora or running sequential QuickJS benchmark/report generation.
if [[ -n "${RSQJS_BASE_REF:-}" ]]; then
  "${script_dir}/check-version-bump.sh" "${RSQJS_BASE_REF}"
fi

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

if [[ "${RSQJS_FAST_RUNNER:-0}" == "1" ]]; then
  git submodule update --init --recursive
  engine_override=(--config "paths=['${repo_root}']")
  cargo fmt --manifest-path runner/Cargo.toml --all -- --check
  cargo clippy --manifest-path runner/Cargo.toml "${engine_override[@]}" --all-targets --all-features -- -D warnings
  cargo test --manifest-path runner/Cargo.toml "${engine_override[@]}" --all-targets --all-features
  RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path runner/Cargo.toml "${engine_override[@]}" --no-deps --all-features
fi
