#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

default_tested_source_archive_ref="refs/rsqjs/ci-tested-sources"
default_legacy_tested_source_archive_ref="refs/heads/ci-tested-sources"
tested_source_archive_local_branch="rsqjs-tested-source-archive"

fail() {
  printf 'publish-report-artifact: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

clear_metadata() {
  unset RSQJS_ARTIFACT_SCHEMA RSQJS_ARTIFACT_REPORT_MODE
  unset RSQJS_ARTIFACT_REPORT_FILE RSQJS_ARTIFACT_REPORT_RELATIVE_PATH
  unset RSQJS_ARTIFACT_REPORT_YAML_FILE RSQJS_ARTIFACT_REPORT_YAML_RELATIVE_PATH
  unset RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH
  unset RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH
  unset RSQJS_ARTIFACT_TIMESTAMP RSQJS_ARTIFACT_COMMIT_SHA RSQJS_ARTIFACT_TREE_SHA
  unset RSQJS_ARTIFACT_EVENT_NAME RSQJS_ARTIFACT_RUN_ID RSQJS_ARTIFACT_RUN_ATTEMPT
  unset RSQJS_ARTIFACT_REPOSITORY RSQJS_ARTIFACT_WORKFLOW
  unset RSQJS_ARTIFACT_PR_NUMBER RSQJS_ARTIFACT_TASK
}

valid_metadata_key() {
  case "$1" in
    RSQJS_ARTIFACT_SCHEMA | RSQJS_ARTIFACT_REPORT_MODE | \
      RSQJS_ARTIFACT_REPORT_FILE | RSQJS_ARTIFACT_REPORT_RELATIVE_PATH | \
      RSQJS_ARTIFACT_REPORT_YAML_FILE | RSQJS_ARTIFACT_REPORT_YAML_RELATIVE_PATH | \
      RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE | RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH | \
      RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE | RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH | \
      RSQJS_ARTIFACT_TIMESTAMP | RSQJS_ARTIFACT_COMMIT_SHA | RSQJS_ARTIFACT_TREE_SHA | \
      RSQJS_ARTIFACT_EVENT_NAME | RSQJS_ARTIFACT_RUN_ID | RSQJS_ARTIFACT_RUN_ATTEMPT | \
      RSQJS_ARTIFACT_REPOSITORY | RSQJS_ARTIFACT_WORKFLOW | \
      RSQJS_ARTIFACT_PR_NUMBER | RSQJS_ARTIFACT_TASK)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
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
    if [[ -n "${seen[${key}]:-}" ]]; then
      return 1
    fi
    if ! decoded="$(printf '%s' "${encoded}" | base64 --decode 2>/dev/null)"; then
      return 1
    fi
    seen["${key}"]=1
    printf -v "${key}" '%s' "${decoded}"
  done < "${metadata_file}"
}

valid_report_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^rsqjs-test-report-[0-9]{8}T[0-9]{6}Z\.md$ ]]
}

valid_report_yaml_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^rsqjs-test-report-[0-9]{8}T[0-9]{6}Z\.yaml$ ]]
}

valid_report_details_yaml_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^rsqjs-test-report-[0-9]{8}T[0-9]{6}Z-details\.yaml$ ]]
}

valid_jetstream_report_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^rsqjs-jetstream-report-[0-9]{8}T[0-9]{6}Z\.md$ ]]
}

valid_artifact_relative_path() {
  local relative_path="$1"
  local directory="$2"
  local file_name="$3"
  [[ "${relative_path}" == "${directory}/${file_name}" ]]
}

download_matching_artifact() {
  local repository="$1"
  local artifact_name="$2"
  local expected_tree="$3"
  local expected_mode="$4"
  local target_dir="$5"

  local artifact_lines
  artifact_lines="$(gh api "/repos/${repository}/actions/artifacts?name=${artifact_name}&per_page=100" \
    --jq '.artifacts | sort_by(.created_at) | reverse | .[] | select(.expired == false) | [.id, .workflow_run.id] | @tsv')"
  [[ -n "${artifact_lines}" ]] || fail "no non-expired artifact named '${artifact_name}'"

  local artifact_id run_id candidate metadata_file
  while IFS=$'\t' read -r artifact_id run_id; do
    [[ -n "${artifact_id}" && -n "${run_id}" ]] || continue
    candidate="${target_dir}/artifact-${artifact_id}"
    mkdir -p "${candidate}"
    if ! gh run download "${run_id}" --repo "${repository}" --name "${artifact_name}" --dir "${candidate}" >/dev/null; then
      printf 'skipping artifact %s from run %s: download failed\n' "${artifact_id}" "${run_id}" >&2
      continue
    fi
    metadata_file="${candidate}/rsqjs-report-metadata.env"
    if ! read_metadata "${metadata_file}"; then
      printf 'skipping artifact %s: missing or invalid metadata\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${RSQJS_ARTIFACT_SCHEMA:-}" != "2" ]]; then
      printf 'skipping artifact %s: unsupported metadata schema\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${RSQJS_ARTIFACT_REPORT_MODE:-}" != "${expected_mode}" ]]; then
      printf 'skipping artifact %s: report mode mismatch\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${RSQJS_ARTIFACT_TREE_SHA:-}" != "${expected_tree}" ]]; then
      printf 'skipping artifact %s: tree mismatch\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -z "${RSQJS_ARTIFACT_REPORT_FILE:-}" || -z "${RSQJS_ARTIFACT_REPORT_RELATIVE_PATH:-}" ]]; then
      printf 'skipping artifact %s: missing report path metadata\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_report_file "${RSQJS_ARTIFACT_REPORT_FILE}"; then
      printf 'skipping artifact %s: invalid report file name %s\n' "${artifact_id}" "${RSQJS_ARTIFACT_REPORT_FILE}" >&2
      continue
    fi
    if [[ ! "${RSQJS_ARTIFACT_TIMESTAMP:-}" =~ ^[0-9]{8}T[0-9]{6}Z$ || "${RSQJS_ARTIFACT_REPORT_FILE}" != "rsqjs-test-report-${RSQJS_ARTIFACT_TIMESTAMP}.md" ]]; then
      printf 'skipping artifact %s: report timestamp metadata does not match file name\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_artifact_relative_path "${RSQJS_ARTIFACT_REPORT_RELATIVE_PATH}" test-runs "${RSQJS_ARTIFACT_REPORT_FILE}"; then
      printf 'skipping artifact %s: invalid report relative path\n' "${artifact_id}" >&2
      continue
    fi
    if [[ ! -f "${candidate}/${RSQJS_ARTIFACT_REPORT_RELATIVE_PATH}" ]]; then
      printf 'skipping artifact %s: report file is absent\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -z "${RSQJS_ARTIFACT_REPORT_YAML_FILE:-}" || -z "${RSQJS_ARTIFACT_REPORT_YAML_RELATIVE_PATH:-}" ]]; then
      printf 'skipping artifact %s: missing YAML summary path metadata\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_report_yaml_file "${RSQJS_ARTIFACT_REPORT_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML summary file name %s\n' "${artifact_id}" "${RSQJS_ARTIFACT_REPORT_YAML_FILE}" >&2
      continue
    fi
    if ! valid_artifact_relative_path "${RSQJS_ARTIFACT_REPORT_YAML_RELATIVE_PATH}" test-runs "${RSQJS_ARTIFACT_REPORT_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML summary relative path\n' "${artifact_id}" >&2
      continue
    fi
    if [[ ! -f "${candidate}/${RSQJS_ARTIFACT_REPORT_YAML_RELATIVE_PATH}" ]]; then
      printf 'skipping artifact %s: YAML summary file is absent\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -z "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE:-}" || -z "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH:-}" ]]; then
      printf 'skipping artifact %s: missing YAML details path metadata\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_report_details_yaml_file "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML details file name %s\n' "${artifact_id}" "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE}" >&2
      continue
    fi
    if ! valid_artifact_relative_path "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH}" test-runs "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML details relative path\n' "${artifact_id}" >&2
      continue
    fi
    local expected_yaml_file="${RSQJS_ARTIFACT_REPORT_FILE%.md}.yaml"
    local expected_details_yaml_file="${RSQJS_ARTIFACT_REPORT_FILE%.md}-details.yaml"
    if [[ "${RSQJS_ARTIFACT_REPORT_YAML_FILE}" != "${expected_yaml_file}" || "${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_FILE}" != "${expected_details_yaml_file}" ]]; then
      printf 'skipping artifact %s: YAML files do not match Markdown report name\n' "${artifact_id}" >&2
      continue
    fi
    if [[ ! -f "${candidate}/${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH}" ]]; then
      printf 'skipping artifact %s: YAML details file is absent\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -n "${RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE:-}" || -n "${RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH:-}" ]]; then
      if [[ -z "${RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE:-}" || -z "${RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH:-}" ]]; then
        printf 'skipping artifact %s: incomplete JetStream report metadata\n' "${artifact_id}" >&2
        continue
      fi
      if ! valid_jetstream_report_file "${RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE}"; then
        printf 'skipping artifact %s: invalid JetStream report file name %s\n' "${artifact_id}" "${RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE}" >&2
        continue
      fi
      if ! valid_artifact_relative_path "${RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH}" jetstream-runs "${RSQJS_ARTIFACT_JETSTREAM_REPORT_FILE}"; then
        printf 'skipping artifact %s: invalid JetStream report relative path\n' "${artifact_id}" >&2
        continue
      fi
      if [[ ! -f "${candidate}/${RSQJS_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH}" ]]; then
        printf 'skipping artifact %s: JetStream report file is absent\n' "${artifact_id}" >&2
        continue
      fi
    fi
    printf '%s\n' "${candidate}"
    return 0
  done <<< "${artifact_lines}"

  fail "no artifact named '${artifact_name}' matched tree ${expected_tree}"
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

fetch_tested_source_commit() {
  local source_commit="$1"
  local expected_tree="$2"

  if [[ ! "${source_commit}" =~ ^[0-9a-f]{40}$ ]]; then
    fail "invalid source commit in artifact metadata: ${source_commit}"
  fi

  git fetch --no-tags origin "${source_commit}"
  git cat-file -e "${source_commit}^{commit}" || fail "source commit is not a commit: ${source_commit}"

  local source_tree
  source_tree="$(git rev-parse "${source_commit}^{tree}")"
  if [[ "${source_tree}" != "${expected_tree}" ]]; then
    fail "source commit tree mismatch: ${source_tree} != ${expected_tree}"
  fi
}

archive_commit_message() {
  local source_commit="$1"
  local expected_tree="$2"
  local source_run="$3"

  printf 'Archive tested source %.12s [skip ci]\n\n' "${source_commit}"
  printf 'Source commit: %s\n' "${source_commit}"
  printf 'Source tree: %s\n' "${expected_tree}"
  printf 'Source workflow run: %s\n' "${source_run}"
}

validate_full_ref() {
  local ref_name="$1"
  local label="$2"

  [[ "${ref_name}" == refs/* ]] || fail "${label} must be a full ref: ${ref_name}"
  git check-ref-format "${ref_name}" >/dev/null ||
    fail "invalid ${label}: ${ref_name}"
}

resolve_archive_ref() {
  local archive_ref="${RSQJS_TESTED_SOURCE_ARCHIVE_REF:-}"
  if [[ -z "${archive_ref}" ]]; then
    local archive_name="${RSQJS_TESTED_SOURCE_ARCHIVE_BRANCH:-}"
    if [[ -n "${archive_name}" ]]; then
      archive_name="${archive_name#refs/heads/}"
      [[ "${archive_name}" != refs/* ]] ||
        fail "legacy archive branch must be a branch name: ${archive_name}"
      archive_ref="refs/rsqjs/${archive_name}"
    else
      archive_ref="${default_tested_source_archive_ref}"
    fi
  fi

  validate_full_ref "${archive_ref}" "tested source archive ref"
  [[ "${archive_ref}" != refs/heads/* ]] ||
    fail "tested source archive ref must not live under refs/heads: ${archive_ref}"
  printf '%s\n' "${archive_ref}"
}

resolve_legacy_archive_ref() {
  local legacy_ref="${RSQJS_TESTED_SOURCE_ARCHIVE_LEGACY_REF:-${default_legacy_tested_source_archive_ref}}"
  if [[ "${legacy_ref}" != refs/* ]]; then
    legacy_ref="refs/heads/${legacy_ref}"
  fi
  validate_full_ref "${legacy_ref}" "legacy tested source archive ref"
  printf '%s\n' "${legacy_ref}"
}

checkout_archive_ref() {
  local archive_ref="$1"
  local legacy_ref="$2"
  local local_branch="$3"

  if git ls-remote --exit-code origin "${archive_ref}" >/dev/null 2>&1; then
    git fetch --no-tags origin "${archive_ref}"
    git checkout -B "${local_branch}" FETCH_HEAD
    printf 'tested source archive base: %s\n' "${archive_ref}"
    return 0
  fi
  if git ls-remote --exit-code origin "${legacy_ref}" >/dev/null 2>&1; then
    git fetch --no-tags origin "${legacy_ref}"
    git checkout -B "${local_branch}" FETCH_HEAD
    printf 'tested source archive base: %s\n' "${legacy_ref}"
    return 0
  fi
  return 1
}

archive_tested_source_commit() {
  local archive_ref="$1"
  local legacy_ref="$2"
  local source_commit="$3"
  local expected_tree="$4"
  local source_run="$5"

  fetch_tested_source_commit "${source_commit}" "${expected_tree}"
  git check-ref-format --branch "${tested_source_archive_local_branch}" >/dev/null ||
    fail "invalid tested source archive local branch: ${tested_source_archive_local_branch}"

  local archive_message
  archive_message="$(archive_commit_message "${source_commit}" "${expected_tree}" "${source_run}")"

  if checkout_archive_ref "${archive_ref}" "${legacy_ref}" "${tested_source_archive_local_branch}"; then
    if git merge-base --is-ancestor "${source_commit}" HEAD; then
      if ! git ls-remote --exit-code origin "${archive_ref}" >/dev/null 2>&1; then
        git push origin "HEAD:${archive_ref}"
        printf 'tested source archive ref migrated: %s\n' "${archive_ref}"
      fi
      printf 'tested source commit already archived: %s\n' "${source_commit}"
      return 0
    fi
    git merge --no-ff -s ours -m "${archive_message}" "${source_commit}"
  else
    git checkout -B "${tested_source_archive_local_branch}" "${source_commit}"
    git commit --allow-empty -m "${archive_message}"
  fi

  git push origin "HEAD:${archive_ref}"
  printf 'tested source archive ref updated: %s -> %s\n' "${archive_ref}" "${source_commit}"
}

stage_report_outputs() {
  local source_report="$1"
  local report_file="$2"
  local source_report_yaml="$3"
  local report_yaml_file="$4"
  local source_jetstream_report="${5:-}"
  local jetstream_report_file="${6:-}"

  mkdir -p reports/test-runs
  local target_report="reports/test-runs/${report_file}"
  if [[ -f "${target_report}" ]] && ! cmp -s "${source_report}" "${target_report}"; then
    fail "tracked report already exists with different content: ${target_report}"
  fi
  cp "${source_report}" "${target_report}"

  local target_report_yaml="reports/test-runs/${report_yaml_file}"
  if [[ -f "${target_report_yaml}" ]] && ! cmp -s "${source_report_yaml}" "${target_report_yaml}"; then
    fail "tracked YAML report already exists with different content: ${target_report_yaml}"
  fi
  cp "${source_report_yaml}" "${target_report_yaml}"

  if [[ -n "${source_jetstream_report}" && -n "${jetstream_report_file}" ]]; then
    mkdir -p reports/jetstream-runs
    local target_jetstream_report="reports/jetstream-runs/${jetstream_report_file}"
    if [[ -f "${target_jetstream_report}" ]] && ! cmp -s "${source_jetstream_report}" "${target_jetstream_report}"; then
      fail "tracked JetStream report already exists with different content: ${target_jetstream_report}"
    fi
    cp "${source_jetstream_report}" "${target_jetstream_report}"
  fi

  cargo run --manifest-path runner/Cargo.toml -- --aggregate-reports reports/test-runs
}

create_report_commit_payload() {
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
    contents = pathlib.Path(path).read_bytes()
    additions.append({
        "path": path,
        "contents": base64.b64encode(contents).decode("ascii"),
    })

query = """
mutation($input: CreateCommitOnBranchInput!) {
  createCommitOnBranch(input: $input) {
    commit {
      oid
      url
    }
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
            "message": {
                "headline": headline,
                "body": body,
            },
            "fileChanges": {
                "additions": additions,
            },
        },
    },
}
pathlib.Path(payload_path).write_text(json.dumps(payload), encoding="utf-8")
PY
}

create_signed_main_commit() {
  local repository="$1"
  local headline="$2"
  local body="$3"
  shift 3

  local expected_head payload_path commit_oid
  expected_head="$(git rev-parse HEAD)"
  payload_path="$(mktemp)"
  create_report_commit_payload "${payload_path}" "${repository}" "${expected_head}" "${headline}" "${body}" "$@"
  if ! commit_oid="$(gh api graphql --input "${payload_path}" --jq '.data.createCommitOnBranch.commit.oid')"; then
    rm -f "${payload_path}"
    return 1
  fi
  rm -f "${payload_path}"
  [[ -n "${commit_oid}" ]] || return 1
  printf 'signed GitHub report commit: %s\n' "${commit_oid}"
}

reset_report_outputs() {
  local target_report="$1"
  local target_report_yaml="$2"
  local target_jetstream_report="${3:-}"

  if git ls-files --error-unmatch "${target_report}" >/dev/null 2>&1; then
    git restore --worktree -- "${target_report}"
  else
    rm -f "${target_report}"
  fi
  if git ls-files --error-unmatch "${target_report_yaml}" >/dev/null 2>&1; then
    git restore --worktree -- "${target_report_yaml}"
  else
    rm -f "${target_report_yaml}"
  fi
  if [[ -n "${target_jetstream_report}" ]]; then
    if git ls-files --error-unmatch "${target_jetstream_report}" >/dev/null 2>&1; then
      git restore --worktree -- "${target_jetstream_report}"
    else
      rm -f "${target_jetstream_report}"
    fi
  fi
  git restore --worktree -- reports/benchmark-rollup.md reports/benchmark-summary.jpg
}

commit_and_push() {
  local report_file="$1"
  local report_yaml_file="$2"
  local expected_tree="$3"
  local source_commit="$4"
  local source_run="$5"

  local target_report="reports/test-runs/${report_file}"
  local target_report_yaml="reports/test-runs/${report_yaml_file}"
  local target_jetstream_report=""
  local commit_paths=("${target_report}" "${target_report_yaml}" reports/benchmark-rollup.md reports/benchmark-summary.jpg)
  if [[ -n "${jetstream_report_file:-}" ]]; then
    target_jetstream_report="reports/jetstream-runs/${jetstream_report_file}"
    commit_paths=("${target_report}" "${target_report_yaml}" "${target_jetstream_report}" reports/benchmark-rollup.md reports/benchmark-summary.jpg)
  fi

  if [[ -z "$(git status --porcelain -- "${commit_paths[@]}")" ]]; then
    printf 'canonical report outputs are already up to date\n'
    return 0
  fi

  local timestamp="${report_file#rsqjs-test-report-}"
  timestamp="${timestamp%.md}"
  local headline="Add rsqjs report ${timestamp} [skip ci]"
  local body
  body="$(printf 'Source commit: %s\n\nSource tree: %s\n\nSource workflow run: %s\n' \
    "${source_commit}" "${expected_tree}" "${source_run}")"

  if create_signed_main_commit "${repository}" "${headline}" "${body}" \
    "${commit_paths[@]}"; then
    return 0
  fi

  printf 'initial signed report commit failed; retrying once on latest origin/main\n' >&2
  reset_report_outputs "${target_report}" "${target_report_yaml}" "${target_jetstream_report}"
  checkout_latest_main
  stage_report_outputs "${source_report}" "${report_file}" "${source_report_yaml}" "${report_yaml_file}" "${source_jetstream_report:-}" "${jetstream_report_file:-}"
  create_signed_main_commit "${repository}" "${headline}" "${body}" \
    "${commit_paths[@]}"
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
  return 0
fi

need_cmd gh
need_cmd git
need_cmd cargo
need_cmd python3
need_cmd base64

repository="${GITHUB_REPOSITORY:-}"
[[ -n "${repository}" ]] || fail "GITHUB_REPOSITORY is required"

merge_commit="${RSQJS_MERGE_COMMIT_SHA:-}"
[[ -n "${merge_commit}" ]] || fail "RSQJS_MERGE_COMMIT_SHA is required"

expected_tree="${RSQJS_EXPECTED_TREE_SHA:-}"
if [[ -z "${expected_tree}" ]]; then
  expected_tree="$(git rev-parse "${merge_commit}^{tree}")"
fi
performance_artifact_name="${RSQJS_REPORT_ARTIFACT_NAME:-rsqjs-reports-${expected_tree}}"
correctness_artifact_name="${RSQJS_CORRECTNESS_ARTIFACT_NAME:-rsqjs-correctness-${expected_tree}}"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

performance_artifact_dir="$(download_matching_artifact "${repository}" "${performance_artifact_name}" "${expected_tree}" performance "${tmp_dir}/performance")"
performance_metadata_file="${performance_artifact_dir}/rsqjs-report-metadata.env"
read_metadata "${performance_metadata_file}" || fail "failed to read performance artifact metadata"
performance_details="${performance_artifact_dir}/${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH}"
performance_timestamp="${RSQJS_ARTIFACT_TIMESTAMP:-}"
source_commit="${RSQJS_ARTIFACT_COMMIT_SHA:-unknown}"
performance_run="${RSQJS_ARTIFACT_RUN_ID:-unknown}"

correctness_artifact_dir="$(download_matching_artifact "${repository}" "${correctness_artifact_name}" "${expected_tree}" correctness "${tmp_dir}/correctness")"
correctness_metadata_file="${correctness_artifact_dir}/rsqjs-report-metadata.env"
read_metadata "${correctness_metadata_file}" || fail "failed to read correctness artifact metadata"
correctness_details="${correctness_artifact_dir}/${RSQJS_ARTIFACT_REPORT_DETAILS_YAML_RELATIVE_PATH}"
correctness_run="${RSQJS_ARTIFACT_RUN_ID:-unknown}"

[[ "${performance_timestamp}" =~ ^[0-9]{8}T[0-9]{6}Z$ ]] ||
  fail "invalid performance artifact timestamp: ${performance_timestamp}"
report_file="rsqjs-test-report-${performance_timestamp}.md"
report_yaml_file="${report_file%.md}.yaml"
composed_dir="${tmp_dir}/composed/test-runs"
source_report="${composed_dir}/${report_file}"
source_report_yaml="${composed_dir}/${report_yaml_file}"
mkdir -p "${composed_dir}"
cargo run --manifest-path runner/Cargo.toml -- --compose-reports \
  "${expected_tree}" "${correctness_details}" "${performance_details}" "${source_report}"

jetstream_report_file=""
source_jetstream_report=""
source_run="correctness:${correctness_run}, performance:${performance_run}"
archive_ref="$(resolve_archive_ref)"
legacy_archive_ref="$(resolve_legacy_archive_ref)"

archive_tested_source_commit "${archive_ref}" "${legacy_archive_ref}" "${source_commit}" "${expected_tree}" "${source_run}"
checkout_latest_main
stage_report_outputs "${source_report}" "${report_file}" "${source_report_yaml}" "${report_yaml_file}" "${source_jetstream_report}" "${jetstream_report_file}"
commit_and_push "${report_file}" "${report_yaml_file}" "${expected_tree}" "${source_commit}" "${source_run}"
