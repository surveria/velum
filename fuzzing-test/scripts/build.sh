#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
fuzzing_dir="$(dirname "${script_dir}")"
target_manifest="${fuzzing_dir}/velum-reprl/Cargo.toml"

"${script_dir}/bootstrap-fuzzilli.sh"

if ! command -v swift >/dev/null 2>&1; then
    printf '%s\n' 'Swift is required to build Fuzzilli.' >&2
    printf '%s\n' 'On the current Ubuntu host, install it with: sudo apt install swiftlang' >&2
    exit 1
fi
if ! cargo +nightly --version >/dev/null 2>&1; then
    printf '%s\n' 'The Rust nightly toolchain is required for sanitizer coverage.' >&2
    exit 1
fi

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
