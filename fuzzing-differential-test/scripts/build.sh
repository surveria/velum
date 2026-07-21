#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
differential_dir="$(dirname "${script_dir}")"
repo_root="$(dirname "${differential_dir}")"
fuzzing_dir="${repo_root}/fuzzing-test"
fuzzilli_binary="${fuzzing_dir}/.bin/FuzzilliCli"
driver_manifest="${differential_dir}/driver/Cargo.toml"

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
require_command "${CC:-cc}" 'sudo apt install build-essential'
require_command node 'install Node.js from https://nodejs.org/ or with your system package manager'

if ! cargo +nightly --version >/dev/null 2>&1 \
    || ! rustc +nightly --version >/dev/null 2>&1; then
    printf '%s\n' 'The Rust nightly toolchain is required for sanitizer coverage.' >&2
    printf '%s\n' 'Install it with: rustup toolchain install nightly' >&2
    exit 1
fi

source_revision="$(git -C "${repo_root}" rev-parse --short=12 HEAD)"
if [[ -n "$(git -C "${repo_root}" status --short --untracked-files=normal)" ]]; then
    source_revision="${source_revision}+dirty"
fi
printf 'Building differential fuzzing from Velum checkout: %s\n' "${source_revision}"

if ! "${fuzzing_dir}/scripts/fuzzilli-cache.sh" restore "${fuzzilli_binary}"; then
    require_command swift 'sudo apt install swiftlang'
    "${fuzzing_dir}/scripts/bootstrap-fuzzilli.sh"

    swift_version="$(swift --version | sed -n \
        '1s/^Swift version \([0-9][^ ]*\).*/\1/p')"
    if [[ ! "${swift_version}" =~ ^([0-9]+)\.([0-9]+) ]]; then
        printf 'Failed to determine the Swift compiler version from: %s\n' \
            "$(swift --version | head -n 1)" >&2
        exit 1
    fi

    swift_major="${BASH_REMATCH[1]}"
    swift_minor="${BASH_REMATCH[2]}"
    if (( swift_major < 6 )); then
        printf 'Fuzzilli requires Swift 6 or newer; found Swift %s\n' \
            "${swift_version}" >&2
        printf '%s\n' 'Install it with: sudo apt install swiftlang' >&2
        exit 1
    fi

    swift_build_args=(
        --package-path "${fuzzing_dir}/fuzzilli"
        --configuration release
        --product FuzzilliCli
    )
    if (( swift_major == 6 && swift_minor == 0 )); then
        printf '%s\n' \
            'Swift 6.0 optimizer workaround enabled for the Fuzzilli release build.'
        swift_build_args+=(
            -Xswiftc -Xfrontend
            -Xswiftc -disable-sil-perf-optzns
        )
    fi
    swift build "${swift_build_args[@]}"
    "${fuzzing_dir}/scripts/fuzzilli-cache.sh" store \
        "${fuzzing_dir}/fuzzilli/.build/release/FuzzilliCli" \
        "${fuzzilli_binary}"
fi

target_triple="$(rustc +nightly --version --verbose | sed -n 's/^host: //p')"
if [[ -z "${target_triple}" ]]; then
    printf '%s\n' 'Failed to determine the Rust nightly host target.' >&2
    exit 1
fi

cargo build --release --manifest-path "${driver_manifest}" \
    --bin velum-diff-fuzz

coverage_flags=(
    -C passes=sancov-module
    -C llvm-args=-sanitizer-coverage-level=3
    -C llvm-args=-sanitizer-coverage-trace-pc-guard
    -C codegen-units=1
)
case "${VELUM_DIFF_SANITIZER:-none}" in
    address)
        coverage_flags+=(-Z sanitizer=address)
        ;;
    none)
        ;;
    *)
        printf 'Unsupported VELUM_DIFF_SANITIZER=%s; use address or none\n' \
            "${VELUM_DIFF_SANITIZER}" >&2
        exit 1
        ;;
esac

encoded_flags="${coverage_flags[*]}"
RUSTFLAGS="${encoded_flags}" cargo +nightly build \
    --manifest-path "${driver_manifest}" \
    --release \
    --target "${target_triple}" \
    --bin velum-diff-target

printf 'Fuzzilli: %s\n' "${fuzzilli_binary}"
printf 'Differential target: %s\n' \
    "${differential_dir}/driver/target/${target_triple}/release/velum-diff-target"
