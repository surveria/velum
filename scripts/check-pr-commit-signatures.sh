#!/usr/bin/env bash
set -euo pipefail

fail() {
  printf 'check-pr-commit-signatures: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

need_cmd gh

if [[ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]]; then
  printf 'commit signature check skipped outside pull_request events\n'
  exit 0
fi

repository="${GITHUB_REPOSITORY:-}"
[[ -n "${repository}" ]] || fail "GITHUB_REPOSITORY is required"

pr_number="${VELUM_REPORT_PR_NUMBER:-}"
[[ -n "${pr_number}" ]] || fail "VELUM_REPORT_PR_NUMBER is required"

commit_lines="$(gh api --paginate "/repos/${repository}/pulls/${pr_number}/commits" \
  --jq '.[] | [.sha, .commit.verification.verified, .commit.verification.reason, (.commit.verification.signature != null), .commit.author.email, .commit.committer.email] | @tsv')"
[[ -n "${commit_lines}" ]] || fail "pull request has no commits"

failed=0
while IFS=$'\t' read -r sha verified reason has_signature author_email committer_email; do
  [[ -n "${sha}" ]] || continue
  if [[ "${verified}" == "true" && "${has_signature}" == "true" ]]; then
    printf 'verified signed PR commit: %.12s\n' "${sha}"
    continue
  fi

  printf 'unverified PR commit: %s reason=%s has_signature=%s author=%s committer=%s\n' \
    "${sha}" "${reason}" "${has_signature}" "${author_email}" "${committer_email}" >&2
  failed=1
done <<< "${commit_lines}"

if [[ "${failed}" != "0" ]]; then
  fail "all PR commits must have GitHub-verified signatures"
fi
