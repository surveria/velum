#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# The runner lives in the `runner/` submodule (rs-quickjs-testing); make sure it
# is checked out before building or running it.
git submodule update --init --recursive

timestamp="${RSQJS_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${RSQJS_TEST_REPORT_PATH:-reports/test-runs/rsqjs-test-report-${timestamp}.md}"

quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
if [[ -n "${quickjs_path}" ]]; then
  export RSQJS_QUICKJS="${quickjs_path}"
fi

test262_path="$("${script_dir}/prepare-test262.sh")"
if [[ -n "${test262_path}" ]]; then
  export RSQJS_TEST262_DIR="${test262_path}"
fi
export RSQJS_TEST262_RUN_ALL="${RSQJS_TEST262_RUN_ALL:-1}"

# Engine crate (repository root).
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Runner crate (its own workspace inside the `runner/` submodule). Override its
# git dependency on the engine with THIS checkout, addressed by an absolute
# path, so a nested worktree measures its own branch rather than the main
# checkout that cargo would otherwise pick up via its upward config search.
engine_override=(--config "paths=['${repo_root}']")
cargo fmt --manifest-path runner/Cargo.toml --all -- --check
cargo clippy --manifest-path runner/Cargo.toml "${engine_override[@]}" --all-targets --all-features -- -D warnings
cargo test --manifest-path runner/Cargo.toml "${engine_override[@]}" --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path runner/Cargo.toml "${engine_override[@]}" --no-deps --all-features

# Build the engine CLI, then run the report/benchmarks through the runner, which
# drives everything in-process and compares against an embedded QuickJS
# reference behind the `reference-quickjs` feature.
cargo build --release --bin rsqjs
cargo run --release --manifest-path runner/Cargo.toml "${engine_override[@]}" --features reference-quickjs -- --report "${report_path}"

printf 'test report: %s\n' "${report_path}"
