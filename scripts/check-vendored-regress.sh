#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
vendor_dir="${repo_root}/src/regress"
manifest="${vendor_dir}/VENDORED-SOURCE-SHA256SUMS"

fail() {
  printf 'check-vendored-regress: %s\n' "$*" >&2
  exit 1
}

for required in \
  Cargo.toml \
  Cargo.toml.orig \
  LICENSE-APACHE \
  LICENSE-MIT \
  README.md \
  VENDORED.md \
  VENDORED-SOURCE-SHA256SUMS; do
  [[ -f "${vendor_dir}/${required}" ]] || fail "missing src/regress/${required}"
done

grep -Fq 'regress = { path = "src/regress"' "${repo_root}/Cargo.toml" \
  || fail "root Cargo.toml must use the local src/regress path dependency"

grep -Fq 'version = "0.11.1"' "${vendor_dir}/Cargo.toml" \
  || fail "vendored package version must remain explicit"
grep -Fq 'publish = false' "${vendor_dir}/Cargo.toml" \
  || fail "vendored package must not be publishable"

mapfile -t expected_sources < <(awk '{ print $2 }' "${manifest}" | sort)
mapfile -t actual_sources < <(
  cd "${repo_root}"
  find src/regress/src -maxdepth 1 -type f -name '*.rs' -print | sort
)
if [[ "${expected_sources[*]}" != "${actual_sources[*]}" ]]; then
  fail "source manifest does not match the vendored Rust file set"
fi

(
  cd "${repo_root}"
  sha256sum --check --strict "${manifest}"
) >/dev/null || fail "vendored source checksum mismatch"

printf 'check-vendored-regress: ok (%s source files)\n' "${#actual_sources[@]}"
