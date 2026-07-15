#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# The runner lives in `runner/` as a nested workspace and depends on this local
# engine crate through `velum = { path = ".." }`.
export VELUM_BUILD_REPO_ROOT="${VELUM_BUILD_REPO_ROOT:-${repo_root}}"
export VELUM_BUILD_COMMIT_SHA="${VELUM_BUILD_COMMIT_SHA:-$(git rev-parse HEAD)}"
export VELUM_BENCH_SET="${VELUM_BENCH_SET:-sentinel}"
export VELUM_JETSTREAM_ENABLED="${VELUM_JETSTREAM_ENABLED:-0}"

if [[ "${VELUM_CORRECTNESS_ONLY:-0}" == "1" && "${VELUM_PERFORMANCE_ONLY:-0}" == "1" ]]; then
  printf 'correctness-only and performance-only modes are mutually exclusive\n' >&2
  exit 1
fi
if [[ "${VELUM_PERFORMANCE_ONLY:-0}" == "1" ]]; then
  report_mode=performance
elif [[ "${VELUM_CORRECTNESS_ONLY:-0}" == "1" ]]; then
  report_mode=correctness
else
  report_mode=full
fi
if [[ "${GITHUB_ACTIONS:-false}" == "true" && "${VELUM_REPORT_EXHAUSTIVE:-0}" == "1" ]]; then
  printf 'VELUM_REPORT_EXHAUSTIVE is local-only and cannot run in GitHub Actions\n' >&2
  exit 1
fi

# Post-merge performance collection reuses the exact tree that already passed
# the required correctness gate. The runner locks only measured benchmark
# execution; preparation and compilation remain outside the exclusive slot.
if [[ "${VELUM_PERFORMANCE_ONLY:-0}" != "1" ]]; then
  # --- Fast gates: run the cheap checks first so the pipeline stops before it
  # compiles anything or downloads corpora. On pull requests and merge groups CI
  # sets VELUM_BASE_REF, which turns on base-relative policy gates.
  "${script_dir}/check-vendored-regress.sh"
  "${script_dir}/check-touched-file-sizes.sh" "${VELUM_BASE_REF:-origin/main}"
  "${script_dir}/check-architecture-boundaries.sh" --self-test
  "${script_dir}/test-report-artifact-metadata.sh"
  "${script_dir}/test-jetstream-artifact-metadata.sh"
  cargo fmt --all -- --check
  cargo fmt --manifest-path runner/Cargo.toml --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo clippy --manifest-path runner/Cargo.toml --all-targets --all-features -- -D warnings

  # --- Tests and docs for both crates. ---
  cargo test --all-targets --all-features
  RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
  cargo test --manifest-path runner/Cargo.toml --all-targets --all-features
  RUSTDOCFLAGS="-D warnings" cargo doc --manifest-path runner/Cargo.toml --no-deps --all-features
fi

# --- Reference engine and corpora: only needed for the report/benchmark run, so
# prepare them after the gates and tests have passed. ---
timestamp="${VELUM_TEST_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
if [[ -n "${VELUM_TEST_REPORT_PATH:-}" ]]; then
  report_path="${VELUM_TEST_REPORT_PATH}"
elif [[ "${VELUM_TRACKED_REPORT:-0}" == "1" ]]; then
  report_path="reports/test-runs/velum-test-report-${timestamp}.md"
else
  report_path="target/velum-reports/test-runs/velum-test-report-${timestamp}.md"
fi

report_file="$(basename "${report_path}")"
report_dir="$(dirname "${report_path}")"
reports_root="$(dirname "${report_dir}")"
report_stem="${report_file%.md}"
report_yaml_file="${report_stem}.yaml"
report_component_yaml_file="${report_stem}-component.yaml"
report_exhaustive_yaml_file="${report_stem}-exhaustive.yaml"
report_yaml_path="${report_dir}/${report_yaml_file}"
report_component_yaml_path="${report_dir}/${report_component_yaml_file}"
report_exhaustive_yaml_path="${report_dir}/${report_exhaustive_yaml_file}"
export VELUM_TEST262_PASS_CANDIDATE_PATH="${VELUM_TEST262_PASS_CANDIDATE_PATH:-${reports_root}/test262-pass-baseline.txt}"
jetstream_report_file="velum-jetstream-report-${timestamp}.md"
if [[ "${report_path}" == reports/test-runs/* ]]; then
  jetstream_report_path="reports/jetstream-runs/${jetstream_report_file}"
else
  jetstream_report_path="${reports_root}/jetstream-runs/${jetstream_report_file}"
fi
export VELUM_REPORT_TIMESTAMP="${VELUM_REPORT_TIMESTAMP:-${timestamp}}"
export VELUM_REPORT_REPORT_FILE="${VELUM_REPORT_REPORT_FILE:-${report_file}}"
export VELUM_REPORT_REPORT_RELATIVE_PATH="${VELUM_REPORT_REPORT_RELATIVE_PATH:-$(basename "${report_dir}")/${report_file}}"
export VELUM_REPORT_YAML_FILE="${report_yaml_file}"
export VELUM_REPORT_YAML_RELATIVE_PATH="$(basename "${report_dir}")/${report_yaml_file}"
export VELUM_REPORT_COMPONENT_YAML_FILE="${report_component_yaml_file}"
export VELUM_REPORT_COMPONENT_YAML_RELATIVE_PATH="$(basename "${report_dir}")/${report_component_yaml_file}"
export VELUM_JETSTREAM_REPORT_PATH="${VELUM_JETSTREAM_REPORT_PATH:-${jetstream_report_path}}"
export VELUM_REPORT_JETSTREAM_REPORT_FILE="${VELUM_REPORT_JETSTREAM_REPORT_FILE:-${jetstream_report_file}}"
export VELUM_REPORT_JETSTREAM_REPORT_RELATIVE_PATH="${VELUM_REPORT_JETSTREAM_REPORT_RELATIVE_PATH:-jetstream-runs/${jetstream_report_file}}"
export VELUM_REPORT_COMMIT_SHA="${VELUM_REPORT_COMMIT_SHA:-$(git rev-parse HEAD)}"
export VELUM_REPORT_TREE_SHA="${VELUM_REPORT_TREE_SHA:-$(git rev-parse 'HEAD^{tree}')}"
export VELUM_REPORT_EVENT_NAME="${VELUM_REPORT_EVENT_NAME:-${GITHUB_EVENT_NAME:-local}}"
export VELUM_REPORT_RUN_ID="${VELUM_REPORT_RUN_ID:-${GITHUB_RUN_ID:-}}"
export VELUM_REPORT_RUN_ATTEMPT="${VELUM_REPORT_RUN_ATTEMPT:-${GITHUB_RUN_ATTEMPT:-}}"
export VELUM_REPORT_REPOSITORY="${VELUM_REPORT_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
export VELUM_REPORT_WORKFLOW="${VELUM_REPORT_WORKFLOW:-${GITHUB_WORKFLOW:-}}"
case "${VELUM_JETSTREAM_ENABLED:-0}" in
  0|false|FALSE|no|NO)
    jetstream_enabled=0
    ;;
  *)
    jetstream_enabled=1
    ;;
esac

write_metadata_value() {
  local key="$1"
  local value="$2"
  local encoded
  encoded="$(printf '%s' "${value}" | base64 | tr -d '\n')"
  printf '%s=%s\n' "${key}" "${encoded}"
}

# --- Run either the required correctness report or the full performance report.
# Correctness keeps the external QuickJS differential check but does not compile
# the embedded QuickJS reference used only by project/JetStream benchmarks. ---
if [[ "${report_mode}" == performance ]]; then
  cargo run --release --manifest-path runner/Cargo.toml -- --performance "${report_path}"
else
  quickjs_path="$("${script_dir}/prepare-quickjs.sh")"
  if [[ -n "${quickjs_path}" ]]; then
    export VELUM_QUICKJS="${quickjs_path}"
  fi

  test262_path="$("${script_dir}/prepare-test262.sh")"
  if [[ -n "${test262_path}" ]]; then
    export VELUM_TEST262_DIR="${test262_path}"
  fi
  export VELUM_TEST262_RUN_ALL="${VELUM_TEST262_RUN_ALL:-1}"
fi

if [[ "${report_mode}" == correctness ]]; then
  cargo run --release --manifest-path runner/Cargo.toml -- --correctness "${report_path}"
elif [[ "${report_mode}" == full ]]; then
  cargo run --release --manifest-path runner/Cargo.toml --features reference-quickjs -- --report "${report_path}"
fi

[[ -f "${report_yaml_path}" ]] || {
  printf 'missing structured YAML report summary: %s\n' "${report_yaml_path}" >&2
  exit 1
}
[[ -f "${report_component_yaml_path}" ]] || {
  printf 'missing bounded YAML composition source: %s\n' "${report_component_yaml_path}" >&2
  exit 1
}
if [[ "${VELUM_REPORT_EXHAUSTIVE:-0}" == "1" && ! -f "${report_exhaustive_yaml_path}" ]]; then
  printf 'missing requested exhaustive YAML report: %s\n' "${report_exhaustive_yaml_path}" >&2
  exit 1
fi

mkdir -p "${reports_root}"
metadata_path="${reports_root}/velum-report-metadata.env"
{
  write_metadata_value 'VELUM_ARTIFACT_SCHEMA' '3'
  write_metadata_value 'VELUM_ARTIFACT_REPORT_MODE' "${report_mode}"
  write_metadata_value 'VELUM_ARTIFACT_REPORT_FILE' "${VELUM_REPORT_REPORT_FILE}"
  write_metadata_value 'VELUM_ARTIFACT_REPORT_RELATIVE_PATH' "${VELUM_REPORT_REPORT_RELATIVE_PATH}"
  write_metadata_value 'VELUM_ARTIFACT_REPORT_YAML_FILE' "${VELUM_REPORT_YAML_FILE}"
  write_metadata_value 'VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH' "${VELUM_REPORT_YAML_RELATIVE_PATH}"
  write_metadata_value 'VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE' "${VELUM_REPORT_COMPONENT_YAML_FILE}"
  write_metadata_value 'VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH' "${VELUM_REPORT_COMPONENT_YAML_RELATIVE_PATH}"
  if [[ "${report_mode}" == full && "${jetstream_enabled}" != "0" ]]; then
    write_metadata_value 'VELUM_ARTIFACT_JETSTREAM_REPORT_FILE' "${VELUM_REPORT_JETSTREAM_REPORT_FILE}"
    write_metadata_value 'VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH' "${VELUM_REPORT_JETSTREAM_REPORT_RELATIVE_PATH}"
  fi
  write_metadata_value 'VELUM_ARTIFACT_TIMESTAMP' "${VELUM_REPORT_TIMESTAMP}"
  write_metadata_value 'VELUM_ARTIFACT_COMMIT_SHA' "${VELUM_REPORT_COMMIT_SHA}"
  write_metadata_value 'VELUM_ARTIFACT_TREE_SHA' "${VELUM_REPORT_TREE_SHA}"
  write_metadata_value 'VELUM_ARTIFACT_EVENT_NAME' "${VELUM_REPORT_EVENT_NAME}"
  write_metadata_value 'VELUM_ARTIFACT_RUN_ID' "${VELUM_REPORT_RUN_ID}"
  write_metadata_value 'VELUM_ARTIFACT_RUN_ATTEMPT' "${VELUM_REPORT_RUN_ATTEMPT}"
  write_metadata_value 'VELUM_ARTIFACT_REPOSITORY' "${VELUM_REPORT_REPOSITORY}"
  write_metadata_value 'VELUM_ARTIFACT_WORKFLOW' "${VELUM_REPORT_WORKFLOW}"
  write_metadata_value 'VELUM_ARTIFACT_PR_NUMBER' "${VELUM_REPORT_PR_NUMBER:-}"
  write_metadata_value 'VELUM_ARTIFACT_TASK' "${VELUM_REPORT_TASK:-}"
} > "${metadata_path}"

if [[ "${report_path}" == target/velum-reports/* ]]; then
  printf 'local/CI report artifact: %s\n' "${report_path}"
  printf 'local/CI structured YAML summary: %s\n' "${report_yaml_path}"
  printf 'local/CI bounded YAML composition source: %s\n' "${report_component_yaml_path}"
  if [[ "${VELUM_REPORT_EXHAUSTIVE:-0}" == "1" ]]; then
    printf 'local/CI exhaustive YAML artifact: %s\n' "${report_exhaustive_yaml_path}"
  fi
  if [[ "${report_mode}" == full && "${jetstream_enabled}" != "0" ]]; then
    printf 'local/CI JetStream report artifact: %s\n' "${VELUM_JETSTREAM_REPORT_PATH}"
  fi
  printf 'local/CI report artifact root: %s\n' "${reports_root}"
  printf 'report metadata artifact: %s\n' "${metadata_path}"
  printf 'do not commit this report from a feature PR; CI uploads the artifact and the post-merge publisher commits the canonical reports/test-runs copy\n'
else
  printf 'canonical tracked test report: %s\n' "${report_path}"
  printf 'canonical tracked structured YAML summary: %s\n' "${report_yaml_path}"
  printf 'bounded YAML composition source: %s\n' "${report_component_yaml_path}"
  if [[ "${VELUM_REPORT_EXHAUSTIVE:-0}" == "1" ]]; then
    printf 'untracked exhaustive YAML artifact: %s\n' "${report_exhaustive_yaml_path}"
  fi
  if [[ "${report_mode}" == full && "${jetstream_enabled}" != "0" ]]; then
    printf 'canonical tracked JetStream report: %s\n' "${VELUM_JETSTREAM_REPORT_PATH}"
  fi
  printf 'report metadata: %s\n' "${metadata_path}"
fi
