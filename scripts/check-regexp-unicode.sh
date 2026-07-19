#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

manifest="crates/velum-regexp/unicode/SOURCES.manifest"
checksums="crates/velum-regexp/unicode/GENERATED-SHA256SUMS"

if grep -Fq '/latest/' "${manifest}"; then
  printf 'check-regexp-unicode: source manifest contains a mutable latest URL\n' >&2
  exit 1
fi

sha256sum --check "${checksums}"

if ! grep -Fq '// Unicode version: 17.0.0' \
  crates/velum-regexp/src/unicode/generated_core.rs; then
  printf 'check-regexp-unicode: generated Unicode version header is missing\n' >&2
  exit 1
fi

printf 'check-regexp-unicode: provenance and generated checksums are valid\n'
