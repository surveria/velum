#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# The runner lives in the `runner/` submodule (rs-quickjs-testing); check it out
# before formatting, linting, or building it. Its git dependency on the engine
# is overridden with THIS checkout (absolute path) so every step measures the
# local engine, not the published one, and a nested worktree measures its own
# branch rather than the main checkout cargo would find via its config search.
git submodule update --init --recursive
engine_override=(--config "paths=['${repo_root}']")

# --- Fast gates: run the cheap checks first so the pipeline stops before it
# compiles anything or downloads corpora. On a pull request CI sets
# RSQJS_BASE_REF, which turns on the engine version-bump check against the base.
if [[ -n "${RSQJS_BASE_REF:-}" ]]; then
  "${script_dir}/check-version-bump.sh" "${RSQJS_BASE_REF}"
fi
cargo fmt --all -- --check
cargo fmt --manifest-path runner/Cargo.toml --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --manifest-path runner/Cargo.toml "${engine_override[@]}" --all-targets --all-features -- -D warnings

# --- Tests and docs for both crates. ---
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test --manifest-path runner/Cargo.toml "${engine_override[@]}" --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path runner/Cargo.toml "${engine_override[@]}" --no-deps --all-features

# --- Reference engine and corpora: only needed for the report/benchmark run, so
# prepare them after the gates and tests have passed. ---
timestamp="${RSQJS_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
if [[ -n "${RSQJS_TEST_REPORT_PATH:-}" ]]; then
  report_path="${RSQJS_TEST_REPORT_PATH}"
elif [[ "${RSQJS_TRACKED_REPORT:-0}" == "1" ]]; then
  report_path="reports/test-runs/rsqjs-test-report-${timestamp}.md"
else
  report_path="target/reports/test-runs/rsqjs-test-report-${timestamp}.md"
fi

quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
if [[ -n "${quickjs_path}" ]]; then
  export RSQJS_QUICKJS="${quickjs_path}"
fi

test262_path="$("${script_dir}/prepare-test262.sh")"
if [[ -n "${test262_path}" ]]; then
  export RSQJS_TEST262_DIR="${test262_path}"
fi
export RSQJS_TEST262_RUN_ALL="${RSQJS_TEST262_RUN_ALL:-1}"

# --- Build the engine CLI, then run the report/benchmarks through the runner,
# which drives everything in-process and compares against an embedded QuickJS
# reference behind the `reference-quickjs` feature. ---
cargo build --release --bin rsqjs
cargo run --release --manifest-path runner/Cargo.toml "${engine_override[@]}" --features reference-quickjs -- --report "${report_path}"

printf 'test report: %s\n' "${report_path}"
if [[ "${report_path}" == target/reports/* ]]; then
  printf 'test report is untracked by default; set RSQJS_TRACKED_REPORT=1 for a canonical tracked report\n'
fi
