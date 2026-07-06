#!/usr/bin/env bash
# Fail when the engine crate changed but its version was not increased.
#
# Rationale: every change to the engine's own sources or its dependency set must
# ship a new engine version, and the version must actually go *up* relative to
# what is already on the base branch (so it stays monotonic even when other PRs
# land concurrently).
#
# Usage: scripts/check-version-bump.sh [base-ref]
#   base-ref defaults to origin/main and must already be fetched.
#
# Exit codes: 0 = ok (bumped, or no engine change), 1 = missing bump, 2 = usage.
set -euo pipefail

base="${1:-origin/main}"

if ! git rev-parse --verify --quiet "${base}^{commit}" >/dev/null; then
  echo "error: base ref '${base}' not found; fetch it first (e.g. 'git fetch origin main')" >&2
  exit 2
fi

# Files that make up the engine crate. A change to any of these requires a bump.
engine_paths=(src Cargo.toml Cargo.lock)

merge_base="$(git merge-base "${base}" HEAD)"
mapfile -t changed < <(git diff --name-only "${merge_base}" HEAD -- "${engine_paths[@]}")

if [[ "${#changed[@]}" -eq 0 ]]; then
  echo "check-version-bump: no engine crate changes (${engine_paths[*]}); version bump not required."
  exit 0
fi

# Print the [package] version from a Cargo.toml supplied on stdin.
package_version() {
  awk '
    /^\[/ { in_pkg = ($0 == "[package]") }
    in_pkg && /^version[[:space:]]*=/ {
      if (match($0, /"[^"]+"/)) { print substr($0, RSTART + 1, RLENGTH - 2); exit }
    }'
}

base_version="$(git show "${base}:Cargo.toml" | package_version)"
head_version="$(package_version <Cargo.toml)"

echo "check-version-bump: engine files changed:"
printf '  %s\n' "${changed[@]}"
echo "check-version-bump: engine version base(${base})=${base_version:-<none>} head=${head_version:-<none>}"

if [[ -z "${base_version}" || -z "${head_version}" ]]; then
  echo "error: could not read [package] version from Cargo.toml" >&2
  exit 2
fi

if [[ "${head_version}" == "${base_version}" ]]; then
  echo "error: the engine crate changed but its version is still ${head_version}." >&2
  echo "       bump [package].version in Cargo.toml above ${base_version}." >&2
  exit 1
fi

highest="$(printf '%s\n%s\n' "${base_version}" "${head_version}" | sort -V | tail -n1)"
if [[ "${highest}" != "${head_version}" ]]; then
  echo "error: the engine version went backwards (${base_version} -> ${head_version}); it must increase." >&2
  exit 1
fi

echo "check-version-bump: OK, engine version ${base_version} -> ${head_version}."
