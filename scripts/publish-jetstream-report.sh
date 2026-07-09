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
  local report_file="$2"

  mkdir -p reports/jetstream-runs
  target_report="reports/jetstream-runs/${report_file}"
  if [[ -f "${target_report}" ]] && ! cmp -s "${source_report}" "${target_report}"; then
    fail "tracked JetStream report already exists with different content: ${target_report}"
  fi
  cp "${source_report}" "${target_report}"
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
  if git ls-files --error-unmatch "${report_path}" >/dev/null 2>&1; then
    git restore --worktree -- "${report_path}"
  else
    rm -f "${report_path}"
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

# The metadata is produced by scripts/run-jetstream.sh as shell-escaped values.
# shellcheck disable=SC1090
source "${metadata_path}"
[[ "${RSQJS_JETSTREAM_ARTIFACT_SCHEMA:-}" == "1" ]] || fail "unsupported artifact schema"
report_file="${RSQJS_JETSTREAM_ARTIFACT_REPORT_FILE:-}"
report_relative_path="${RSQJS_JETSTREAM_ARTIFACT_REPORT_RELATIVE_PATH:-}"
expected_commit="${RSQJS_JETSTREAM_ARTIFACT_COMMIT_SHA:-}"
expected_tree="${RSQJS_JETSTREAM_ARTIFACT_TREE_SHA:-}"
source_run="${RSQJS_JETSTREAM_ARTIFACT_RUN_ID:-unknown}"
[[ "${RSQJS_JETSTREAM_ARTIFACT_FILTER:-}" == "" ]] || fail "filtered reports are not canonical"
[[ "${RSQJS_JETSTREAM_ARTIFACT_BASELINE_MODE:-}" == "read" ]] || fail "only read-only baseline runs are canonical"
valid_report_file "${report_file}" || fail "invalid JetStream report file name: ${report_file}"
[[ "${expected_commit}" =~ ^[0-9a-f]{40}$ ]] || fail "invalid tested commit"
[[ "${expected_tree}" =~ ^[0-9a-f]{40}$ ]] || fail "invalid tested tree"
git cat-file -e "${expected_commit}^{commit}" || fail "tested commit is unavailable"
actual_tree="$(git rev-parse "${expected_commit}^{tree}")"
[[ "${actual_tree}" == "${expected_tree}" ]] || fail "tested commit tree mismatch"

metadata_dir="$(dirname "${metadata_path}")"
source_report="${metadata_dir}/${report_relative_path}"
[[ -f "${source_report}" ]] || fail "JetStream report is absent: ${source_report}"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT
source_copy="${tmp_dir}/${report_file}"
cp "${source_report}" "${source_copy}"

if [[ -n "${RSQJS_DEPENDENCY_TOKEN:-}" ]]; then
  export GIT_CONFIG_COUNT=1
  export GIT_CONFIG_KEY_0="url.https://x-access-token:${RSQJS_DEPENDENCY_TOKEN}@github.com/chertov/.insteadOf"
  export GIT_CONFIG_VALUE_0="https://github.com/chertov/"
fi

checkout_latest_main
stage_outputs "${source_copy}" "${report_file}"
timestamp="${report_file#rsqjs-jetstream-report-}"
timestamp="${timestamp%.md}"
headline="Add JetStream report ${timestamp} [skip ci]"
body="$(printf 'Source commit: %s\n\nSource tree: %s\n\nSource workflow run: %s\n' \
  "${expected_commit}" "${expected_tree}" "${source_run}")"
commit_paths=("${target_report}" reports/benchmark-rollup.md reports/benchmark-summary.jpg)

if create_signed_commit "${repository}" "${headline}" "${body}" "${commit_paths[@]}"; then
  exit 0
fi

printf 'initial signed JetStream report commit failed; retrying once on latest origin/main\n' >&2
reset_outputs "${target_report}"
checkout_latest_main
stage_outputs "${source_copy}" "${report_file}"
create_signed_commit "${repository}" "${headline}" "${body}" "${commit_paths[@]}"
