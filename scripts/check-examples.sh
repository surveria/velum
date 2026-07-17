#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"

examples=(
  00_hello_world
  01_basic_eval
  02_typed_host_functions
  03_async_rust_javascript_roundtrip
  04_callbacks_and_intervals
  05_rust_backed_websocket
  06_javascript_class_from_rust
  07_values_and_handles
  08_compile_once_run_many
  09_sandboxed_execution
  10_custom_module_loader
  11_promises_and_jobs
  12_realms_and_plugins
  13_shared_memory_between_vms
  14_observability_and_teardown
)

for example in "${examples[@]}"; do
  cargo run --quiet --example "${example}" >/dev/null
done

echo "check-examples: ok (${#examples[@]} examples)"
