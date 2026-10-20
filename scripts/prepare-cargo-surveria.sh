#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"

# cargo-surveria provides `cargo surveria check`, which
# scripts/check-registry-freshness.sh runs. It is published to the private
# Surveria registry; its sources live in surveria/healthcheck. This script
# installs the pinned release into a version-keyed cache directory, reuses it
# on later runs, and prints the binary path. CI keeps the cache on the
# persistent runner volume next to QuickJS and Test262.
cargo_surveria_version="0.1.0"
# cargo-surveria 0.1.0 declares rust-version 1.98, which is newer than the CI
# runner's default toolchain, so the CLI is built with this pinned release.
# Revisit both pins when `cargo surveria check` reports a newer cargo-surveria.
cargo_surveria_toolchain="1.98.0"

cache_dir="${VELUM_CARGO_SURVERIA_CACHE_DIR:-${repo_root}/target/cargo-surveria}"
install_root="${cache_dir}/cargo-surveria-${cargo_surveria_version}"
cargo_surveria_binary="${install_root}/bin/cargo-surveria"
expected_version="cargo-surveria ${cargo_surveria_version}"

log() {
  printf '%s\n' "$*" >&2
}

require_tool() {
  local tool="$1"
  if command -v "${tool}" >/dev/null 2>&1; then
    return 0
  fi
  log "Missing required tool: ${tool}"
  return 1
}

install_cargo_surveria() {
  local tmp_dir="$1"
  log "Installing cargo-surveria ${cargo_surveria_version} with Rust ${cargo_surveria_toolchain}"
  rustup toolchain install "${cargo_surveria_toolchain}" --profile minimal --no-self-update >&2
  # cargo install reads only the user and system Cargo configuration, so the
  # repository file that defines the surveria registry and its cargo:token
  # credential provider is passed explicitly. The registry token comes from
  # CARGO_REGISTRIES_SURVERIA_TOKEN or $CARGO_HOME/credentials.toml.
  rustup run "${cargo_surveria_toolchain}" cargo install \
    --config "${repo_root}/.cargo/config.toml" \
    --registry surveria \
    --version "=${cargo_surveria_version}" \
    --locked \
    --root "${tmp_dir}/root" \
    --target-dir "${tmp_dir}/target" \
    cargo-surveria >&2
}

if [[ ! -x "${cargo_surveria_binary}" ]]; then
  require_tool rustup
  mkdir -p "${cache_dir}"
  tmp_dir="$(mktemp -d "${cache_dir}/.cargo-surveria-${cargo_surveria_version}.XXXXXX")"
  trap 'rm -rf "${tmp_dir}"' EXIT
  install_cargo_surveria "${tmp_dir}"
  # Publish the install root only after a complete install, so an interrupted
  # run never leaves a half-written binary in the cache.
  rm -rf "${install_root}"
  mv "${tmp_dir}/root" "${install_root}"
  log "Cached cargo-surveria ${cargo_surveria_version} in ${install_root}"
fi

actual_version="$("${cargo_surveria_binary}" --version)"
if [[ "${actual_version}" != "${expected_version}" ]]; then
  log "cargo-surveria version mismatch in ${install_root}."
  log "Expected: ${expected_version}"
  log "Actual:   ${actual_version}"
  exit 1
fi

printf '%s\n' "${cargo_surveria_binary}"
