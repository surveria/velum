#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

# Surveria registry freshness gate, run next to clippy. `cargo surveria check
# --strict` (cargo-surveria, sources in surveria/healthcheck) reads a
# workspace's Cargo.lock and fails when the private Surveria registry has a
# newer compatible release of a locked registry crate (`cargo update -p
# <crate>` fixes it) or when a crates.io package shadows a registry crate
# name; a newer incompatible release is a warning. Like Cargo for the runner,
# it needs a registry read token.
#
# Usage: scripts/check-registry-freshness.sh [<workspace>/Cargo.toml ...]
# Arguments are workspace root manifests relative to the repository root;
# without arguments every workspace with a tracked Cargo.lock is a candidate.
#
# Locally, a candidate whose Cargo.lock takes no registry crates is skipped,
# so the engine and the CLI keep building without registry access, and a
# missing cargo-surveria CLI produces a warning with the install command. The
# warning is also appended to VELUM_DEFERRED_WARNINGS_FILE when that is set,
# so scripts/check-fast.sh can repeat it after its long test output. GitHub
# Actions has the token and installs the CLI with
# scripts/prepare-cargo-surveria.sh first, so CI checks every candidate (the
# crates.io name-shadowing check applies to all of them) and fails without
# the CLI.

# cargo-surveria matches this Cargo.lock source; it is the index that
# .cargo/config.toml defines, and both honor the same override variable.
registry_source="${CARGO_REGISTRIES_SURVERIA_INDEX:-sparse+https://crates.surveria.org/api/packages/surveria/cargo/}"
install_command='cargo install --registry surveria cargo-surveria --locked'
in_ci=0
if [[ "${GITHUB_ACTIONS:-false}" == "true" ]]; then
  in_ci=1
fi

log() {
  printf '%s\n' "$*" >&2
}

defer_warning() {
  local line
  for line in "$@"; do
    log "${line}"
    if [[ -n "${VELUM_DEFERRED_WARNINGS_FILE:-}" ]]; then
      printf '%s\n' "${line}" >> "${VELUM_DEFERRED_WARNINGS_FILE}"
    fi
  done
}

takes_registry_crates() {
  local lockfile="$1"
  grep -qF "source = \"${registry_source}\"" "${lockfile}"
}

# Path of a file next to a workspace manifest, without a leading "./".
sibling_path() {
  local manifest="$1"
  local name="$2"
  local dir
  dir="$(dirname "${manifest}")"
  if [[ "${dir}" == "." ]]; then
    printf '%s\n' "${name}"
  else
    printf '%s/%s\n' "${dir}" "${name}"
  fi
}

candidates=()
if [[ "$#" -gt 0 ]]; then
  candidates=("$@")
else
  # Capture the list first: a failing command inside a process substitution
  # would not stop the script and would look like "nothing to check".
  tracked_lockfiles="$(git ls-files -- '*Cargo.lock')"
  while IFS= read -r lockfile; do
    [[ -n "${lockfile}" ]] || continue
    candidates+=("$(sibling_path "${lockfile}" Cargo.toml)")
  done <<< "${tracked_lockfiles}"
fi

manifests=()
skipped=()
if [[ "${#candidates[@]}" -gt 0 ]]; then
  for manifest in "${candidates[@]}"; do
    lockfile="$(sibling_path "${manifest}" Cargo.lock)"
    if [[ ! -f "${manifest}" || ! -f "${lockfile}" ]]; then
      log "error: ${manifest} is not a workspace root manifest with a Cargo.lock next to it"
      exit 1
    fi
    if [[ "${in_ci}" == "1" ]] || takes_registry_crates "${lockfile}"; then
      manifests+=("${manifest}")
    else
      skipped+=("${lockfile}")
    fi
  done
fi

if [[ "${#skipped[@]}" -gt 0 ]]; then
  log "Surveria registry freshness: no registry crates in ${skipped[*]}; skipped locally, CI checks them for crates.io name shadowing"
fi

if [[ "${#manifests[@]}" -eq 0 ]]; then
  log "Surveria registry freshness: no workspace to check"
  exit 0
fi

if ! cargo_surveria_version="$(cargo surveria --version 2>/dev/null)"; then
  if [[ "${in_ci}" == "1" ]]; then
    log "error: cargo-surveria is not installed; CI must run scripts/prepare-cargo-surveria.sh before this check"
    exit 1
  fi
  defer_warning \
    "warning: cargo-surveria is not installed, so the Surveria registry freshness check (cargo surveria check --strict) did not run for: ${manifests[*]}" \
    "  = help: install it with \`${install_command}\`; requirements are in runner/README.md"
  exit 0
fi

log "Surveria registry freshness: ${cargo_surveria_version}"
failed=()
for manifest in "${manifests[@]}"; do
  log "Surveria registry freshness: cargo surveria check --strict --manifest-path ${manifest}"
  if ! cargo surveria check --strict --manifest-path "${manifest}"; then
    failed+=("${manifest}")
  fi
done

if [[ "${#failed[@]}" -gt 0 ]]; then
  log "error: the Surveria registry freshness check failed for: ${failed[*]}"
  exit 1
fi
