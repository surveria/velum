#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
driver_manifest="${script_dir}/driver/Cargo.toml"

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'Missing required command: cargo' >&2
    printf '%s\n' 'Install Rust with rustup from https://rustup.rs/' >&2
    exit 1
fi

exec cargo run --release --manifest-path "${driver_manifest}" \
    --bin velum-diff-fuzz -- "$@"
