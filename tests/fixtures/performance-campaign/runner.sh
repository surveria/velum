#!/usr/bin/env bash
set -euo pipefail

[[ $# == 2 ]] || exit 64
report="$2"
directory="${report%/*}"
lane="${report##*/}"
lane="${lane%.md}"
printf '%s\n' "$$" >> "${directory}/mock-pids.txt"
printf 'lane=%s\nsource=%s\nworking_directory=%s\n' \
  "${lane}" "$(<workload.txt)" "${PWD}" > "${report}"
printf 'mock lane: %s\n' "${lane}"

if [[ "${MOCK_FAILURES:-0}" == 1 ]]; then
  case "${lane}" in
    representative) exit 7 ;;
    holdout)
      # Both processes are finite even if a broken launcher fails to time out.
      sleep 3 &
      child="$!"
      printf '%s\n' "${child}" >> "${directory}/mock-pids.txt"
      stop_child() {
        if kill -0 "${child}" 2>/dev/null; then
          kill "${child}" 2>/dev/null || printf 'child already exited\n'
        fi
        if wait "${child}"; then
          printf 'child completed\n'
        else
          printf 'child stopped\n'
        fi
        printf 'mock child reaped\n'
        exit 143
      }
      trap stop_child TERM INT
      wait "${child}"
      ;;
  esac
fi
