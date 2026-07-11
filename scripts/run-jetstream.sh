#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

if (($# > 1)); then
  printf 'usage: %s [report-path]\n' "$0" >&2
  exit 2
fi
if [[ "${GITHUB_ACTIONS:-false}" == "true" && "${RSQJS_REPORT_EXHAUSTIVE:-0}" == "1" ]]; then
  printf 'RSQJS_REPORT_EXHAUSTIVE is local-only and cannot run in GitHub Actions\n' >&2
  exit 1
fi

timestamp="${RSQJS_REPORT_TIMESTAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
report_path="${1:-${RSQJS_JETSTREAM_REPORT_PATH:-target/rsqjs-reports/jetstream-runs/rsqjs-jetstream-report-${timestamp}.md}}"
report_file="$(basename "${report_path}")"
report_stem="${report_file%.md}"
report_dir="$(dirname "${report_path}")"
reports_root="$(dirname "${report_dir}")"
metadata_path="${reports_root}/rsqjs-jetstream-metadata.env"
report_yaml_file="${report_stem}.yaml"
report_component_yaml_file="${report_stem}-component.yaml"
report_timing_file="${report_stem}-timings.tsv"

export RSQJS_BUILD_REPO_ROOT="${RSQJS_BUILD_REPO_ROOT:-${repo_root}}"
export RSQJS_BUILD_COMMIT_SHA="${RSQJS_BUILD_COMMIT_SHA:-$(git rev-parse HEAD)}"
export RSQJS_REPORT_TIMESTAMP="${timestamp}"
export RSQJS_REPORT_COMMIT_SHA="${RSQJS_REPORT_COMMIT_SHA:-$(git rev-parse HEAD)}"
export RSQJS_REPORT_TREE_SHA="${RSQJS_REPORT_TREE_SHA:-$(git rev-parse 'HEAD^{tree}')}"
export RSQJS_REPORT_EVENT_NAME="${RSQJS_REPORT_EVENT_NAME:-${GITHUB_EVENT_NAME:-local}}"
export RSQJS_REPORT_RUN_ID="${RSQJS_REPORT_RUN_ID:-${GITHUB_RUN_ID:-}}"
export RSQJS_REPORT_RUN_ATTEMPT="${RSQJS_REPORT_RUN_ATTEMPT:-${GITHUB_RUN_ATTEMPT:-}}"
export RSQJS_REPORT_REPOSITORY="${RSQJS_REPORT_REPOSITORY:-${GITHUB_REPOSITORY:-}}"
export RSQJS_REPORT_WORKFLOW="${RSQJS_REPORT_WORKFLOW:-${GITHUB_WORKFLOW:-}}"
export RSQJS_JETSTREAM_QUICKJS_BASELINE="${RSQJS_JETSTREAM_QUICKJS_BASELINE:-read}"
export RSQJS_JETSTREAM_QUICKJS_BASELINE_PATH="${RSQJS_JETSTREAM_QUICKJS_BASELINE_PATH:-tests/corpora/jetstream/quickjs-baseline.tsv}"

write_metadata_value() {
  local key="$1"
  local value="$2"
  local encoded
  encoded="$(printf '%s' "${value}" | base64 | tr -d '\n')"
  printf '%s=%s\n' "${key}" "${encoded}"
}

mkdir -p "${report_dir}" "${reports_root}"
{
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_SCHEMA' '3'
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPORT_FILE' "${report_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPORT_RELATIVE_PATH' "$(basename "${report_dir}")/${report_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_FILE' "${report_yaml_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_RELATIVE_PATH' "$(basename "${report_dir}")/${report_yaml_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_FILE' "${report_component_yaml_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH' "$(basename "${report_dir}")/${report_component_yaml_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_TIMING_FILE' "${report_timing_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_TIMING_RELATIVE_PATH' "$(basename "${report_dir}")/${report_timing_file}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_TIMESTAMP' "${timestamp}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_COMMIT_SHA' "${RSQJS_REPORT_COMMIT_SHA}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_TREE_SHA' "${RSQJS_REPORT_TREE_SHA}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_EVENT_NAME' "${RSQJS_REPORT_EVENT_NAME}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_RUN_ID' "${RSQJS_REPORT_RUN_ID}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_RUN_ATTEMPT' "${RSQJS_REPORT_RUN_ATTEMPT}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_REPOSITORY' "${RSQJS_REPORT_REPOSITORY}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_WORKFLOW' "${RSQJS_REPORT_WORKFLOW}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_FILTER' "${RSQJS_JETSTREAM_FILTER:-}"
  write_metadata_value 'RSQJS_JETSTREAM_ARTIFACT_BASELINE_MODE' "${RSQJS_JETSTREAM_QUICKJS_BASELINE}"
} > "${metadata_path}"

cargo_features=()
if [[ "${RSQJS_JETSTREAM_QUICKJS_BASELINE}" == "refresh" ]]; then
  cargo_features=(--features reference-quickjs)
fi

cargo run --release --manifest-path runner/Cargo.toml "${cargo_features[@]}" -- --jetstream "${report_path}"

printf 'JetStream report artifact: %s\n' "${report_path}"
printf 'JetStream YAML summary artifact: %s/%s\n' "${report_dir}" "${report_yaml_file}"
printf 'JetStream bounded component artifact: %s/%s\n' "${report_dir}" "${report_component_yaml_file}"
printf 'JetStream bounded timing artifact: %s/%s\n' "${report_dir}" "${report_timing_file}"
printf 'JetStream metadata artifact: %s\n' "${metadata_path}"
printf 'JetStream QuickJS baseline mode: %s\n' "${RSQJS_JETSTREAM_QUICKJS_BASELINE}"
