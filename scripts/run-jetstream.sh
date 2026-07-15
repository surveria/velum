#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

if (($# > 1)); then
  printf 'usage: %s [report-path]\n' "$0" >&2
  exit 2
fi
if [[ "${GITHUB_ACTIONS:-false}" == "true" && "${VELUM_REPORT_EXHAUSTIVE:-0}" == "1" ]]; then
  printf 'VELUM_REPORT_EXHAUSTIVE is local-only and cannot run in GitHub Actions\n' >&2
  exit 1
fi

timestamp="${VELUM_REPORT_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${1:-${VELUM_JETSTREAM_REPORT_PATH:-target/velum-reports/jetstream-runs/velum-jetstream-report-${timestamp}.md}}"
report_file="$(basename "${report_path}")"
report_stem="${report_file%.md}"
report_dir="$(dirname "${report_path}")"
reports_root="$(dirname "${report_dir}")"
metadata_path="${reports_root}/velum-jetstream-metadata.env"
report_yaml_file="${report_stem}.yaml"
report_component_yaml_file="${report_stem}-component.yaml"
report_timing_file="${report_stem}-timings.tsv"

export VELUM_BUILD_REPO_ROOT="${VELUM_BUILD_REPO_ROOT:-${repo_root}}"
export VELUM_BUILD_COMMIT_SHA="${VELUM_BUILD_COMMIT_SHA:-$(git rev-parse HEAD)}"
export VELUM_REPORT_TIMESTAMP="${timestamp}"
export VELUM_REPORT_COMMIT_SHA="${VELUM_REPORT_COMMIT_SHA:-$(git rev-parse HEAD)}"
export VELUM_REPORT_TREE_SHA="${VELUM_REPORT_TREE_SHA:-$(git rev-parse 'HEAD^{tree}')}"
export VELUM_REPORT_EVENT_NAME="${VELUM_REPORT_EVENT_NAME:-${GITHUB_EVENT_NAME:-local}}"
export VELUM_REPORT_RUN_ID="${VELUM_REPORT_RUN_ID:-${GITHUB_RUN_ID:-}}"
export VELUM_REPORT_RUN_ATTEMPT="${VELUM_REPORT_RUN_ATTEMPT:-${GITHUB_RUN_ATTEMPT:-}}"
export VELUM_REPORT_REPOSITORY="${VELUM_REPORT_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
export VELUM_REPORT_WORKFLOW="${VELUM_REPORT_WORKFLOW:-${GITHUB_WORKFLOW:-}}"
export VELUM_JETSTREAM_QUICKJS_BASELINE="${VELUM_JETSTREAM_QUICKJS_BASELINE:-read}"
export VELUM_JETSTREAM_QUICKJS_BASELINE_PATH="${VELUM_JETSTREAM_QUICKJS_BASELINE_PATH:-tests/corpora/jetstream/quickjs-baseline.tsv}"

write_metadata_value() {
  local key="$1"
  local value="$2"
  local encoded
  encoded="$(printf '%s' "${value}" | base64 | tr -d '\n')"
  printf '%s=%s\n' "${key}" "${encoded}"
}

mkdir -p "${report_dir}" "${reports_root}"
{
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_SCHEMA' '3'
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPORT_FILE' "${report_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPORT_RELATIVE_PATH' "$(basename "${report_dir}")/${report_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPORT_YAML_FILE' "${report_yaml_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPORT_YAML_RELATIVE_PATH' "$(basename "${report_dir}")/${report_yaml_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_FILE' "${report_component_yaml_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH' "$(basename "${report_dir}")/${report_component_yaml_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_TIMING_FILE' "${report_timing_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_TIMING_RELATIVE_PATH' "$(basename "${report_dir}")/${report_timing_file}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_TIMESTAMP' "${timestamp}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_COMMIT_SHA' "${VELUM_REPORT_COMMIT_SHA}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_TREE_SHA' "${VELUM_REPORT_TREE_SHA}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_EVENT_NAME' "${VELUM_REPORT_EVENT_NAME}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_RUN_ID' "${VELUM_REPORT_RUN_ID}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_RUN_ATTEMPT' "${VELUM_REPORT_RUN_ATTEMPT}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_REPOSITORY' "${VELUM_REPORT_REPOSITORY}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_WORKFLOW' "${VELUM_REPORT_WORKFLOW}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_FILTER' "${VELUM_JETSTREAM_FILTER:-}"
  write_metadata_value 'VELUM_JETSTREAM_ARTIFACT_BASELINE_MODE' "${VELUM_JETSTREAM_QUICKJS_BASELINE}"
} > "${metadata_path}"

cargo_features=()
if [[ "${VELUM_JETSTREAM_QUICKJS_BASELINE}" == "refresh" ]]; then
  cargo_features=(--features reference-quickjs)
fi

cargo run --release --manifest-path runner/Cargo.toml "${cargo_features[@]}" -- --jetstream "${report_path}"

printf 'JetStream report artifact: %s\n' "${report_path}"
printf 'JetStream YAML summary artifact: %s/%s\n' "${report_dir}" "${report_yaml_file}"
printf 'JetStream bounded component artifact: %s/%s\n' "${report_dir}" "${report_component_yaml_file}"
printf 'JetStream bounded timing artifact: %s/%s\n' "${report_dir}" "${report_timing_file}"
printf 'JetStream metadata artifact: %s\n' "${metadata_path}"
printf 'JetStream QuickJS baseline mode: %s\n' "${VELUM_JETSTREAM_QUICKJS_BASELINE}"
