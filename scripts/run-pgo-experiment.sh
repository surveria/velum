#!/usr/bin/env bash
# Opt-in, fail-closed PGO evidence collection. Ordinary builds and CI are unchanged.
# The saved ordinary runner validates evidence; no Python or package installation.
set -euo pipefail
set +m
export LC_ALL=C

usage() {
  printf '%s\n' \
    'usage: run-pgo-experiment.sh --execute [--repo /absolute/clean/checkout] [--artifact-root /absolute/directory] [--preset release|thin-lto] [--rounds 2] [--cpu 0|inherit]' \
    'Opt-in experiment: frozen source -> ordinary/instrumented -> representative training -> PGO -> holdout + memory.' \
    'No compiler defaults or repository files are changed. Each run owns its complete target/build directory.' \
    'The thin-lto preset adds ThinLTO and one codegen unit to all three builds; profiles are never reused.' \
    'Two alternating A/B rounds by default; 1..5 allowed. Baseline settings and cohorts are fixed before evaluation.'
}

fail() { printf 'PGO experiment: %s\n' "$*" >&2; exit 2; }
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
repo="$(cd "$script_dir/.." && pwd -P)"
preset=release
root="${HOME}/velum-fuzzing-artifacts/performance/pgo"
rounds=2
cpu=0
execute=0
while (($#)); do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --execute) execute=1; shift ;;
    --repo|--artifact-root|--rounds|--cpu|--preset)
      (($# >= 2)) || fail "missing value after $1"
      case "$1" in
        --repo) repo="$2" ;;
        --artifact-root) root="$2" ;;
        --rounds) rounds="$2" ;;
        --cpu) cpu="$2" ;;
        --preset) preset="$2" ;;
      esac
      shift 2
      ;;
    *) fail "unknown argument: $1" ;;
  esac
done
[[ "$execute" == 1 ]] || fail 'explicit --execute is required; --help describes the experiment'
[[ "$repo" == /* && "$root" == /* ]] || fail 'repository and artifact roots must be absolute'
[[ "$rounds" =~ ^[1-5]$ ]] || fail '--rounds must be between 1 and 5'
[[ "$preset" == release || "$preset" == thin-lto ]] || fail '--preset must be release or thin-lto'
[[ "$cpu" == inherit || "$cpu" =~ ^(0|[1-9][0-9]{0,4})$ ]] || fail '--cpu must be a nonnegative CPU number or inherit'
[[ "${GITHUB_ACTIONS:-false}" != true ]] || fail 'this opt-in experiment must not run in ordinary CI'
[[ "$(uname -s)" == Linux ]] || fail 'this workflow requires Linux memory sampling'
for tool in git cargo rustc date mkdir mktemp cp sha256sum tar timeout stat find sort xargs awk grep chmod cmp taskset setsid sleep; do
  command -v "$tool" >/dev/null 2>&1 || fail "missing prerequisite: $tool"
done
[[ -x /usr/bin/time ]] || fail 'GNU /usr/bin/time is required'
if [[ "$cpu" != inherit ]]; then
  taskset --cpu-list "$cpu" /bin/true || fail "requested CPU $cpu is unavailable; no fallback affinity is permitted"
fi
for variable in RUSTFLAGS CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_TARGET RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTC_BOOTSTRAP; do
  [[ -z "${!variable:-}" ]] || fail "unset $variable; this experiment owns its common compiler flags"
done
while IFS= read -r variable; do
  [[ "$variable" != CARGO_PROFILE_* ]] || fail "unset $variable; select the explicit --preset instead"
done < <(compgen -e)

repo="$(cd "$repo" && pwd -P)"
[[ "$(git -C "$repo" rev-parse --show-toplevel)" == "$repo" ]] || fail '--repo must name the checkout root'
mkdir -p "$root"
root="$(cd "$root" && pwd -P)"
case "$root/" in "$repo/"*) fail 'artifact root must be outside the measured checkout' ;; esac
[[ "$repo$root" != *$'\n'* && "$repo$root" != *$'\t'* ]] || fail 'paths must not contain tabs or newlines'
[[ "$root" != *%* && "$root" != *$'\x1f'* ]] || fail 'artifact root must not contain LLVM profile substitutions or flag separators'
[[ -z "$(git -C "$repo" status --porcelain)" ]] || fail 'measurement requires a clean committed checkout'
# Cargo searches the working directory's ancestors and CARGO_HOME. Do not let
# an unrecorded profile, source replacement, or compiler setting alter a preset.
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
[[ "$cargo_home" == /* ]] || fail 'CARGO_HOME must be absolute when set'
for config in "$cargo_home/config" "$cargo_home/config.toml" "$repo/.cargo/config" "$repo/.cargo/config.toml"; do
  [[ ! -e "$config" ]] || fail "unsupported ambient Cargo configuration: $config"
done
ancestor="$root"
while :; do
  for config in "$ancestor/.cargo/config" "$ancestor/.cargo/config.toml"; do
    [[ ! -e "$config" ]] || fail "unsupported ancestor Cargo configuration: $config"
  done
  [[ "$ancestor" != / ]] || break
  ancestor="$(dirname "$ancestor")"
done
commit="$(git -C "$repo" rev-parse HEAD)"
tree="$(git -C "$repo" rev-parse "$commit^{tree}")"
sysroot="$(rustc --print sysroot)"
rustc_bin="$sysroot/bin/rustc"
cargo_bin="$sysroot/bin/cargo"
[[ -x "$rustc_bin" && -x "$cargo_bin" ]] || fail 'selected toolchain must contain rustc and cargo'
compiler_version="$("$rustc_bin" -Vv)"
host="$(awk '/^host: / {print $2}' <<< "$compiler_version")"
llvm_version="$(awk '/^LLVM version: / {print $3}' <<< "$compiler_version")"
[[ "$host" == x86_64-unknown-linux-gnu && "$llvm_version" == 22.* ]] || fail 'profile diagnostic guards are pinned to native x86_64 Linux and LLVM 22'
llvm_bin="$sysroot/lib/rustlib/$host/bin"
profdata="$llvm_bin/llvm-profdata"
llvm_size="$llvm_bin/llvm-size"
[[ -x "$profdata" && -x "$llvm_size" ]] || fail 'install matching llvm-tools separately before this experiment'
profdata_version="$("$profdata" --version)"
tool_llvm_version="$(awk '/LLVM version/ {print $3}' <<< "$profdata_version")"
[[ "$tool_llvm_version" == "$llvm_version" || "$tool_llvm_version" == "$llvm_version"-* ]] || fail 'rustc and bundled llvm-profdata LLVM versions do not match'

run="$(mktemp -d "$root/pgo-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXXXX")"
source_root="$run/source"
target="$run/target"
raw="$run/raw"
profile="$run/merged.profdata"
# Cargo creates and tags its own target/build directory. Do not precreate it:
# modern cargo clean rejects an existing explicit target without CACHEDIR.TAG.
mkdir "$source_root" "$raw" "$run/bin" "$run/steps" "$run/reports"
printf 'step\texit_code\twall_seconds\tcommand\tstdout\tstderr\ttime\n' > "$run/steps.tsv"
printf 'variant\tbytes\tsha256\n' > "$run/binaries.tsv"
active_step=initialization
active_pid=''
active_start=0
launching_step=0
pending_signal=0
completion=incomplete
finish() {
  local code=$?
  trap - EXIT
  printf 'status=%s\nexit_code=%s\nlast_step=%s\nfinished_utc=%s\n' \
    "$completion" "$code" "$active_step" "$(date -u +%Y%m%dT%H%M%SZ)" > "$run/result.txt"
  printf 'PGO artifacts: %s; status: %s; exit: %s\n' "$run" "$completion" "$code" >&2
  exit "$code"
}
trap finish EXIT

record_step() {
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$active_step" "$1" "$((SECONDS-active_start))" \
    "steps/$active_step.command" "steps/$active_step.log" "steps/$active_step.stderr" "steps/$active_step.time" >> "$run/steps.tsv"
}

stop_active_group() {
  [[ -n "$active_pid" ]] || return 0
  # Every step has its own session; timeout must not create a second group.
  # The direct-PID fallback covers the brief interval before setsid executes.
  kill -TERM -- "-$active_pid" 2>/dev/null || kill -TERM "$active_pid" 2>/dev/null || true
  local attempt
  for attempt in {1..10}; do
    kill -0 -- "-$active_pid" 2>/dev/null || break
    sleep 0.05
  done
  kill -KILL -- "-$active_pid" 2>/dev/null || true
}

interrupt_step() {
  local code="$1" child_status=0
  # Defer only across the asynchronous spawn/PID assignment, never a wait.
  if [[ "$launching_step" == 1 ]]; then pending_signal="$code"; return; fi
  trap '' INT TERM
  if [[ -n "$active_pid" ]]; then
    stop_active_group
    wait "$active_pid" || child_status=$?
    # Group cancellation can kill GNU time itself: preserve that limitation.
    if [[ ! -s "$run/steps/$active_step.time" ]]; then
      printf 'elapsed_seconds=%s\nuser_seconds=unavailable\nsystem_seconds=unavailable\nmax_rss_kib=unavailable\nexit_status=%s\n' \
        "$((SECONDS-active_start))" "$code" > "$run/steps/$active_step.time"
    fi
    printf 'interrupted_status=%s\nsupervisor_wait_status=%s\n' "$code" "$child_status" >> "$run/steps/$active_step.time"
    record_step "$code"
    active_pid=''
  fi
  exit "$code"
}
trap 'interrupt_step 130' INT
trap 'interrupt_step 143' TERM

run_step() {
  local id="$1" limit="$2" status=0
  shift 2
  active_step="$id"
  {
    printf 'cd %q\n' "$PWD"
    printf 'CARGO_ENCODED_RUSTFLAGS=%q\nLLVM_PROFILE_FILE=%q\n' "${CARGO_ENCODED_RUSTFLAGS:-}" "${LLVM_PROFILE_FILE:-}"
    printf 'VELUM_BENCH_FILTER=%q\nVELUM_BENCH_MIN_TIME_MS=%q\nVELUM_BENCH_SAMPLES=%q\nVELUM_BENCH_MAX_TOTAL_MS=%q\n' \
      "${VELUM_BENCH_FILTER:-}" "${VELUM_BENCH_MIN_TIME_MS:-}" "${VELUM_BENCH_SAMPLES:-}" "${VELUM_BENCH_MAX_TOTAL_MS:-}"
    printf '%q ' "$@"
    printf '\n'
  } > "$run/steps/$id.command"
  printf 'Starting %s\n' "$id"
  active_start="$SECONDS"
  launching_step=1
  setsid /usr/bin/time -f 'elapsed_seconds=%e\nuser_seconds=%U\nsystem_seconds=%S\nmax_rss_kib=%M\nexit_status=%x' \
    -o "$run/steps/$id.time" timeout --foreground --kill-after=10s "${limit}s" "$@" <&0 > "$run/steps/$id.log" 2> "$run/steps/$id.stderr" &
  active_pid=$!
  launching_step=0
  if [[ "$pending_signal" != 0 ]]; then interrupt_step "$pending_signal"; fi
  if wait "$active_pid"; then
    status=0
  else
    status=$?
  fi
  # Foreground timeout only signals its immediate child; remove any remaining
  # descendants in this step's owned group after every unsuccessful command.
  if [[ "$status" != 0 ]]; then stop_active_group; fi
  launching_step=1
  record_step "$status"
  active_pid=''
  launching_step=0
  if [[ "$pending_signal" != 0 ]]; then interrupt_step "$pending_signal"; fi
  [[ "$status" == 0 ]] || fail "$id failed with status $status; inspect $run/steps/$id.log and .stderr"
}

# Pin compiler executables and override ambient rustflags, including Cargo config.
# LLVM diagnostics are common to all variants and do not request PGO by themselves.
common_flags='-Cllvm-args=-pgo-warn-missing-function -no-pgo-warn-mismatch=false -no-pgo-warn-mismatch-comdat-weak=false'
export RUSTC="$rustc_bin" CARGO_TARGET_DIR="$target" CARGO_BUILD_BUILD_DIR="$target" CARGO_INCREMENTAL=0
export RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER=''
export CARGO_ENCODED_RUSTFLAGS="$common_flags"
cargo_config=()
if [[ "$preset" == thin-lto ]]; then
  cargo_config=(--config 'profile.release.lto="thin"' --config 'profile.release.codegen-units=1')
fi
unset RUSTFLAGS LLVM_PROFILE_FILE LLVM_PROFILE_MERGE_POOL_SIZE LLVM_PROFILE_CONTINUOUS_MODE
export VELUM_BUILD_COMMIT_SHA="$commit" VELUM_BUILD_REPO_ROOT="$source_root"
export VELUM_REPORT_COMMIT_SHA="$commit" VELUM_REPORT_TREE_SHA="$tree"
export VELUM_REPORT_EVENT_NAME=local VELUM_REPORT_REPOSITORY=surveria/velum
export VELUM_REPORT_TASK='Local opt-in PGO experiment; no default PGO enablement'
unset VELUM_REPORT_RUN_ID VELUM_REPORT_RUN_ATTEMPT VELUM_REPORT_WORKFLOW VELUM_REPORT_PR_NUMBER
unset VELUM_TEST262_RUN_ALL VELUM_TEST262_DIR VELUM_QUICKJS VELUM_JETSTREAM_FILTER
unset VELUM_MEMORY_FILTER VELUM_REPORT_EXHAUSTIVE
export VELUM_JETSTREAM_ENABLED=0 VELUM_BENCH_SET=full VELUM_QUICKJS_BASELINE=refresh
export VELUM_HOST_LOCK_PATH=/run/lock/velum/host-performance.lock
export VELUM_BENCH_WARMUP_MS=150 VELUM_BENCH_MIN_TIME_MS=5000 VELUM_BENCH_SAMPLES=5
export VELUM_BENCH_MIN_OP_US=1000 VELUM_BENCH_MAX_CV_PERCENT=10 VELUM_BENCH_ATTEMPTS=3
export VELUM_BENCH_MAX_OP_MS=2000 VELUM_BENCH_MAX_TOTAL_MS=30000
export VELUM_MEMORY_REPETITIONS=3 VELUM_MEMORY_CHILD_TIMEOUT_MS=120000
export VELUM_MEMORY_NODES=1024 VELUM_MEMORY_BYTES_PER_NODE=256 VELUM_MEMORY_CHURN_ROUNDS=3
{
  printf 'schema_version=1\ncommit=%s\ntree=%s\nhost=%s\nrounds=%s\npreset=%s\n' "$commit" "$tree" "$host" "$rounds" "$preset"
  printf 'cargo_profile_overrides='; printf '%q ' "${cargo_config[@]}"; printf '\n'
  printf 'source=%q\ntarget=%q\ncommon_encoded_rustflags=%q\n' "$source_root" "$target" "$common_flags"
  printf 'cargo_build_dir=%q\ncargo_build_jobs=%q\nrustc_wrapper=disabled\nworkspace_wrapper=disabled\n' "$target" "${CARGO_BUILD_JOBS:-<cargo-default>}"
  printf 'build_timeout_seconds=3600\nlane_timeout_seconds=1800\nprofile_timeout_seconds=120\n'
  printf 'training_filter=representative_*\nevaluation_filter=holdout_*\n'
  printf 'execution_filter_policy=one exact workload ID per report\nrequested_cpu=%s\n' "$cpu"
  printf 'protocol_version=2-frozen-before-pgo-training\nwarmup_ms=150\nminimum_sample_time_ms=5000\nsamples=5\nmax_total_time_ms=30000\nmax_cv_percent=10\n'
  printf 'long_exact_ids=representative_json_ingestion,holdout_json_ingestion,representative_tree_allocation,holdout_tree_allocation\nlong_minimum_time_ms=15000\nlong_samples=3\nlong_max_total_time_ms=60000\n'
  printf 'tuning_policy=no protocol changes after training or holdout evaluation begins\n'
  printf 'cold_build_policy=empty experiment-owned target before every variant\n'
  printf 'measurement_policy=ordinary-pgo then pgo-ordinary, sequential runner-owned host locking\n'
  printf 'scope=whole Rust runner and dependencies; ELF bytes and sections are not pure engine library size\n'
  printf 'unprofiled=precompiled standard library and bundled C QuickJS\n'
  printf 'host_lock_path=%q\n' "${VELUM_HOST_LOCK_PATH:-/run/lock/velum/host-performance.lock}"
  printf '%s\n' "$compiler_version" "$profdata_version"
  "$cargo_bin" --version
} > "$run/provenance.txt"
cp "${BASH_SOURCE[0]}" "$run/launcher.sh"
sha256sum "$rustc_bin" "$cargo_bin" "$profdata" "$llvm_size" "$run/launcher.sh" > "$run/tools.sha256"
run_step source-export 120 git -C "$repo" archive --format=tar --output="$run/source.tar" "$commit"
sha256sum "$run/source.tar" > "$run/source-archive.sha256"
run_step source-extract 120 tar -xf "$run/source.tar" -C "$source_root"
chmod -R a-w "$source_root"
cd "$source_root"
find . -type f -print0 | sort -z | xargs -0 sha256sum > "$run/source-files.sha256"
find . -printf '%y\t%p\t%l\n' | sort > "$run/source-layout.tsv"
case_suffixes=(object_transform method_dispatch json_ingestion string_processing collection_index tree_allocation)
printf 'id\tsource\tsha256\n' > "$run/cases.tsv"
for cohort in representative holdout; do
  for suffix in "${case_suffixes[@]}"; do
    case_id="${cohort}_${suffix}"
    case_path="tests/corpora/benchmarks/prepared/$case_id.js"
    [[ -f "$case_path" && ! -L "$case_path" ]] || fail "missing regular workload source: $case_path"
    case_digest="$(sha256sum "$case_path")"
    printf '%s\t%s\t%s\n' "$case_id" "$case_path" "${case_digest%% *}" >> "$run/cases.tsv"
  done
done
sha256sum "$run/cases.tsv" "$run/source-files.sha256" "$run/source-layout.tsv" >> "$run/tools.sha256"

verify_inputs() {
  local label="$1"
  run_step "$label-source" 120 sha256sum --check "$run/source-files.sha256"
  find . -printf '%y\t%p\t%l\n' | sort > "$run/steps/$label-source-layout.tsv"
  run_step "$label-layout" 120 cmp "$run/source-layout.tsv" "$run/steps/$label-source-layout.tsv"
  run_step "$label-tools" 120 sha256sum --check "$run/tools.sha256"
}

build_variant() {
  local variant="$1" flag="$2" executable digest bytes
  verify_inputs "$variant"
  export CARGO_ENCODED_RUSTFLAGS="$common_flags"
  if [[ -n "$flag" ]]; then export CARGO_ENCODED_RUSTFLAGS+=$'\x1f'"$flag"; fi
  # Only this freshly created experiment's target directory is cleaned.
  run_step "$variant-clean" 120 "$cargo_bin" clean --manifest-path runner/Cargo.toml --target-dir "$target"
  run_step "$variant-build" 3600 "$cargo_bin" "${cargo_config[@]}" build --locked --release --verbose \
    --target "$host" --manifest-path runner/Cargo.toml --features reference-quickjs
  executable="$target/$host/release/velum-test-runner"
  [[ -x "$executable" ]] || fail "$variant did not produce the expected native-target runner"
  cp "$executable" "$run/bin/$variant"
  chmod a-w "$run/bin/$variant"
  sha256sum "$run/bin/$variant" > "$run/bin/$variant.sha256"
  digest="$(awk '{print $1}' "$run/bin/$variant.sha256")"
  bytes="$(stat -c %s "$run/bin/$variant")"
  printf '%s\t%s\t%s\n' "$variant" "$bytes" "$digest" >> "$run/binaries.tsv"
  run_step "$variant-sections" 120 "$llvm_size" -A "$run/bin/$variant"
  run_step "$variant-manifest" 120 "$run/bin/ordinary" --pgo-manifest \
    "$run" "$commit" "$tree" "$host" "$cpu" "$rounds" "$preset"
}

sampling_for_case() {
  export VELUM_BENCH_MIN_TIME_MS=5000 VELUM_BENCH_SAMPLES=5 VELUM_BENCH_MAX_TOTAL_MS=30000
  case "$1" in
    representative_json_ingestion|holdout_json_ingestion|representative_tree_allocation|holdout_tree_allocation)
      export VELUM_BENCH_MIN_TIME_MS=15000 VELUM_BENCH_SAMPLES=3 VELUM_BENCH_MAX_TOTAL_MS=60000 ;;
  esac
}

raw_profile_count() {
  find "$raw" -maxdepth 1 -type f -name '*.profraw' -size +0c -printf '.\n' | awk 'END {print NR}'
}

run_lane() {
  local label="$1" variant="$2" case_id="$3" mode=--performance kind=performance
  local profiles_before=0 profiles_after=0
  local -a affinity=()
  if [[ "$variant" == instrumented ]]; then
    [[ "$case_id" == representative_* && -n "${LLVM_PROFILE_FILE:-}" ]] || fail 'instrumented execution is limited to the training cohort'
    profiles_before="$(raw_profile_count)"
  else
    [[ -z "${LLVM_PROFILE_FILE+x}" ]] || fail 'evaluation must not receive LLVM_PROFILE_FILE'
  fi
  if [[ "$cpu" != inherit ]]; then affinity=(taskset --cpu-list "$cpu"); fi
  export VELUM_REPORT_TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
  export VELUM_QUICKJS_BASELINE_PATH="$run/reports/$label-quickjs.tsv"
  if [[ "$case_id" == memory ]]; then
    mode=--memory-benchmarks
    kind=memory
    unset VELUM_BENCH_FILTER
  else
    sampling_for_case "$case_id"
    export VELUM_BENCH_FILTER="$case_id"
  fi
  run_step "$label-binary" 120 sha256sum --check "$run/bin/$variant.sha256"
  run_step "$label" 1800 "${affinity[@]}" "$run/bin/$variant" "$mode" "$run/reports/$label.md"
  if [[ "$variant" == instrumented ]]; then
    profiles_after="$(raw_profile_count)"
    ((profiles_after > profiles_before)) || fail "$case_id produced no new nonempty training profile"
  fi
  if [[ "$kind" == memory ]]; then
    run_step "$label-verify" 120 "$run/bin/ordinary" --pgo-report-check "$run" "$variant" "$label" "$kind"
  else
    run_step "$label-verify" 120 "$run/bin/ordinary" --pgo-report-check "$run" "$variant" "$label" "$kind" "$case_id"
  fi
}

run_cohort() {
  local label="$1" variant="$2" cohort="$3" suffix
  for suffix in "${case_suffixes[@]}"; do
    run_lane "$label-${cohort}_$suffix" "$variant" "${cohort}_$suffix"
  done
}

build_variant ordinary ''
build_variant instrumented "-Cprofile-generate=$raw"
export LLVM_PROFILE_FILE="$raw/%m-%p.profraw"
run_cohort training instrumented representative
unset LLVM_PROFILE_FILE
shopt -s nullglob
raw_files=("$raw"/*.profraw)
shopt -u nullglob
((${#raw_files[@]} > 0)) || fail 'training produced no raw profiles'
for file in "${raw_files[@]}"; do [[ -s "$file" ]] || fail "empty training profile: $file"; done
sha256sum "${raw_files[@]}" > "$run/raw.sha256"
run_step profile-merge 120 "$profdata" merge --instr --failure-mode=any --sparse=false -o "$profile" "${raw_files[@]}"
[[ ! -s "$run/steps/profile-merge.stderr" ]] || fail 'merge emitted diagnostics; review before proceeding'
[[ -s "$profile" ]] || fail 'merge produced no usable profile'
run_step profile-show 120 "$profdata" show --instr --all-functions --counts --profile-version "$profile"
[[ ! -s "$run/steps/profile-show.stderr" ]] || fail 'profile inspection emitted diagnostics'
run_step profile-verify 120 "$run/bin/ordinary" --pgo-profile-check "$run/steps/profile-show.log" "$run/trained-engine-symbols.txt"
chmod a-w "$profile"
sha256sum "$profile" > "$run/profile.sha256"
build_variant pgo "-Cprofile-use=$profile"
run_step pgo-profile-integrity 120 sha256sum --check "$run/profile.sha256" "$run/raw.sha256"
run_step pgo-diagnostics 120 "$run/bin/ordinary" --pgo-diagnostics-check \
  "$run/steps/pgo-build.stderr" "$run/trained-engine-symbols.txt" "$run/profile-diagnostics.json"

# Holdouts and memory never contribute profile counters.
for ((round=1; round<=rounds; round++)); do
  order=(ordinary pgo)
  if ((round % 2 == 0)); then order=(pgo ordinary); fi
  for variant in "${order[@]}"; do
    run_cohort "round-$round-$variant" "$variant" holdout
    run_lane "round-$round-$variant-memory" "$variant" memory
  done
done
verify_inputs final
run_step final-profile-integrity 120 sha256sum --check "$run/profile.sha256" "$run/raw.sha256"
run_step compare-holdouts 120 "$run/bin/ordinary" --pgo-summary "$run"
completion=complete-needs-review
printf 'Evidence collection finished; review summary, cost/size logs and saved-candidate correctness before adoption.\n'
printf 'Run saved-candidate correctness separately: bash %q/check-pgo-correctness.sh %q /absolute/pinned-test262 /absolute/qjs\n' "$script_dir" "$run"
