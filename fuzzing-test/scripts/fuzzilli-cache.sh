#!/usr/bin/env bash
set -euo pipefail

cache_schema_version="1"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fuzzing_dir="$(dirname "${script_dir}")"
revision_file="${fuzzing_dir}/FUZZILLI_REVISION"
profile_patch="${fuzzing_dir}/patches/fuzzilli-velum-profile.patch"

fail() {
    printf '%s\n' "$1" >&2
    exit 1
}

revision="$(tr -d '[:space:]' < "${revision_file}")"
if [[ ! "${revision}" =~ ^[0-9a-f]{40}$ ]]; then
    fail "Invalid Fuzzilli revision in ${revision_file}"
fi

if ! command -v git >/dev/null 2>&1; then
    fail 'Missing required command: git'
fi
profile_hash="$(git hash-object --no-filters "${profile_patch}")"
if [[ ! "${profile_hash}" =~ ^[0-9a-f]{40}$ ]]; then
    fail "Failed to hash the Fuzzilli profile patch: ${profile_patch}"
fi

if [[ -n "${VELUM_FUZZILLI_CACHE_DIR:-}" ]]; then
    cache_root="${VELUM_FUZZILLI_CACHE_DIR}"
elif [[ -n "${XDG_CACHE_HOME:-}" ]]; then
    cache_root="${XDG_CACHE_HOME}/velum/fuzzilli"
elif [[ -n "${HOME:-}" ]]; then
    cache_root="${HOME}/.cache/velum/fuzzilli"
else
    fail 'Cannot determine the Fuzzilli cache directory; set VELUM_FUZZILLI_CACHE_DIR.'
fi

if [[ "${cache_root}" != /* ]]; then
    fail "The Fuzzilli cache directory must be absolute: ${cache_root}"
fi

platform="$(uname -s)-$(uname -m)"
if [[ ! "${platform}" =~ ^[A-Za-z0-9._-]+$ ]]; then
    fail "Unsupported platform name for the Fuzzilli cache: ${platform}"
fi

cache_dir="${cache_root}/v${cache_schema_version}/${platform}/${revision}/${profile_hash}"
cached_binary="${cache_dir}/FuzzilliCli"

link_cached_binary() {
    local destination="$1"
    local destination_dir
    destination_dir="$(dirname "${destination}")"
    mkdir -p "${destination_dir}"
    ln -sfn -- "${cached_binary}" "${destination}"
}

case "${1:-}" in
    path)
        if (( $# != 1 )); then
            fail 'Usage: fuzzilli-cache.sh path'
        fi
        printf '%s\n' "${cached_binary}"
        ;;
    restore)
        if (( $# != 2 )); then
            fail 'Usage: fuzzilli-cache.sh restore DESTINATION'
        fi
        if [[ ! -x "${cached_binary}" ]]; then
            printf 'Fuzzilli cache miss: %s\n' "${cached_binary}"
            exit 1
        fi
        link_cached_binary "$2"
        printf 'Reusing cached Fuzzilli: %s\n' "${cached_binary}"
        ;;
    store)
        if (( $# != 3 )); then
            fail 'Usage: fuzzilli-cache.sh store SOURCE DESTINATION'
        fi
        source_binary="$2"
        destination="$3"
        if [[ ! -x "${source_binary}" ]]; then
            fail "Cannot cache a missing or non-executable Fuzzilli binary: ${source_binary}"
        fi
        mkdir -p "${cache_dir}"
        temporary_binary="${cached_binary}.tmp.$$"
        cleanup_temporary() {
            rm -f -- "${temporary_binary}"
        }
        trap cleanup_temporary EXIT INT TERM
        cp -- "${source_binary}" "${temporary_binary}"
        chmod 0755 "${temporary_binary}"
        mv -f -- "${temporary_binary}" "${cached_binary}"
        trap - EXIT INT TERM
        link_cached_binary "${destination}"
        printf 'Stored Fuzzilli in the machine cache: %s\n' "${cached_binary}"
        ;;
    *)
        fail 'Usage: fuzzilli-cache.sh {path|restore DESTINATION|store SOURCE DESTINATION}'
        ;;
esac
