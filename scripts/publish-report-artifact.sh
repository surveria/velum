#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

default_tested_source_archive_ref="refs/velum/ci-tested-sources"
default_legacy_tested_source_archive_ref="refs/heads/ci-tested-sources"
tested_source_archive_local_branch="velum-tested-source-archive"
null_workflow_conclusion="__VELUM_NULL_CONCLUSION__"
default_artifact_wait_attempts=37
default_artifact_wait_seconds=10
test262_baseline_path="${repo_root}/tests/corpora/test262/full-pass-baseline.txt"
test262_baseline_schema='# velum-test262-pass-baseline-v2'

fail() {
  printf 'publish-report-artifact: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

clear_metadata() {
  unset VELUM_ARTIFACT_SCHEMA VELUM_ARTIFACT_REPORT_MODE
  unset VELUM_ARTIFACT_REPORT_FILE VELUM_ARTIFACT_REPORT_RELATIVE_PATH
  unset VELUM_ARTIFACT_REPORT_YAML_FILE VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH
  unset VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH
  unset VELUM_ARTIFACT_JETSTREAM_REPORT_FILE VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH
  unset VELUM_ARTIFACT_TIMESTAMP VELUM_ARTIFACT_COMMIT_SHA VELUM_ARTIFACT_TREE_SHA
  unset VELUM_ARTIFACT_EVENT_NAME VELUM_ARTIFACT_RUN_ID VELUM_ARTIFACT_RUN_ATTEMPT
  unset VELUM_ARTIFACT_REPOSITORY VELUM_ARTIFACT_WORKFLOW
  unset VELUM_ARTIFACT_PR_NUMBER VELUM_ARTIFACT_TASK
}

valid_metadata_key() {
  case "$1" in
    VELUM_ARTIFACT_SCHEMA | VELUM_ARTIFACT_REPORT_MODE | \
      VELUM_ARTIFACT_REPORT_FILE | VELUM_ARTIFACT_REPORT_RELATIVE_PATH | \
      VELUM_ARTIFACT_REPORT_YAML_FILE | VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH | \
      VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE | VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH | \
      VELUM_ARTIFACT_JETSTREAM_REPORT_FILE | VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH | \
      VELUM_ARTIFACT_TIMESTAMP | VELUM_ARTIFACT_COMMIT_SHA | VELUM_ARTIFACT_TREE_SHA | \
      VELUM_ARTIFACT_EVENT_NAME | VELUM_ARTIFACT_RUN_ID | VELUM_ARTIFACT_RUN_ATTEMPT | \
      VELUM_ARTIFACT_REPOSITORY | VELUM_ARTIFACT_WORKFLOW | \
      VELUM_ARTIFACT_PR_NUMBER | VELUM_ARTIFACT_TASK)
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
  [[ "${file_name}" =~ ^velum-test-report-[0-9]{8}T[0-9]{6}Z\.md$ ]]
}

valid_report_yaml_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^velum-test-report-[0-9]{8}T[0-9]{6}Z\.yaml$ ]]
}

valid_report_component_yaml_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^velum-test-report-[0-9]{8}T[0-9]{6}Z-component\.yaml$ ]]
}

valid_test262_baseline_candidate() {
  local path="$1"
  [[ -f "${path}" && ! -L "${path}" ]] || return 1
  [[ "$(stat -c '%s' "${path}")" -le 33554432 ]] || return 1
  [[ -f "${test262_baseline_path}" && ! -L "${test262_baseline_path}" ]] || return 1
  [[ "$(sed -n '1p' "${path}")" == "${test262_baseline_schema}" ]] || return 1
  [[ "$(sed -n '1p' "${test262_baseline_path}")" == "${test262_baseline_schema}" ]] || return 1
  [[ "$(sed -n '2p' "${path}")" == "$(sed -n '2p' "${test262_baseline_path}")" ]] || return 1
  [[ "$(sed -n '3p' "${path}")" == "$(sed -n '3p' "${test262_baseline_path}")" ]] || return 1
  [[ -n "$(sed -n '4p' "${path}")" ]] || return 1
  tail -n +4 "${path}" | LC_ALL=C sort -c -u >/dev/null 2>&1
}

valid_jetstream_report_file() {
  local file_name="$1"
  [[ "${file_name}" =~ ^velum-jetstream-report-[0-9]{8}T[0-9]{6}Z\.md$ ]]
}

valid_artifact_relative_path() {
  local relative_path="$1"
  local directory="$2"
  local file_name="$3"
  [[ "${relative_path}" == "${directory}/${file_name}" ]]
}

valid_artifact_run_attempt() {
  local expected_mode="$1"
  local artifact_attempt="$2"
  local workflow_attempt="$3"

  [[ "${artifact_attempt}" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ "${workflow_attempt}" =~ ^[1-9][0-9]*$ ]] || return 1
  if [[ "${expected_mode}" == correctness ]]; then
    [[ "${artifact_attempt}" == "${workflow_attempt}" ]]
    return
  fi
  [[ "${expected_mode}" == performance ]] || return 1
  if ((${#artifact_attempt} < ${#workflow_attempt})); then
    return 0
  fi
  if ((${#artifact_attempt} > ${#workflow_attempt})); then
    return 1
  fi
  [[ "${artifact_attempt}" == "${workflow_attempt}" || "${artifact_attempt}" < "${workflow_attempt}" ]]
}

validate_workflow_run_fields() {
  local expected_mode="$1"
  local expected_repository="$2"
  local expected_tree="$3"
  local expected_run_id="$4"
  local actual_run_id="$5"
  local actual_repository="$6"
  local workflow_path="$7"
  local workflow_name="$8"
  local event_name="$9"
  local status="${10}"
  local conclusion="${11}"
  local workflow_head_tree="${12}"

  [[ "${actual_run_id}" =~ ^[0-9]+$ ]] || return 1
  [[ "${actual_repository}" == "${expected_repository}" ]] || return 1
  [[ "${workflow_path}" == ".github/workflows/ci.yml" ]] || return 1
  [[ "${workflow_name}" == "CI" ]] || return 1
  if [[ "${expected_mode}" == performance ]]; then
    [[ -n "${expected_run_id}" && "${actual_run_id}" == "${expected_run_id}" ]] || return 1
    [[ "${event_name}" == pull_request ]] || return 1
    if [[ "${status}" == completed ]]; then
      [[ "${conclusion}" == success ]] || return 1
    else
      [[ "${status}" == in_progress && "${conclusion}" == "${null_workflow_conclusion}" ]] || return 1
    fi
    return 0
  fi
  [[ "${expected_mode}" == correctness ]] || return 1
  if [[ "${event_name}" == workflow_dispatch ]]; then
    [[ "${status}" == completed && "${conclusion}" == success ]] || return 1
    return 0
  fi
  [[ "${workflow_head_tree}" == "${expected_tree}" ]] || return 1
  [[ "${event_name}" == pull_request || "${event_name}" == merge_group ]] || return 1
  [[ "${status}" == completed && "${conclusion}" == success ]]
}

load_trusted_workflow_run() {
  local repository="$1"
  local run_id="$2"
  local expected_tree="$3"
  local expected_mode="$4"
  local expected_run_id="$5"

  local fields
  if ! fields="$(gh api "/repos/${repository}/actions/runs/${run_id}" \
    --jq "[.id, .repository.full_name, .path, .name, .event, .status, (.conclusion // \"${null_workflow_conclusion}\"), .head_sha, .run_attempt] | @tsv")"; then
    return 1
  fi
  IFS=$'\t' read -r RUN_ID RUN_REPOSITORY RUN_PATH RUN_NAME RUN_EVENT RUN_STATUS RUN_CONCLUSION RUN_HEAD_SHA RUN_ATTEMPT <<< "${fields}"
  [[ "${RUN_HEAD_SHA}" =~ ^[0-9a-f]{40}$ ]] || return 1
  if ! RUN_HEAD_TREE="$(gh api "/repos/${repository}/git/commits/${RUN_HEAD_SHA}" --jq '.tree.sha')"; then
    return 1
  fi
  validate_workflow_run_fields "${expected_mode}" "${repository}" "${expected_tree}" \
    "${expected_run_id}" "${RUN_ID}" "${RUN_REPOSITORY}" "${RUN_PATH}" "${RUN_NAME}" \
    "${RUN_EVENT}" "${RUN_STATUS}" "${RUN_CONCLUSION}" "${RUN_HEAD_TREE}"
}

commit_tree_from_github() {
  local repository="$1"
  local commit_sha="$2"
  [[ "${commit_sha}" =~ ^[0-9a-f]{40}$ ]] || return 1
  gh api "/repos/${repository}/git/commits/${commit_sha}" --jq '.tree.sha'
}

download_matching_artifact() {
  local repository="$1"
  local artifact_name="$2"
  local expected_tree="$3"
  local expected_mode="$4"
  local target_dir="$5"
  local expected_run_id="${6:-}"

  local artifact_lines
  artifact_lines="$(gh api "/repos/${repository}/actions/artifacts?name=${artifact_name}&per_page=100" \
    --jq '.artifacts | sort_by(.created_at) | reverse | .[] | select(.expired == false) | [.id, .workflow_run.id] | @tsv')"
  if [[ -z "${artifact_lines}" ]]; then
    printf 'no non-expired artifact named %q\n' "${artifact_name}" >&2
    return 1
  fi

  local artifact_id run_id candidate metadata_file
  while IFS=$'\t' read -r artifact_id run_id; do
    [[ -n "${artifact_id}" && -n "${run_id}" ]] || continue
    if ! load_trusted_workflow_run "${repository}" "${run_id}" "${expected_tree}" "${expected_mode}" "${expected_run_id}"; then
      printf 'skipping artifact %s from untrusted workflow run %s\n' "${artifact_id}" "${run_id}" >&2
      continue
    fi
    candidate="${target_dir}/artifact-${artifact_id}"
    mkdir -p "${candidate}"
    if ! gh run download "${run_id}" --repo "${repository}" --name "${artifact_name}" --dir "${candidate}" >/dev/null; then
      printf 'skipping artifact %s from run %s: download failed\n' "${artifact_id}" "${run_id}" >&2
      continue
    fi
    metadata_file="${candidate}/velum-report-metadata.env"
    if ! read_metadata "${metadata_file}"; then
      printf 'skipping artifact %s: missing or invalid metadata\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${VELUM_ARTIFACT_SCHEMA:-}" != "3" ]]; then
      printf 'skipping artifact %s: unsupported metadata schema\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${VELUM_ARTIFACT_REPORT_MODE:-}" != "${expected_mode}" ]]; then
      printf 'skipping artifact %s: report mode mismatch\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${VELUM_ARTIFACT_TREE_SHA:-}" != "${expected_tree}" ]]; then
      printf 'skipping artifact %s: tree mismatch\n' "${artifact_id}" >&2
      continue
    fi
    if [[ "${VELUM_ARTIFACT_RUN_ID:-}" != "${RUN_ID}" \
      || "${VELUM_ARTIFACT_REPOSITORY:-}" != "${RUN_REPOSITORY}" \
      || "${VELUM_ARTIFACT_WORKFLOW:-}" != "${RUN_NAME}" \
      || "${VELUM_ARTIFACT_EVENT_NAME:-}" != "${RUN_EVENT}" ]] \
      || ! valid_artifact_run_attempt "${expected_mode}" \
        "${VELUM_ARTIFACT_RUN_ATTEMPT:-}" "${RUN_ATTEMPT}"; then
      printf 'skipping artifact %s: workflow metadata envelope mismatch\n' "${artifact_id}" >&2
      continue
    fi
    local metadata_commit_tree
    if ! metadata_commit_tree="$(commit_tree_from_github "${repository}" "${VELUM_ARTIFACT_COMMIT_SHA:-}")" \
      || [[ "${metadata_commit_tree}" != "${expected_tree}" ]]; then
      printf 'skipping artifact %s: tested commit does not resolve to expected tree\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -z "${VELUM_ARTIFACT_REPORT_FILE:-}" || -z "${VELUM_ARTIFACT_REPORT_RELATIVE_PATH:-}" ]]; then
      printf 'skipping artifact %s: missing report path metadata\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_report_file "${VELUM_ARTIFACT_REPORT_FILE}"; then
      printf 'skipping artifact %s: invalid report file name %s\n' "${artifact_id}" "${VELUM_ARTIFACT_REPORT_FILE}" >&2
      continue
    fi
    if [[ ! "${VELUM_ARTIFACT_TIMESTAMP:-}" =~ ^[0-9]{8}T[0-9]{6}Z$ || "${VELUM_ARTIFACT_REPORT_FILE}" != "velum-test-report-${VELUM_ARTIFACT_TIMESTAMP}.md" ]]; then
      printf 'skipping artifact %s: report timestamp metadata does not match file name\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_artifact_relative_path "${VELUM_ARTIFACT_REPORT_RELATIVE_PATH}" test-runs "${VELUM_ARTIFACT_REPORT_FILE}"; then
      printf 'skipping artifact %s: invalid report relative path\n' "${artifact_id}" >&2
      continue
    fi
    if [[ ! -f "${candidate}/${VELUM_ARTIFACT_REPORT_RELATIVE_PATH}" \
      || -L "${candidate}/${VELUM_ARTIFACT_REPORT_RELATIVE_PATH}" ]]; then
      printf 'skipping artifact %s: report file is absent\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -z "${VELUM_ARTIFACT_REPORT_YAML_FILE:-}" || -z "${VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH:-}" ]]; then
      printf 'skipping artifact %s: missing YAML summary path metadata\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_report_yaml_file "${VELUM_ARTIFACT_REPORT_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML summary file name %s\n' "${artifact_id}" "${VELUM_ARTIFACT_REPORT_YAML_FILE}" >&2
      continue
    fi
    if ! valid_artifact_relative_path "${VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH}" test-runs "${VELUM_ARTIFACT_REPORT_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML summary relative path\n' "${artifact_id}" >&2
      continue
    fi
    if [[ ! -f "${candidate}/${VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH}" \
      || -L "${candidate}/${VELUM_ARTIFACT_REPORT_YAML_RELATIVE_PATH}" ]]; then
      printf 'skipping artifact %s: YAML summary file is absent\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -z "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE:-}" || -z "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH:-}" ]]; then
      printf 'skipping artifact %s: missing YAML component path metadata\n' "${artifact_id}" >&2
      continue
    fi
    if ! valid_report_component_yaml_file "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML component file name %s\n' "${artifact_id}" "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE}" >&2
      continue
    fi
    if ! valid_artifact_relative_path "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH}" test-runs "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE}"; then
      printf 'skipping artifact %s: invalid YAML component relative path\n' "${artifact_id}" >&2
      continue
    fi
    local expected_yaml_file="${VELUM_ARTIFACT_REPORT_FILE%.md}.yaml"
    local expected_component_yaml_file="${VELUM_ARTIFACT_REPORT_FILE%.md}-component.yaml"
    if [[ "${VELUM_ARTIFACT_REPORT_YAML_FILE}" != "${expected_yaml_file}" || "${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_FILE}" != "${expected_component_yaml_file}" ]]; then
      printf 'skipping artifact %s: YAML files do not match Markdown report name\n' "${artifact_id}" >&2
      continue
    fi
    if [[ ! -f "${candidate}/${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH}" \
      || -L "${candidate}/${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH}" ]]; then
      printf 'skipping artifact %s: YAML component file is absent\n' "${artifact_id}" >&2
      continue
    fi
    if [[ -n "${VELUM_ARTIFACT_JETSTREAM_REPORT_FILE:-}" || -n "${VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH:-}" ]]; then
      if [[ -z "${VELUM_ARTIFACT_JETSTREAM_REPORT_FILE:-}" || -z "${VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH:-}" ]]; then
        printf 'skipping artifact %s: incomplete JetStream report metadata\n' "${artifact_id}" >&2
        continue
      fi
      if ! valid_jetstream_report_file "${VELUM_ARTIFACT_JETSTREAM_REPORT_FILE}"; then
        printf 'skipping artifact %s: invalid JetStream report file name %s\n' "${artifact_id}" "${VELUM_ARTIFACT_JETSTREAM_REPORT_FILE}" >&2
        continue
      fi
      if ! valid_artifact_relative_path "${VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH}" jetstream-runs "${VELUM_ARTIFACT_JETSTREAM_REPORT_FILE}"; then
        printf 'skipping artifact %s: invalid JetStream report relative path\n' "${artifact_id}" >&2
        continue
      fi
      if [[ ! -f "${candidate}/${VELUM_ARTIFACT_JETSTREAM_REPORT_RELATIVE_PATH}" ]]; then
        printf 'skipping artifact %s: JetStream report file is absent\n' "${artifact_id}" >&2
        continue
      fi
    fi
    printf '%s\n' "${candidate}"
    return 0
  done <<< "${artifact_lines}"

  printf 'no artifact named %q matched tree %s\n' "${artifact_name}" "${expected_tree}" >&2
  return 1
}

download_matching_artifact_with_retry() {
  local repository="$1"
  local artifact_name="$2"
  local expected_tree="$3"
  local expected_mode="$4"
  local target_dir="$5"
  local expected_run_id="${6:-}"
  local attempts="${VELUM_ARTIFACT_WAIT_ATTEMPTS:-${default_artifact_wait_attempts}}"
  local wait_seconds="${VELUM_ARTIFACT_WAIT_SECONDS:-${default_artifact_wait_seconds}}"

  [[ "${attempts}" =~ ^[1-9][0-9]*$ ]] ||
    fail "VELUM_ARTIFACT_WAIT_ATTEMPTS must be a positive decimal integer"
  [[ "${wait_seconds}" =~ ^(0|[1-9][0-9]*)$ ]] ||
    fail "VELUM_ARTIFACT_WAIT_SECONDS must be a non-negative decimal integer"
  [[ -n "${target_dir}" ]] || fail "artifact retry target directory is empty"

  local attempt=1 artifact_dir
  while ((attempt <= attempts)); do
    if artifact_dir="$(download_matching_artifact "${repository}" "${artifact_name}" \
      "${expected_tree}" "${expected_mode}" "${target_dir}" "${expected_run_id}")"; then
      printf '%s\n' "${artifact_dir}"
      return 0
    fi
    rm -rf -- "${target_dir}"
    if ((attempt == attempts)); then
      fail "artifact '${artifact_name}' was not ready after ${attempts} attempts"
    fi
    printf 'artifact %q is not ready; retrying in %s seconds (attempt %s/%s)\n' \
      "${artifact_name}" "${wait_seconds}" "${attempt}" "${attempts}" >&2
    sleep "${wait_seconds}"
    attempt=$((attempt + 1))
  done
  fail "artifact retry loop ended unexpectedly"
}

checkout_latest_main() {
  local fetch_args=(--no-tags)
  if [[ "$(git rev-parse --is-shallow-repository)" == "true" ]]; then
    fetch_args+=(--unshallow)
  fi
  git fetch "${fetch_args[@]}" origin \
    '+refs/heads/main:refs/remotes/origin/main'
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
  local archive_ref="${VELUM_TESTED_SOURCE_ARCHIVE_REF:-}"
  if [[ -z "${archive_ref}" ]]; then
    local archive_name="${VELUM_TESTED_SOURCE_ARCHIVE_BRANCH:-}"
    if [[ -n "${archive_name}" ]]; then
      archive_name="${archive_name#refs/heads/}"
      [[ "${archive_name}" != refs/* ]] ||
        fail "legacy archive branch must be a branch name: ${archive_name}"
      archive_ref="refs/velum/${archive_name}"
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
  local legacy_ref="${VELUM_TESTED_SOURCE_ARCHIVE_LEGACY_REF:-${default_legacy_tested_source_archive_ref}}"
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
  local source_test262_baseline="${7:-}"

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

  if [[ -n "${source_test262_baseline}" ]]; then
    cp "${source_test262_baseline}" tests/corpora/test262/full-pass-baseline.txt
  fi

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

create_main_report_commit() {
  local headline="$1"
  local body="$2"
  shift 2

  if ! git diff --cached --quiet; then
    printf 'refusing report commit with pre-existing staged changes\n' >&2
    return 1
  fi

  local expected_head tree_oid commit_oid
  expected_head="$(git rev-parse HEAD)"
  git add -- "$@"
  if git diff --cached --quiet -- "$@"; then
    git restore --staged -- "$@"
    printf 'canonical report outputs are already up to date\n'
    return 0
  fi
  tree_oid="$(git write-tree)"
  if ! commit_oid="$(printf '%s\n\n%s\n' "${headline}" "${body}" | \
    git commit-tree "${tree_oid}" -p "${expected_head}")"; then
    git restore --staged -- "$@"
    return 1
  fi
  git restore --staged -- "$@"
  [[ "${commit_oid}" =~ ^[0-9a-f]{40}$ ]] || return 1
  git push origin "${commit_oid}:refs/heads/main" || return 1
  printf 'report-only Git commit: %s\n' "${commit_oid}"
}

reset_report_outputs() {
  local target_report="$1"
  local target_report_yaml="$2"
  local target_jetstream_report="${3:-}"

  git restore --worktree -- tests/corpora/test262/full-pass-baseline.txt

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
  local chart_path
  for chart_path in \
    reports/benchmark-rollup.md \
    reports/benchmark-summary-light.svg \
    reports/benchmark-summary-dark.svg; do
    if git ls-files --error-unmatch "${chart_path}" >/dev/null 2>&1; then
      git restore --worktree -- "${chart_path}"
    else
      rm -f "${chart_path}"
    fi
  done
}

report_commit_headline() {
  local task="$1"
  local pull_request="$2"
  local timestamp="$3"

  if [[ -n "${task}" && "${pull_request}" =~ ^[0-9]+$ ]]; then
    local pull_request_suffix="(#${pull_request})"
    if [[ "${task}" == *"${pull_request_suffix}" ]]; then
      printf '%s (CI) [skip ci]' "${task}"
    else
      printf '%s %s (CI) [skip ci]' "${task}" "${pull_request_suffix}"
    fi
    return 0
  fi

  printf 'Add Velum report %s (CI) [skip ci]' "${timestamp}"
}

commit_and_push() {
  local report_file="$1"
  local report_yaml_file="$2"
  local expected_tree="$3"
  local source_commit="$4"
  local source_run="$5"
  local source_task="$6"
  local source_pull_request="$7"

  local target_report="reports/test-runs/${report_file}"
  local target_report_yaml="reports/test-runs/${report_yaml_file}"
  local target_jetstream_report=""
  local commit_paths=(
    "${target_report}"
    "${target_report_yaml}"
    reports/benchmark-rollup.md
    reports/benchmark-summary-light.svg
    reports/benchmark-summary-dark.svg
    tests/corpora/test262/full-pass-baseline.txt
  )
  if [[ -n "${jetstream_report_file:-}" ]]; then
    target_jetstream_report="reports/jetstream-runs/${jetstream_report_file}"
    commit_paths=(
      "${target_report}"
      "${target_report_yaml}"
      "${target_jetstream_report}"
      reports/benchmark-rollup.md
      reports/benchmark-summary-light.svg
      reports/benchmark-summary-dark.svg
      tests/corpora/test262/full-pass-baseline.txt
    )
  fi

  if [[ -z "$(git status --porcelain -- "${commit_paths[@]}")" ]]; then
    printf 'canonical report outputs are already up to date\n'
    return 0
  fi

  local timestamp="${report_file#velum-test-report-}"
  timestamp="${timestamp%.md}"
  local headline
  headline="$(report_commit_headline "${source_task}" "${source_pull_request}" "${timestamp}")"
  local body
  body="$(printf 'Source commit: %s\n\nSource tree: %s\n\nSource workflow run: %s\n' \
    "${source_commit}" "${expected_tree}" "${source_run}")"

  if create_main_report_commit "${headline}" "${body}" \
    "${commit_paths[@]}"; then
    return 0
  fi

  printf 'initial report-only push failed; retrying once on latest origin/main\n' >&2
  reset_report_outputs "${target_report}" "${target_report_yaml}" "${target_jetstream_report}"
  checkout_latest_main
  stage_report_outputs "${source_report}" "${report_file}" "${source_report_yaml}" "${report_yaml_file}" "${source_jetstream_report:-}" "${jetstream_report_file:-}" "${source_test262_baseline:-}"
  create_main_report_commit "${headline}" "${body}" \
    "${commit_paths[@]}"
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then
  return 0
fi

need_cmd gh
need_cmd git
need_cmd cargo
need_cmd base64

repository="${GITHUB_REPOSITORY:-}"
[[ -n "${repository}" ]] || fail "GITHUB_REPOSITORY is required"

merge_commit="${VELUM_MERGE_COMMIT_SHA:-}"
[[ -n "${merge_commit}" ]] || fail "VELUM_MERGE_COMMIT_SHA is required"

expected_tree="${VELUM_EXPECTED_TREE_SHA:-}"
if [[ -z "${expected_tree}" ]]; then
  expected_tree="$(git rev-parse "${merge_commit}^{tree}")"
fi
performance_artifact_name="${VELUM_REPORT_ARTIFACT_NAME:-velum-reports-${expected_tree}}"
correctness_artifact_name="${VELUM_CORRECTNESS_ARTIFACT_NAME:-velum-correctness-${expected_tree}}"

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

current_run_id="${GITHUB_RUN_ID:-}"
[[ "${current_run_id}" =~ ^[0-9]+$ ]] || fail "GITHUB_RUN_ID is required for performance artifact binding"
performance_artifact_dir="$(download_matching_artifact "${repository}" "${performance_artifact_name}" "${expected_tree}" performance "${tmp_dir}/performance" "${current_run_id}")"
performance_metadata_file="${performance_artifact_dir}/velum-report-metadata.env"
read_metadata "${performance_metadata_file}" || fail "failed to read performance artifact metadata"
performance_component="${performance_artifact_dir}/${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH}"
performance_timestamp="${VELUM_ARTIFACT_TIMESTAMP:-}"
source_commit="${VELUM_ARTIFACT_COMMIT_SHA:-unknown}"
performance_run="${VELUM_ARTIFACT_RUN_ID:-unknown}"
source_pull_request="${VELUM_ARTIFACT_PR_NUMBER:-}"
source_task="${VELUM_ARTIFACT_TASK:-}"

correctness_artifact_dir="$(download_matching_artifact_with_retry "${repository}" \
  "${correctness_artifact_name}" "${expected_tree}" correctness "${tmp_dir}/correctness")"
correctness_metadata_file="${correctness_artifact_dir}/velum-report-metadata.env"
read_metadata "${correctness_metadata_file}" || fail "failed to read correctness artifact metadata"
correctness_component="${correctness_artifact_dir}/${VELUM_ARTIFACT_REPORT_COMPONENT_YAML_RELATIVE_PATH}"
correctness_run="${VELUM_ARTIFACT_RUN_ID:-unknown}"
source_test262_baseline="${correctness_artifact_dir}/test262-pass-baseline.txt"
valid_test262_baseline_candidate "${source_test262_baseline}" ||
  fail "correctness artifact has no valid Test262 pass baseline candidate"

[[ "${performance_timestamp}" =~ ^[0-9]{8}T[0-9]{6}Z$ ]] ||
  fail "invalid performance artifact timestamp: ${performance_timestamp}"
report_file="velum-test-report-${performance_timestamp}.md"
report_yaml_file="${report_file%.md}.yaml"
composed_dir="${tmp_dir}/composed/test-runs"
source_report="${composed_dir}/${report_file}"
source_report_yaml="${composed_dir}/${report_yaml_file}"
mkdir -p "${composed_dir}"
cargo run --manifest-path runner/Cargo.toml -- --compose-reports \
  "${expected_tree}" "${correctness_component}" "${performance_component}" "${source_report}"

jetstream_report_file=""
source_jetstream_report=""
source_run="correctness:${correctness_run}, performance:${performance_run}"
archive_ref="$(resolve_archive_ref)"
legacy_archive_ref="$(resolve_legacy_archive_ref)"

archive_tested_source_commit "${archive_ref}" "${legacy_archive_ref}" "${source_commit}" "${expected_tree}" "${source_run}"
checkout_latest_main
stage_report_outputs "${source_report}" "${report_file}" "${source_report_yaml}" "${report_yaml_file}" "${source_jetstream_report}" "${jetstream_report_file}" "${source_test262_baseline}"
commit_and_push "${report_file}" "${report_yaml_file}" "${expected_tree}" "${source_commit}" "${source_run}" \
  "${source_task}" "${source_pull_request}"
