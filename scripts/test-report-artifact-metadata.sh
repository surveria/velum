#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=publish-report-artifact.sh
source "${script_dir}/publish-report-artifact.sh"

fail_test() {
  printf 'test-report-artifact-metadata: %s\n' "$*" >&2
  exit 1
}

encoded() {
  printf '%s' "$1" | base64 | tr -d '\n'
}

expect_rejected() {
  local path="$1"
  local label="$2"
  if read_metadata "${path}"; then
    fail_test "accepted ${label}"
  fi
}

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

valid_metadata="${tmp_dir}/valid.env"
{
  printf 'RSQJS_ARTIFACT_SCHEMA=%s\n' "$(encoded 3)"
  printf 'RSQJS_ARTIFACT_REPORT_MODE=%s\n' "$(encoded correctness)"
  printf 'RSQJS_ARTIFACT_TASK=%s\n' "$(encoded 'task with spaces')"
} > "${valid_metadata}"
read_metadata "${valid_metadata}" || fail_test "rejected valid metadata"
[[ "${RSQJS_ARTIFACT_TASK}" == "task with spaces" ]] || fail_test "changed decoded value"

duplicate_metadata="${tmp_dir}/duplicate.env"
{
  printf 'RSQJS_ARTIFACT_SCHEMA=%s\n' "$(encoded 3)"
  printf 'RSQJS_ARTIFACT_SCHEMA=%s\n' "$(encoded 3)"
} > "${duplicate_metadata}"
expect_rejected "${duplicate_metadata}" "duplicate key"

unknown_metadata="${tmp_dir}/unknown.env"
printf 'RSQJS_ARTIFACT_UNKNOWN=%s\n' "$(encoded value)" > "${unknown_metadata}"
expect_rejected "${unknown_metadata}" "unknown key"

invalid_base64_metadata="${tmp_dir}/invalid-base64.env"
printf '%s\n' 'RSQJS_ARTIFACT_SCHEMA=%%%' > "${invalid_base64_metadata}"
expect_rejected "${invalid_base64_metadata}" "invalid base64"

marker="${tmp_dir}/executed"
command_metadata="${tmp_dir}/command.env"
printf 'RSQJS_ARTIFACT_SCHEMA=$(touch %s)\n' "${marker}" > "${command_metadata}"
expect_rejected "${command_metadata}" "command substitution"
[[ ! -e "${marker}" ]] || fail_test "executed artifact metadata"

if valid_artifact_relative_path "../../outside" test-runs report.md; then
  fail_test "accepted path traversal"
fi
valid_artifact_relative_path "test-runs/report.md" test-runs report.md ||
  fail_test "rejected canonical relative path"

tree_sha="0123456789abcdef0123456789abcdef01234567"
validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 100 owner/repo \
  .github/workflows/ci.yml CI pull_request completed success "${tree_sha}" ||
  fail_test "rejected trusted correctness workflow run"
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 101 owner/repo \
  .github/workflows/ci.yml CI pull_request completed failure "${tree_sha}"; then
  fail_test "accepted failed newest correctness workflow run"
fi
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 102 owner/repo \
  .github/workflows/unrelated.yml CI pull_request completed success "${tree_sha}"; then
  fail_test "accepted unrelated newest correctness workflow run"
fi
validate_workflow_run_fields performance owner/repo "${tree_sha}" 200 200 owner/repo \
  .github/workflows/ci.yml CI pull_request in_progress "" "${tree_sha}" ||
  fail_test "rejected current performance workflow run"
if validate_workflow_run_fields performance owner/repo "${tree_sha}" 200 201 owner/repo \
  .github/workflows/ci.yml CI pull_request completed success "${tree_sha}"; then
  fail_test "accepted performance artifact from another run"
fi

printf 'report artifact metadata parser tests passed\n'
