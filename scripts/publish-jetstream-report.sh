#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

fail() {
  printf 'publish-jetstream-report: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

valid_report_file() {
  [[ "$1" =~ ^rsqjs-jetstream-report-[0-9]{8}T[0-9]{6}Z\.md$ ]]
}

valid_report_yaml_file() {
  [[ "$1" =~ ^rsqjs-jetstream-report-[0-9]{8}T[0-9]{6}Z\.yaml$ ]]
}

valid_component_yaml_file() {
  [[ "$1" =~ ^rsqjs-jetstream-report-[0-9]{8}T[0-9]{6}Z-component\.yaml$ ]]
}

valid_timing_file() {
  [[ "$1" =~ ^rsqjs-jetstream-report-[0-9]{8}T[0-9]{6}Z-timings\.tsv$ ]]
}

valid_metadata_key() {
  case "$1" in
    RSQJS_JETSTREAM_ARTIFACT_SCHEMA | \
      RSQJS_JETSTREAM_ARTIFACT_REPORT_FILE | RSQJS_JETSTREAM_ARTIFACT_REPORT_RELATIVE_PATH | \
      RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_FILE | RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_RELATIVE_PATH | \
      RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_FILE | RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH | \
      RSQJS_JETSTREAM_ARTIFACT_TIMING_FILE | RSQJS_JETSTREAM_ARTIFACT_TIMING_RELATIVE_PATH | \
      RSQJS_JETSTREAM_ARTIFACT_TIMESTAMP | RSQJS_JETSTREAM_ARTIFACT_COMMIT_SHA | \
      RSQJS_JETSTREAM_ARTIFACT_TREE_SHA | RSQJS_JETSTREAM_ARTIFACT_EVENT_NAME | \
      RSQJS_JETSTREAM_ARTIFACT_RUN_ID | RSQJS_JETSTREAM_ARTIFACT_RUN_ATTEMPT | \
      RSQJS_JETSTREAM_ARTIFACT_REPOSITORY | RSQJS_JETSTREAM_ARTIFACT_WORKFLOW | \
      RSQJS_JETSTREAM_ARTIFACT_FILTER | RSQJS_JETSTREAM_ARTIFACT_BASELINE_MODE)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

clear_metadata() {
  unset RSQJS_JETSTREAM_ARTIFACT_SCHEMA
  unset RSQJS_JETSTREAM_ARTIFACT_REPORT_FILE RSQJS_JETSTREAM_ARTIFACT_REPORT_RELATIVE_PATH
  unset RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_FILE RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_RELATIVE_PATH
  unset RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_FILE RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH
  unset RSQJS_JETSTREAM_ARTIFACT_TIMING_FILE RSQJS_JETSTREAM_ARTIFACT_TIMING_RELATIVE_PATH
  unset RSQJS_JETSTREAM_ARTIFACT_TIMESTAMP RSQJS_JETSTREAM_ARTIFACT_COMMIT_SHA
  unset RSQJS_JETSTREAM_ARTIFACT_TREE_SHA RSQJS_JETSTREAM_ARTIFACT_EVENT_NAME
  unset RSQJS_JETSTREAM_ARTIFACT_RUN_ID RSQJS_JETSTREAM_ARTIFACT_RUN_ATTEMPT
  unset RSQJS_JETSTREAM_ARTIFACT_REPOSITORY RSQJS_JETSTREAM_ARTIFACT_WORKFLOW
  unset RSQJS_JETSTREAM_ARTIFACT_FILTER RSQJS_JETSTREAM_ARTIFACT_BASELINE_MODE
}

read_metadata() {
  local metadata_file="$1"
  [[ -f "${metadata_file}" && ! -L "${metadata_file}" ]] || return 1
  clear_metadata
  local -A seen=()
  local line key encoded decoded
  while IFS= read -r line || [[ -n "${line}" ]]; do
    if [[ ! "${line}" =~ ^([A-Z0-9_]+)=([A-Za-z0-9+/]*={0,2})$ ]]; then
      return 1
    fi
    key="${BASH_REMATCH[1]}"
    encoded="${BASH_REMATCH[2]}"
    valid_metadata_key "${key}" || return 1
    [[ -z "${seen[${key}]:-}" ]] || return 1
    decoded="$(printf '%s' "${encoded}" | base64 --decode 2>/dev/null)" || return 1
    seen["${key}"]=1
    printf -v "${key}" '%s' "${decoded}"
  done < "${metadata_file}"
}

checkout_latest_main() {
  git fetch --no-tags origin main
  if git show-ref --verify --quiet refs/heads/main; then
    git checkout main
    git merge --ff-only origin/main
  else
    git checkout -b main origin/main
  fi
}

stage_outputs() {
  local source_report="$1"
  local source_yaml="$2"
  local report_file="$3"
  local report_yaml_file="$4"

  mkdir -p reports/jetstream-runs
  target_report="reports/jetstream-runs/${report_file}"
  if [[ -f "${target_report}" ]] && ! cmp -s "${source_report}" "${target_report}"; then
    fail "tracked JetStream report already exists with different content: ${target_report}"
  fi
  cp "${source_report}" "${target_report}"
  target_yaml="reports/jetstream-runs/${report_yaml_file}"
  if [[ -f "${target_yaml}" ]] && ! cmp -s "${source_yaml}" "${target_yaml}"; then
    fail "tracked JetStream YAML already exists with different content: ${target_yaml}"
  fi
  cp "${source_yaml}" "${target_yaml}"
  cargo run --manifest-path runner/Cargo.toml -- --aggregate-reports reports/test-runs
}

create_commit_payload() {
  local payload_path="$1"
  local repository="$2"
  local expected_head="$3"
  local headline="$4"
  local body="$5"
  shift 5

  python3 - "${payload_path}" "${repository}" "${expected_head}" "${headline}" "${body}" "$@" <<'PY'
import base64
import json
import pathlib
import sys

payload_path, repository, expected_head, headline, body, *paths = sys.argv[1:]
additions = []
for path in paths:
    additions.append({
        "path": path,
        "contents": base64.b64encode(pathlib.Path(path).read_bytes()).decode("ascii"),
    })

query = """
mutation($input: CreateCommitOnBranchInput!) {
  createCommitOnBranch(input: $input) {
    commit { oid url }
  }
}
"""
payload = {
    "query": query,
    "variables": {
        "input": {
            "branch": {
                "repositoryNameWithOwner": repository,
                "branchName": "main",
            },
            "expectedHeadOid": expected_head,
            "message": {"headline": headline, "body": body},
            "fileChanges": {"additions": additions},
        },
    },
}
pathlib.Path(payload_path).write_text(json.dumps(payload), encoding="utf-8")
PY
}

create_signed_commit() {
  local repository="$1"
  local headline="$2"
  local body="$3"
  shift 3

  local expected_head payload_path commit_oid
  expected_head="$(git rev-parse HEAD)"
  payload_path="$(mktemp)"
  create_commit_payload "${payload_path}" "${repository}" "${expected_head}" "${headline}" "${body}" "$@"
  if ! commit_oid="$(gh api graphql --input "${payload_path}" --jq '.data.createCommitOnBranch.commit.oid')"; then
    rm -f "${payload_path}"
    return 1
  fi
  rm -f "${payload_path}"
  [[ -n "${commit_oid}" ]] || return 1
  printf 'signed GitHub JetStream report commit: %s\n' "${commit_oid}"
}

reset_outputs() {
  local report_path="$1"
  local yaml_path="$2"
  if git ls-files --error-unmatch "${report_path}" >/dev/null 2>&1; then
    git restore --worktree -- "${report_path}"
  else
    rm -f "${report_path}"
  fi
  if git ls-files --error-unmatch "${yaml_path}" >/dev/null 2>&1; then
    git restore --worktree -- "${yaml_path}"
  else
    rm -f "${yaml_path}"
  fi
  git restore --worktree -- reports/benchmark-rollup.md reports/benchmark-summary.jpg
}

need_cmd cargo
need_cmd gh
need_cmd git
need_cmd python3

repository="${GITHUB_REPOSITORY:-}"
[[ -n "${repository}" ]] || fail "GITHUB_REPOSITORY is required"
metadata_path="${RSQJS_JETSTREAM_METADATA_PATH:-target/rsqjs-reports/rsqjs-jetstream-metadata.env}"
[[ -f "${metadata_path}" ]] || fail "missing JetStream metadata: ${metadata_path}"

read_metadata "${metadata_path}" || fail "invalid JetStream metadata"
[[ "${RSQJS_JETSTREAM_ARTIFACT_SCHEMA:-}" == "3" ]] || fail "unsupported artifact schema"
report_file="${RSQJS_JETSTREAM_ARTIFACT_REPORT_FILE:-}"
report_relative_path="${RSQJS_JETSTREAM_ARTIFACT_REPORT_RELATIVE_PATH:-}"
report_yaml_file="${RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_FILE:-}"
report_yaml_relative_path="${RSQJS_JETSTREAM_ARTIFACT_REPORT_YAML_RELATIVE_PATH:-}"
component_yaml_file="${RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_FILE:-}"
component_yaml_relative_path="${RSQJS_JETSTREAM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH:-}"
timing_file="${RSQJS_JETSTREAM_ARTIFACT_TIMING_FILE:-}"
timing_relative_path="${RSQJS_JETSTREAM_ARTIFACT_TIMING_RELATIVE_PATH:-}"
expected_commit="${RSQJS_JETSTREAM_ARTIFACT_COMMIT_SHA:-}"
expected_tree="${RSQJS_JETSTREAM_ARTIFACT_TREE_SHA:-}"
source_run="${RSQJS_JETSTREAM_ARTIFACT_RUN_ID:-unknown}"
[[ "${RSQJS_JETSTREAM_ARTIFACT_FILTER:-}" == "" ]] || fail "filtered reports are not canonical"
[[ "${RSQJS_JETSTREAM_ARTIFACT_BASELINE_MODE:-}" == "read" ]] || fail "only read-only baseline runs are canonical"
[[ "${RSQJS_JETSTREAM_ARTIFACT_REPOSITORY:-}" == "${GITHUB_REPOSITORY:-}" ]] || fail "artifact repository mismatch"
[[ "${RSQJS_JETSTREAM_ARTIFACT_WORKFLOW:-}" == "${GITHUB_WORKFLOW:-}" ]] || fail "artifact workflow mismatch"
[[ "${RSQJS_JETSTREAM_ARTIFACT_RUN_ID:-}" == "${GITHUB_RUN_ID:-}" ]] || fail "artifact run id mismatch"
[[ "${RSQJS_JETSTREAM_ARTIFACT_RUN_ATTEMPT:-}" == "${GITHUB_RUN_ATTEMPT:-}" ]] || fail "artifact run attempt mismatch"
[[ "${RSQJS_JETSTREAM_ARTIFACT_EVENT_NAME:-}" == "${GITHUB_EVENT_NAME:-}" ]] || fail "artifact event mismatch"
case "${RSQJS_JETSTREAM_ARTIFACT_EVENT_NAME:-}" in
  schedule|workflow_dispatch)
    ;;
  *)
    fail "unsupported JetStream publish event"
    ;;
esac
[[ -n "${RSQJS_DEFAULT_BRANCH:-}" ]] || fail "default branch is required"
[[ "${GITHUB_REF:-}" == "refs/heads/${RSQJS_DEFAULT_BRANCH}" ]] || fail "canonical publish requires the default branch ref"
[[ "${expected_commit}" == "${GITHUB_SHA:-}" ]] || fail "artifact commit does not match the workflow commit"
current_head="$(git rev-parse HEAD)"
[[ "${current_head}" == "${expected_commit}" ]] || fail "artifact commit does not match the checked-out HEAD"
valid_report_file "${report_file}" || fail "invalid JetStream report file name: ${report_file}"
valid_report_yaml_file "${report_yaml_file}" || fail "invalid JetStream YAML file name: ${report_yaml_file}"
valid_component_yaml_file "${component_yaml_file}" || fail "invalid JetStream component file name: ${component_yaml_file}"
valid_timing_file "${timing_file}" || fail "invalid JetStream timing file name: ${timing_file}"
[[ "${report_yaml_file}" == "${report_file%.md}.yaml" ]] || fail "JetStream YAML name does not match Markdown"
[[ "${component_yaml_file}" == "${report_file%.md}-component.yaml" ]] || fail "JetStream component name does not match Markdown"
[[ "${timing_file}" == "${report_file%.md}-timings.tsv" ]] || fail "JetStream timing name does not match Markdown"
[[ "${report_file}" == "rsqjs-jetstream-report-${RSQJS_JETSTREAM_ARTIFACT_TIMESTAMP:-}.md" ]] || fail "JetStream report timestamp mismatch"
[[ "${report_relative_path}" == "jetstream-runs/${report_file}" ]] || fail "invalid JetStream report relative path"
[[ "${report_yaml_relative_path}" == "jetstream-runs/${report_yaml_file}" ]] || fail "invalid JetStream YAML relative path"
[[ "${component_yaml_relative_path}" == "jetstream-runs/${component_yaml_file}" ]] || fail "invalid JetStream component relative path"
[[ "${timing_relative_path}" == "jetstream-runs/${timing_file}" ]] || fail "invalid JetStream timing relative path"
[[ "${expected_commit}" =~ ^[0-9a-f]{40}$ ]] || fail "invalid tested commit"
[[ "${expected_tree}" =~ ^[0-9a-f]{40}$ ]] || fail "invalid tested tree"
git cat-file -e "${expected_commit}^{commit}" || fail "tested commit is unavailable"
actual_tree="$(git rev-parse "${expected_commit}^{tree}")"
[[ "${actual_tree}" == "${expected_tree}" ]] || fail "tested commit tree mismatch"

metadata_dir="$(dirname "${metadata_path}")"
source_report="${metadata_dir}/${report_relative_path}"
source_yaml="${metadata_dir}/${report_yaml_relative_path}"
source_component="${metadata_dir}/${component_yaml_relative_path}"
source_timing="${metadata_dir}/${timing_relative_path}"
[[ -f "${source_report}" && ! -L "${source_report}" ]] || fail "JetStream report is absent: ${source_report}"
[[ -f "${source_yaml}" && ! -L "${source_yaml}" ]] || fail "JetStream YAML is absent: ${source_yaml}"
[[ -f "${source_component}" && ! -L "${source_component}" ]] || fail "JetStream component is absent: ${source_component}"
[[ -f "${source_timing}" && ! -L "${source_timing}" ]] || fail "JetStream timing is absent: ${source_timing}"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT
source_copy="${tmp_dir}/${report_file}"
source_yaml_copy="${tmp_dir}/${report_yaml_file}"
cp "${source_report}" "${source_copy}"
cp "${source_yaml}" "${source_yaml_copy}"

if [[ -n "${RSQJS_DEPENDENCY_TOKEN:-}" ]]; then
  export GIT_CONFIG_COUNT=1
  export GIT_CONFIG_KEY_0="url.https://x-access-token:${RSQJS_DEPENDENCY_TOKEN}@github.com/chertov/.insteadOf"
  export GIT_CONFIG_VALUE_0="https://github.com/chertov/"
fi

checkout_latest_main
stage_outputs "${source_copy}" "${source_yaml_copy}" "${report_file}" "${report_yaml_file}"
timestamp="${report_file#rsqjs-jetstream-report-}"
timestamp="${timestamp%.md}"
headline="Add JetStream report ${timestamp} [skip ci]"
body="$(printf 'Source commit: %s\n\nSource tree: %s\n\nSource workflow run: %s\n' \
  "${expected_commit}" "${expected_tree}" "${source_run}")"
commit_paths=("${target_report}" "${target_yaml}" reports/benchmark-rollup.md reports/benchmark-summary.jpg)

if create_signed_commit "${repository}" "${headline}" "${body}" "${commit_paths[@]}"; then
  exit 0
fi

printf 'initial signed JetStream report commit failed; retrying once on latest origin/main\n' >&2
reset_outputs "${target_report}" "${target_yaml}"
checkout_latest_main
stage_outputs "${source_copy}" "${source_yaml_copy}" "${report_file}" "${report_yaml_file}"
create_signed_commit "${repository}" "${headline}" "${body}" "${commit_paths[@]}"
