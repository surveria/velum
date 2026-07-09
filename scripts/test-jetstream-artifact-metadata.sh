#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
current_head="$(git -C "${script_dir}/.." rev-parse HEAD)"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

write_value() {
  local path="$1"
  local key="$2"
  local value="$3"
  local encoded
  encoded="$(printf '%s' "${value}" | base64 | tr -d '\n')"
  printf '%s=%s\n' "${key}" "${encoded}" >> "${path}"
}

write_envelope() {
  local path="$1"
  local event="$2"
  local include_workflow="$3"
  : > "${path}"
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_SCHEMA 3
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_FILTER ''
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_BASELINE_MODE read
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_REPOSITORY test/repository
  if [[ "${include_workflow}" == 1 ]]; then
    write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_WORKFLOW JetStream
  fi
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_RUN_ID 42
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_RUN_ATTEMPT 1
  write_value "${path}" RSQJS_JETSTREAM_ARTIFACT_EVENT_NAME "${event}"
}

expect_rejected() {
  local metadata_path="$1"
  local expected="$2"
  local github_ref="${3:-refs/heads/main}"
  local output
  if output="$(
    GITHUB_REPOSITORY=test/repository \
      GITHUB_WORKFLOW=JetStream \
      GITHUB_RUN_ID=42 \
      GITHUB_RUN_ATTEMPT=1 \
      GITHUB_EVENT_NAME=schedule \
      GITHUB_REF="${github_ref}" \
      GITHUB_SHA="${current_head}" \
      RSQJS_DEFAULT_BRANCH=main \
      RSQJS_JETSTREAM_METADATA_PATH="${metadata_path}" \
      RSQJS_JETSTREAM_ARTIFACT_WORKFLOW=JetStream \
      "${script_dir}/publish-jetstream-report.sh" 2>&1
  )"; then
    printf 'metadata fixture unexpectedly passed: %s\n' "${metadata_path}" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected}"* ]]; then
    printf 'metadata rejection did not contain %q:\n%s\n' "${expected}" "${output}" >&2
    exit 1
  fi
}

missing_path="${tmp_dir}/missing-workflow.env"
write_envelope "${missing_path}" schedule 0
expect_rejected "${missing_path}" 'artifact workflow mismatch'

stale_path="${tmp_dir}/stale-event.env"
write_envelope "${stale_path}" pull_request 1
expect_rejected "${stale_path}" 'artifact event mismatch'

arbitrary_ref_path="${tmp_dir}/arbitrary-ref.env"
write_envelope "${arbitrary_ref_path}" schedule 1
expect_rejected \
  "${arbitrary_ref_path}" \
  'canonical publish requires the default branch ref' \
  refs/heads/codex/feature

stale_commit_path="${tmp_dir}/stale-commit.env"
write_envelope "${stale_commit_path}" schedule 1
write_value \
  "${stale_commit_path}" \
  RSQJS_JETSTREAM_ARTIFACT_COMMIT_SHA \
  0000000000000000000000000000000000000000
expect_rejected \
  "${stale_commit_path}" \
  'artifact commit does not match the workflow commit'

printf 'JetStream artifact metadata rejection tests passed\n'
