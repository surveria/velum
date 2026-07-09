#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# The runner lives in `runner/` as a nested workspace and depends on this local
# engine crate through `rs-quickjs = { path = ".." }`.
export RSQJS_BUILD_REPO_ROOT="${RSQJS_BUILD_REPO_ROOT:-${repo_root}}"
export RSQJS_BUILD_COMMIT_SHA="${RSQJS_BUILD_COMMIT_SHA:-$(git rev-parse HEAD)}"

# --- Fast gates: run the cheap checks first so the pipeline stops before it
# compiles anything or downloads corpora. On pull requests and merge groups CI
# sets RSQJS_BASE_REF, which turns on base-relative policy gates.
"${script_dir}/check-touched-file-sizes.sh" "${RSQJS_BASE_REF:-origin/main}"
cargo fmt --all -- --check
cargo fmt --manifest-path runner/Cargo.toml --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --manifest-path runner/Cargo.toml --all-targets --all-features -- -D warnings

# --- Tests and docs for both crates. ---
cargo test --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test --manifest-path runner/Cargo.toml --all-targets --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path runner/Cargo.toml --no-deps --all-features

# --- Reference engine and corpora: only needed for the report/benchmark run, so
# prepare them after the gates and tests have passed. ---
timestamp="${RSQJS_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
if [[ -n "${RSQJS_TEST_REPORT_PATH:-}" ]]; then
  report_path="${RSQJS_TEST_REPORT_PATH}"
elif [[ "${RSQJS_TRACKED_REPORT:-0}" == "1" ]]; then
  report_path="reports/test-runs/rsqjs-test-report-${timestamp}.md"
else
  report_path="target/rsqjs-reports/test-runs/rsqjs-test-report-${timestamp}.md"
fi

report_file="$(basename "${report_path}")"
report_dir="$(dirname "${report_path}")"
reports_root="$(dirname "${report_dir}")"
jetstream_report_file="rsqjs-jetstream-report-${timestamp}.md"
if [[ "${report_path}" == reports/test-runs/* ]]; then
  jetstream_report_path="reports/jetstream-runs/${jetstream_report_file}"
else
  jetstream_report_path="${reports_root}/jetstream-runs/${jetstream_report_file}"
fi
export RSQJS_REPORT_TIMESTAMP="${RSQJS_REPORT_TIMESTAMP:-${timestamp}}"
export RSQJS_REPORT_REPORT_FILE="${RSQJS_REPORT_REPORT_FILE:-${report_file}}"
export RSQJS_REPORT_REPORT_RELATIVE_PATH="${RSQJS_REPORT_REPORT_RELATIVE_PATH:-$(basename "${report_dir}")/${report_file}}"
export RSQJS_JETSTREAM_REPORT_PATH="${RSQJS_JETSTREAM_REPORT_PATH:-${jetstream_report_path}}"
export RSQJS_REPORT_JETSTREAM_REPORT_FILE="${RSQJS_REPORT_JETSTREAM_REPORT_FILE:-${jetstream_report_file}}"
export RSQJS_REPORT_JETSTREAM_REPORT_RELATIVE_PATH="${RSQJS_REPORT_JETSTREAM_REPORT_RELATIVE_PATH:-jetstream-runs/${jetstream_report_file}}"
export RSQJS_REPORT_COMMIT_SHA="${RSQJS_REPORT_COMMIT_SHA:-$(git rev-parse HEAD)}"
export RSQJS_REPORT_TREE_SHA="${RSQJS_REPORT_TREE_SHA:-$(git rev-parse 'HEAD^{tree}')}"
export RSQJS_REPORT_EVENT_NAME="${RSQJS_REPORT_EVENT_NAME:-${GITHUB_EVENT_NAME:-local}}"
export RSQJS_REPORT_RUN_ID="${RSQJS_REPORT_RUN_ID:-${GITHUB_RUN_ID:-}}"
export RSQJS_REPORT_RUN_ATTEMPT="${RSQJS_REPORT_RUN_ATTEMPT:-${GITHUB_RUN_ATTEMPT:-}}"
export RSQJS_REPORT_REPOSITORY="${RSQJS_REPORT_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
export RSQJS_REPORT_WORKFLOW="${RSQJS_REPORT_WORKFLOW:-${GITHUB_WORKFLOW:-}}"

write_metadata_value() {
  local key="$1"
  local value="$2"
  printf '%s=' "${key}"
  printf '%q\n' "${value}"
}

quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
if [[ -n "${quickjs_path}" ]]; then
  export RSQJS_QUICKJS="${quickjs_path}"
fi

test262_path="$("${script_dir}/prepare-test262.sh")"
if [[ -n "${test262_path}" ]]; then
  export RSQJS_TEST262_DIR="${test262_path}"
fi
export RSQJS_TEST262_RUN_ALL="${RSQJS_TEST262_RUN_ALL:-1}"

# --- Run either the required correctness report or the full performance report.
# Correctness keeps the external QuickJS differential check but does not compile
# the embedded QuickJS reference used only by project/JetStream benchmarks. ---
if [[ "${RSQJS_CORRECTNESS_ONLY:-0}" == "1" ]]; then
  cargo run --release --manifest-path runner/Cargo.toml -- --correctness "${report_path}"
else
  cargo run --release --manifest-path runner/Cargo.toml --features reference-quickjs -- --report "${report_path}"
fi

mkdir -p "${reports_root}"
metadata_path="${reports_root}/rsqjs-report-metadata.env"
{
  write_metadata_value 'RSQJS_ARTIFACT_SCHEMA' '1'
  write_metadata_value 'RSQJS_ARTIFACT_REPORT_FILE' "${RSQJS_REPORT_REPORT_FILE}"
  write_metadata_value 'RSQJS_ARTIFACT_REPORT_RELATIVE_PATH' "${RSQJS_REPORT_REPORT_RELATIVE_PATH}"
  write_metadata_value 'RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE' "${RSQJS_REPORT_JETSTREAM_REPORT_FILE}"
  write_metadata_value 'RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH' "${RSQJS_REPORT_JETSTREAM_REPORT_RELATIVE_PATH}"
  write_metadata_value 'RSQJS_ARTIFACT_TIMESTAMP' "${RSQJS_REPORT_TIMESTAMP}"
  write_metadata_value 'RSQJS_ARTIFACT_COMMIT_SHA' "${RSQJS_REPORT_COMMIT_SHA}"
  write_metadata_value 'RSQJS_ARTIFACT_TREE_SHA' "${RSQJS_REPORT_TREE_SHA}"
  write_metadata_value 'RSQJS_ARTIFACT_EVENT_NAME' "${RSQJS_REPORT_EVENT_NAME}"
  write_metadata_value 'RSQJS_ARTIFACT_RUN_ID' "${RSQJS_REPORT_RUN_ID}"
  write_metadata_value 'RSQJS_ARTIFACT_RUN_ATTEMPT' "${RSQJS_REPORT_RUN_ATTEMPT}"
  write_metadata_value 'RSQJS_ARTIFACT_REPOSITORY' "${RSQJS_REPORT_REPOSITORY}"
  write_metadata_value 'RSQJS_ARTIFACT_WORKFLOW' "${RSQJS_REPORT_WORKFLOW}"
  write_metadata_value 'RSQJS_ARTIFACT_PR_NUMBER' "${RSQJS_REPORT_PR_NUMBER:-}"
  write_metadata_value 'RSQJS_ARTIFACT_TASK' "${RSQJS_REPORT_TASK:-}"
} > "${metadata_path}"

if [[ "${report_path}" == target/rsqjs-reports/* ]]; then
  printf 'local/CI report artifact: %s\n' "${report_path}"
  if [[ "${RSQJS_CORRECTNESS_ONLY:-0}" != "1" ]]; then
    printf 'local/CI JetStream report artifact: %s\n' "${RSQJS_JETSTREAM_REPORT_PATH}"
  fi
  printf 'local/CI report artifact root: %s\n' "${reports_root}"
  printf 'report metadata artifact: %s\n' "${metadata_path}"
  printf 'do not commit this report from a feature PR; CI uploads the artifact and the post-merge publisher commits the canonical reports/test-runs copy\n'
else
  printf 'canonical tracked test report: %s\n' "${report_path}"
  if [[ "${RSQJS_CORRECTNESS_ONLY:-0}" != "1" ]]; then
    printf 'canonical tracked JetStream report: %s\n' "${RSQJS_JETSTREAM_REPORT_PATH}"
  fi
  printf 'report metadata: %s\n' "${metadata_path}"
fi
