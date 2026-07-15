#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

fail() {
  printf 'check-touched-file-sizes: %s\n' "$*" >&2
  exit 1
}

base_ref="${1:-${VELUM_BASE_REF:-origin/main}}"
max_lines="${VELUM_MAX_RUST_FILE_LINES:-800}"

if ! [[ "${max_lines}" =~ ^[0-9]+$ ]]; then
  fail "VELUM_MAX_RUST_FILE_LINES must be a positive integer"
fi
if [[ "${max_lines}" -eq 0 ]]; then
  fail "VELUM_MAX_RUST_FILE_LINES must be greater than zero"
fi

if ! git rev-parse --verify --quiet "${base_ref}^{commit}" >/dev/null; then
  fail "base ref '${base_ref}' not found; fetch it first"
fi

merge_base="$(git merge-base "${base_ref}" HEAD)"
mapfile -t rust_files < <(git diff --name-only --diff-filter=ACMR "${merge_base}" HEAD -- '*.rs')

if [[ "${#rust_files[@]}" -eq 0 ]]; then
  printf 'check-touched-file-sizes: no touched Rust files relative to %s\n' "${base_ref}"
  exit 0
fi

failed=0
for rust_file in "${rust_files[@]}"; do
  [[ -f "${rust_file}" ]] || continue
  if [[ "${rust_file}" == src/regress/src/*.rs ]]; then
    if ! grep -Fq "  ${rust_file}" src/regress/VENDORED-SOURCE-SHA256SUMS; then
      fail "vendored Rust file is missing from the source manifest: ${rust_file}"
    fi
    printf 'check-touched-file-sizes: vendored snapshot %s (dedicated checksum gate)\n' \
      "${rust_file}"
    continue
  fi
  line_count="$(wc -l <"${rust_file}")"
  line_count="${line_count//[[:space:]]/}"
  if [[ "${line_count}" -le "${max_lines}" ]]; then
    printf 'check-touched-file-sizes: ok %s (%s/%s lines)\n' \
      "${rust_file}" "${line_count}" "${max_lines}"
    continue
  fi

  printf 'check-touched-file-sizes: too large %s (%s/%s lines)\n' \
    "${rust_file}" "${line_count}" "${max_lines}" >&2
  failed=1
done

if [[ "${failed}" != "0" ]]; then
  fail "touched Rust files must stay at or below ${max_lines} lines"
fi
