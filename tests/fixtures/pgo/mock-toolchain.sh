#!/usr/bin/env bash
# Synthetic fixture: no compiler, JavaScript engine, or benchmark is invoked.
set -euo pipefail
name="${0##*/}"
root="${MOCK_PGO_ROOT:?missing fixture root}"
scenario="${MOCK_PGO_SCENARIO:-success}"
fail() { printf 'mock PGO: %s\n' "$*" >&2; exit 90; }
event() { printf '%s\t%s\t%s\t%s\n' "$1" "${2:-}" "${3:-}" "${4:-}" >> "$root/events.tsv"; }
case "$name" in
  rustc)
    event tool rustc "${1:-}"
    case "$*" in
      '--print sysroot') printf '%s/toolchain\n' "$root" ;;
      -Vv) printf 'rustc 1.96.0 (synthetic)\nhost: x86_64-unknown-linux-gnu\nLLVM version: 22.1.2\n' ;;
      *) fail 'real compilation is forbidden' ;;
    esac
    ;;
  cargo)
    if [[ "${1:-}" == --version ]]; then
      event tool cargo version
      printf 'cargo 1.96.0 (synthetic)\n'
      exit 0
    fi
    run="${PWD%/source}"
    target="${CARGO_TARGET_DIR:?missing target}"
    [[ "$PWD" == "$run/source" && "$run" == "$root/artifacts/"* ]] || fail 'source escaped fixture'
    [[ "$target" == "$run/target" && "${CARGO_BUILD_BUILD_DIR:-}" == "$target" ]] || fail 'build output escaped run'
    [[ "${RUSTC_WRAPPER-unset}" == '' && "${RUSTC_WORKSPACE_WRAPPER-unset}" == '' ]] || fail 'wrapper was inherited'
    flags="${CARGO_ENCODED_RUSTFLAGS:-}"
    variant=ordinary
    if [[ "$flags" == *-Cprofile-generate=* ]]; then variant=instrumented; fi
    if [[ "$flags" == *-Cprofile-use=* ]]; then variant=pgo; fi
    if [[ "$variant" == instrumented ]]; then
      [[ "$flags" == *"-Cprofile-generate=$run/raw" ]] || fail 'wrong generation path'
    elif [[ "$variant" == pgo ]]; then
      [[ "$flags" == *"-Cprofile-use=$run/merged.profdata" ]] || fail 'wrong profile-use path'
    fi
    preset=release
    if [[ "${1:-}" == --config ]]; then
      [[ "${2:-}" == 'profile.release.lto="thin"' && "${3:-}" == --config && "${4:-}" == 'profile.release.codegen-units=1' ]] || fail 'wrong preset settings'
      preset=thin-lto
      shift 4
    fi
    case "${1:-}" in
      clean)
        [[ "$*" == "clean --manifest-path runner/Cargo.toml --target-dir $target" ]] || fail 'wrong clean arguments'
        event clean "$variant" "$target"
        if [[ -d "$target" ]]; then
          [[ -f "$target/CACHEDIR.TAG" ]] || fail 'precreated target has no Cargo tag'
          rm -rf -- "$target"
        fi
        ;;
      build)
        [[ "$*" == 'build --locked --release --verbose --target x86_64-unknown-linux-gnu --manifest-path runner/Cargo.toml --features reference-quickjs' ]] || fail 'wrong build arguments'
        event build "$variant" "$preset" "$PWD"
        if [[ "$variant" == ordinary && "$scenario" == build_failure ]]; then
          printf 'partial synthetic build stdout\n'
          printf 'partial synthetic build stderr\n' >&2
          exit 7
        fi
        if [[ "$variant" == ordinary && "$scenario" == signal ]]; then
          printf 'partial synthetic build stdout\n'
          printf 'partial synthetic build stderr\n' >&2
          sleep 20 &
          descendant=$!
          trap 'kill -TERM "$descendant" 2>/dev/null || true; wait "$descendant" || true; printf "reaped\n" > "$root/child-reaped.txt"; exit 143' TERM INT
          printf '%s\n' "$$" "$descendant" > "$root/mock-pids.txt"
          printf '%s\n' "$descendant" > "$root/ready.txt"
          wait "$descendant"
          fail 'fake compiler was not interrupted'
        fi
        mkdir -p "$target/x86_64-unknown-linux-gnu/release"
        printf 'synthetic Cargo cache tag\n' > "$target/CACHEDIR.TAG"
        cp "$0" "$target/x86_64-unknown-linux-gnu/release/velum-test-runner"
        chmod 755 "$target/x86_64-unknown-linux-gnu/release/velum-test-runner"
        ;;
      *) fail 'unknown fake Cargo operation' ;;
    esac
    ;;
  llvm-profdata)
    case "${1:-}" in
      --version) printf 'LLVM version 22.1.2-rust-fixture\n' ;;
      merge)
        [[ "${2:-}" == --instr && "${3:-}" == --failure-mode=any && "${4:-}" == --sparse=false && "${5:-}" == -o ]] || fail 'unsafe merge options'
        output="${6:?missing profile output}"
        event merge "$output"
        shift 6
        [[ "$#" == 6 ]] || fail 'expected six raw profiles'
        for raw in "$@"; do [[ -s "$raw" ]] || fail 'empty training profile'; done
        printf 'synthetic merged profile\n' > "$output"
        ;;
      show)
        event show
        printf 'Counters:\n  velum.cgu.0;_ZN5velum7runtime7execute17h1234567890abcdefE:\n    Hash: 0x123\n    Counters: 1\n    Block counts: [7]\nInstrumentation level: IR  entry_first = 0\nTotal functions: 1\nTotal number of blocks: 1\nTotal count: 7\nProfile version: 13\n'
        ;;
      *) fail 'unknown fake profile operation' ;;
    esac
    ;;
  llvm-size) printf 'section size\n.text 42\n.rodata 21\n' ;;
  timeout)
    [[ "${1:-}" == --foreground && "${2:-}" == --kill-after=10s ]] || fail 'unexpected timeout supervision'
    shift 3
    if [[ "$scenario" == timeout && "${1:-}" == "$root/toolchain/bin/cargo" ]]; then
      for arg in "$@"; do
        if [[ "$arg" == build ]]; then
          event timeout 125
          printf 'partial synthetic timeout stdout\n'
          printf 'synthetic timeout status 125\n' >&2
          exit 125
        fi
      done
    fi
    # Rust owns the short outer test deadline, independent of campaign limits.
    exec "$@"
    ;;
  ordinary|instrumented|pgo)
    case "${1:-}" in
      --pgo-manifest)
        [[ "$name" == ordinary ]] || fail 'helper did not use ordinary runner'
        event helper manifest
        printf '{"synthetic":true}\n' > "$2/manifest.json"
        ;;
      --pgo-profile-check)
        [[ "$name" == ordinary && -s "$2" ]] || fail 'missing profile dump'
        event helper profile-check
        printf '_ZN5velum7runtime7execute17h1234567890abcdefE\n' > "$3"
        ;;
      --pgo-diagnostics-check)
        [[ "$name" == ordinary && -s "$3" ]] || fail 'missing trained symbols'
        event helper diagnostics-check
        printf 'Missing-function diagnostic lines requiring human review: 0\n' > "$4"
        ;;
      --pgo-report-check)
        [[ "$name" == ordinary && -s "$2/reports/$4.md" ]] || fail 'validation preceded report'
        event helper report-check "$4"
        printf '{"synthetic":true}\n' > "$2/reports/$4.verified.json"
        ;;
      --pgo-summary)
        [[ "$name" == ordinary ]] || fail 'summary did not use ordinary runner'
        event helper summary
        printf 'synthetic summary completed\n' > "$2/mock-summary.txt"
        ;;
      --performance|--memory-benchmarks)
        mode="$1"
        report="$2"
        case_id="${VELUM_BENCH_FILTER:-memory}"
        [[ "$PWD" == "${VELUM_BUILD_REPO_ROOT:-}" ]] || fail 'runner lost frozen source'
        if [[ "$name" == instrumented ]]; then
          [[ "$mode" == --performance && "$case_id" == representative_* && -n "${LLVM_PROFILE_FILE:-}" ]] || fail 'holdout leaked into training'
          raw="${LLVM_PROFILE_FILE//%m/synthetic}"
          raw="${raw//%p/$$}"
          printf 'synthetic raw profile for %s\n' "$case_id" > "$raw"
        else
          [[ -z "${LLVM_PROFILE_FILE+x}" ]] || fail 'evaluation inherited LLVM_PROFILE_FILE'
          [[ "$case_id" == holdout_* || "$case_id" == memory ]] || fail 'wrong evaluation cohort'
        fi
        if [[ "$mode" == --performance ]]; then
          case "$case_id" in
            *_json_ingestion|*_tree_allocation) expected='15000/3/60000' ;;
            *) expected='5000/5/30000' ;;
          esac
          [[ "$VELUM_BENCH_MIN_TIME_MS/$VELUM_BENCH_SAMPLES/$VELUM_BENCH_MAX_TOTAL_MS" == "$expected" ]] || fail 'sampling drift'
        fi
        event lane "$name" "$case_id" "${report##*/}"
        printf '# Synthetic report; no workload was executed.\n' > "$report"
        printf '{"synthetic":true}\n' > "${report%.md}.yaml"
        ;;
      *) fail 'unknown synthetic runner operation' ;;
    esac
    ;;
  *) fail "unrecognized fake tool $name" ;;
esac
