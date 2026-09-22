#!/usr/bin/env bash
# Fixture-only supervisor: deliberately does not measure time or memory.
set -euo pipefail
if [[ $# -lt 5 || "$1" != -f || "$3" != -o ]]; then
  printf 'mock time: expected -f FORMAT -o OUTPUT COMMAND...\n' >&2
  exit 90
fi
expected='elapsed_seconds=%e\nuser_seconds=%U\nsystem_seconds=%S\nmax_rss_kib=%M\nexit_status=%x'
if [[ "$2" != "$expected" ]]; then
  printf 'mock time: unexpected measurement format\n' >&2
  exit 90
fi
output="$4"
shift 4
status=0
"$@" || status=$?
printf 'timing_source=synthetic-fixture\nelapsed_seconds=unavailable\nuser_seconds=unavailable\nsystem_seconds=unavailable\nmax_rss_kib=unavailable\nexit_status=%s\n' \
  "$status" > "$output"
exit "$status"
