#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fuzzing_dir="$(dirname "${script_dir}")"
repo_root="$(dirname "${fuzzing_dir}")"
target_manifest="${fuzzing_dir}/velum-reprl/Cargo.toml"

require_command() {
    local command_name="$1"
    local install_hint="$2"
    if command -v -- "${command_name}" >/dev/null 2>&1; then
        return
    fi
    printf 'Missing required command: %s\n' "${command_name}" >&2
    printf 'How to install it: %s\n' "${install_hint}" >&2
    exit 1
}

require_command git 'sudo apt install git'
require_command cargo 'install Rust with rustup from https://rustup.rs/'
require_command rustc 'install Rust with rustup from https://rustup.rs/'
require_command swift 'sudo apt install swiftlang'
require_command "${CC:-cc}" 'sudo apt install build-essential'

if ! cargo +nightly --version >/dev/null 2>&1 \
    || ! rustc +nightly --version >/dev/null 2>&1; then
    printf '%s\n' 'The Rust nightly toolchain is required for sanitizer coverage.' >&2
    printf '%s\n' 'Install it with: rustup toolchain install nightly' >&2
    exit 1
fi

"${script_dir}/bootstrap-fuzzilli.sh"

source_revision="$(git -C "${repo_root}" rev-parse --short=12 HEAD)"
if [[ -n "$(git -C "${repo_root}" status --short --untracked-files=normal)" ]]; then
    source_revision="${source_revision}+dirty"
fi
printf 'Building from the current Velum checkout: %s\n' "${source_revision}"

swift build --package-path "${fuzzing_dir}/fuzzilli" \
    --configuration release \
    --product FuzzilliCli

target_triple="$(rustc +nightly --version --verbose | sed -n 's/^host: //p')"
if [[ -z "${target_triple}" ]]; then
    printf '%s\n' 'Failed to determine the Rust nightly host target.' >&2
    exit 1
fi

coverage_flags=(
    -C passes=sancov-module
    -C llvm-args=-sanitizer-coverage-level=3
    -C llvm-args=-sanitizer-coverage-trace-pc-guard
    -C codegen-units=1
)
case "${VELUM_FUZZ_SANITIZER:-address}" in
    address)
        coverage_flags+=(-Z sanitizer=address)
        ;;
    none)
        ;;
    *)
        printf 'Unsupported VELUM_FUZZ_SANITIZER=%s; use address or none\n' \
            "${VELUM_FUZZ_SANITIZER}" >&2
        exit 1
        ;;
esac

encoded_flags="${coverage_flags[*]}"
RUSTFLAGS="${encoded_flags}" cargo +nightly build \
    --manifest-path "${target_manifest}" \
    --release \
    --target "${target_triple}"

printf 'Fuzzilli: %s\n' "${fuzzing_dir}/fuzzilli/.build/release/FuzzilliCli"
printf 'Velum target: %s\n' \
    "${fuzzing_dir}/velum-reprl/target/${target_triple}/release/velum-fuzzilli"
