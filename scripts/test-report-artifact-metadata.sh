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

valid_artifact_run_attempt performance 2 2 ||
  fail_test "rejected current performance workflow attempt"
valid_artifact_run_attempt performance 1 2 ||
  fail_test "rejected successful performance output reused by a publisher rerun"
if valid_artifact_run_attempt performance 3 2; then
  fail_test "accepted a performance artifact from a future workflow attempt"
fi
if valid_artifact_run_attempt correctness 1 2; then
  fail_test "accepted correctness metadata from a different workflow attempt"
fi
if valid_artifact_run_attempt performance invalid 2; then
  fail_test "accepted a non-numeric artifact workflow attempt"
fi

valid_baseline="${tmp_dir}/test262-pass-baseline.txt"
{
  sed -n '1,3p' "${test262_baseline_path}"
  printf '%s\n' 'built-ins/Array/a.js#default'
  printf '%s\n' 'built-ins/RegExp/r.js#strict'
} > "${valid_baseline}"
valid_test262_baseline_candidate "${valid_baseline}" ||
  fail_test "rejected valid Test262 pass baseline candidate"
printf '%s\n' 'built-ins/Array/a.js#default' >> "${valid_baseline}"
if valid_test262_baseline_candidate "${valid_baseline}"; then
  fail_test "accepted unsorted or duplicate Test262 pass baseline candidate"
fi

legacy_baseline="${tmp_dir}/legacy-test262-pass-baseline.txt"
{
  printf '%s\n' '# rsqjs-test262-pass-baseline-v1'
  sed -n '2p' "${test262_baseline_path}"
  printf '%s\n' 'built-ins/Array/a.js#default'
} > "${legacy_baseline}"
if valid_test262_baseline_candidate "${legacy_baseline}"; then
  fail_test "accepted legacy Test262 pass baseline schema"
fi

wrong_patch_baseline="${tmp_dir}/wrong-patch-test262-pass-baseline.txt"
{
  sed -n '1,2p' "${test262_baseline_path}"
  printf '%s\n' '# test262_patches=untrusted-corpus-patch'
  printf '%s\n' 'built-ins/Array/a.js#default'
} > "${wrong_patch_baseline}"
if valid_test262_baseline_candidate "${wrong_patch_baseline}"; then
  fail_test "accepted Test262 pass baseline with different patch provenance"
fi

empty_baseline="${tmp_dir}/empty-test262-pass-baseline.txt"
sed -n '1,3p' "${test262_baseline_path}" > "${empty_baseline}"
if valid_test262_baseline_candidate "${empty_baseline}"; then
  fail_test "accepted Test262 pass baseline without case rows"
fi

tree_sha="0123456789abcdef0123456789abcdef01234567"
head_sha="89abcdef0123456789abcdef0123456789abcdef"
head_tree="fedcba9876543210fedcba9876543210fedcba98"
validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 100 owner/repo \
  .github/workflows/ci.yml CI pull_request completed success "${tree_sha}" ||
  fail_test "rejected trusted correctness workflow run"
validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 100 owner/repo \
  .github/workflows/ci.yml CI workflow_dispatch completed success "${head_tree}" ||
  fail_test "rejected trusted historical-source correctness recovery run"
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 100 owner/repo \
  .github/workflows/ci.yml CI workflow_dispatch completed failure "${head_tree}"; then
  fail_test "accepted failed historical-source correctness recovery run"
fi
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 100 owner/repo \
  .github/workflows/ci.yml CI pull_request completed success "${head_tree}"; then
  fail_test "accepted correctness from a different workflow head tree"
fi
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 100 owner/repo \
  .github/workflows/ci.yml CI push completed success "${tree_sha}"; then
  fail_test "accepted correctness from an unsupported workflow event"
fi
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 101 owner/repo \
  .github/workflows/ci.yml CI pull_request completed failure "${tree_sha}"; then
  fail_test "accepted failed newest correctness workflow run"
fi
if validate_workflow_run_fields correctness owner/repo "${tree_sha}" "" 102 owner/repo \
  .github/workflows/unrelated.yml CI pull_request completed success "${tree_sha}"; then
  fail_test "accepted unrelated newest correctness workflow run"
fi
validate_workflow_run_fields performance owner/repo "${tree_sha}" 200 200 owner/repo \
  .github/workflows/ci.yml CI pull_request in_progress "${null_workflow_conclusion}" "${head_tree}" ||
  fail_test "rejected current performance workflow run with a distinct PR head tree"
if validate_workflow_run_fields performance owner/repo "${tree_sha}" 200 200 owner/repo \
  .github/workflows/ci.yml CI pull_request in_progress "" "${head_tree}"; then
  fail_test "accepted an empty parsed performance conclusion"
fi
if validate_workflow_run_fields performance owner/repo "${tree_sha}" 200 201 owner/repo \
  .github/workflows/ci.yml CI pull_request completed success "${head_tree}"; then
  fail_test "accepted performance artifact from another run"
fi

gh() {
  local endpoint="${2:-}"
  local query="${4:-}"
  case "${endpoint}" in
    /repos/owner/repo/actions/runs/200)
      [[ "${query}" == *"${null_workflow_conclusion}"* ]] ||
        fail_test "workflow run query omitted the null conclusion sentinel"
      printf '200\towner/repo\t.github/workflows/ci.yml\tCI\tpull_request\tin_progress\t%s\t%s\t7\n' \
        "${null_workflow_conclusion}" "${head_sha}"
      ;;
    /repos/owner/repo/git/commits/"${head_sha}")
      printf '%s\n' "${head_tree}"
      ;;
    *)
      fail_test "unexpected mocked gh request: ${endpoint}"
      ;;
  esac
}

load_trusted_workflow_run owner/repo 200 "${tree_sha}" performance 200 ||
  fail_test "rejected a merge-tree-bound artifact after a null workflow conclusion"
[[ "${RUN_CONCLUSION}" == "${null_workflow_conclusion}" ]] ||
  fail_test "changed the parsed null conclusion sentinel"
[[ "${RUN_HEAD_SHA}" == "${head_sha}" ]] || fail_test "shifted the parsed head SHA"
[[ "${RUN_ATTEMPT}" == 7 ]] || fail_test "shifted the parsed run attempt"

retry_state="${tmp_dir}/artifact-retry-state"
printf '0\n' > "${retry_state}"
download_matching_artifact() {
  local target_dir="$5"
  local attempt
  attempt="$(sed -n '1p' "${retry_state}")"
  attempt=$((attempt + 1))
  printf '%s\n' "${attempt}" > "${retry_state}"
  if ((attempt < 3)); then
    return 1
  fi
  printf '%s/artifact-ready\n' "${target_dir}"
}

retry_result="$(RSQJS_ARTIFACT_WAIT_ATTEMPTS=3 RSQJS_ARTIFACT_WAIT_SECONDS=0 \
  download_matching_artifact_with_retry owner/repo delayed-artifact "${tree_sha}" \
  correctness "${tmp_dir}/retry-target")"
[[ "${retry_result}" == "${tmp_dir}/retry-target/artifact-ready" ]] ||
  fail_test "artifact retry did not return the delayed artifact"
[[ "$(sed -n '1p' "${retry_state}")" == 3 ]] ||
  fail_test "artifact retry used an unexpected number of attempts"

download_matching_artifact() {
  return 1
}
if retry_result="$(RSQJS_ARTIFACT_WAIT_ATTEMPTS=2 RSQJS_ARTIFACT_WAIT_SECONDS=0 \
  download_matching_artifact_with_retry owner/repo missing-artifact "${tree_sha}" \
  correctness "${tmp_dir}/missing-target" 2>/dev/null)"; then
  fail_test "artifact retry accepted an artifact that never appeared"
fi

headline="$(report_commit_headline \
  'AS-06a2a: attach VM-owned bytecode continuations' 439 20260710T212554Z)"
[[ "${headline}" == \
  'AS-06a2a: attach VM-owned bytecode continuations (#439) (CI) [skip ci]' ]] ||
  fail_test "did not derive the report commit headline from pull request metadata"

headline="$(report_commit_headline \
  'runtime: preserve an existing pull request suffix (#439)' 439 20260710T212554Z)"
[[ "${headline}" == \
  'runtime: preserve an existing pull request suffix (#439) (CI) [skip ci]' ]] ||
  fail_test "duplicated an existing pull request suffix"

headline="$(report_commit_headline '' '' 20260710T212554Z)"
[[ "${headline}" == 'Add rsqjs report 20260710T212554Z (CI) [skip ci]' ]] ||
  fail_test "changed the report commit headline fallback"

publisher_remote="${tmp_dir}/publisher-remote.git"
publisher_worktree="${tmp_dir}/publisher-worktree"
git init --quiet --bare --initial-branch=main "${publisher_remote}"
git init --quiet --initial-branch=main "${publisher_worktree}"
pushd "${publisher_worktree}" >/dev/null
git config user.name test-publisher
git config user.email test-publisher@example.invalid
printf 'base\n' > base.txt
git add base.txt
git commit --quiet -m base
base_commit="$(git rev-parse HEAD)"
git remote add origin "${publisher_remote}"
git push --quiet -u origin main
printf 'published\n' > report.txt
create_main_report_commit 'report headline' 'report body' report.txt >/dev/null ||
  fail_test "failed to push a report-only commit"
published_commit="$(git --git-dir="${publisher_remote}" rev-parse refs/heads/main)"
[[ "${published_commit}" != "${base_commit}" ]] || fail_test "did not advance remote main"
[[ "$(git rev-parse HEAD)" == "${base_commit}" ]] ||
  fail_test "moved the local main branch while publishing"
[[ "$(git --git-dir="${publisher_remote}" show "${published_commit}:report.txt")" == published ]] ||
  fail_test "published report commit has the wrong contents"
[[ "$(git --git-dir="${publisher_remote}" show -s --format=%s "${published_commit}")" == \
  'report headline' ]] || fail_test "published report commit has the wrong headline"
printf 'stale publisher\n' > report.txt
if create_main_report_commit 'stale headline' 'stale body' report.txt >/dev/null 2>&1; then
  fail_test "accepted a non-fast-forward report publication"
fi
git diff --cached --quiet || fail_test "left staged changes after a rejected publication"
[[ "$(git --git-dir="${publisher_remote}" rev-parse refs/heads/main)" == "${published_commit}" ]] ||
  fail_test "changed remote main after a rejected report publication"
popd >/dev/null

printf 'report artifact metadata parser tests passed\n'
