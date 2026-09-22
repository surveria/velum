#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf '%s\n' \
    'usage: run-performance-campaign.sh [--lane all|sentinel|representative|holdout|embedding|jetstream|memory] [--artifact-root /absolute/path]' \
    'Rebuilds the current clean checkout and runs sequential, opt-in measurements.' \
    'Default artifacts: $HOME/velum-fuzzing-artifacts/performance' \
    'Default build cache: $HOME/velum-fuzzing-artifacts/build/performance-runner' \
    'Full campaigns include a bounded JetStream run and isolated memory workers.'
}

fail() {
  printf 'performance campaign: %s\n' "$*" >&2
  exit 2
}

lane=all
artifact_root="${VELUM_PERFORMANCE_ARTIFACT_ROOT:-${HOME}/velum-fuzzing-artifacts/performance}"
while (($#)); do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --lane|--artifact-root)
      (($# >= 2)) || fail "missing value after $1"
      [[ -n "$2" && "$2" != --* ]] || fail "missing value after $1"
      if [[ "$1" == --lane ]]; then lane="$2"; else artifact_root="$2"; fi
      shift 2
      ;;
    *) fail "unknown argument: $1" ;;
  esac
done
case "${lane}" in
  all|sentinel|representative|holdout|embedding|jetstream|memory) ;;
  *) fail "unknown lane '${lane}'; run with --help for supported lanes" ;;
esac
[[ "${artifact_root}" == /* ]] || fail 'artifact root must be an absolute path'
[[ "${GITHUB_ACTIONS:-false}" != true ]] || fail 'performance campaigns are opt-in local work, not ordinary CI'
lane_timeout="${VELUM_PERFORMANCE_LANE_TIMEOUT_SECONDS:-1800}"
[[ "${lane_timeout}" =~ ^[1-9][0-9]{0,5}$ ]] || fail 'VELUM_PERFORMANCE_LANE_TIMEOUT_SECONDS must be a positive integer of at most six digits'

for prerequisite in git cargo rustc date mkdir cp tee sha256sum tar flock timeout; do
  command -v "${prerequisite}" >/dev/null 2>&1 || fail "missing prerequisite '${prerequisite}'; install it before running this campaign"
done
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "${script_dir}/.." && pwd)"
cd "${repo_root}"
[[ -z "$(git status --porcelain)" ]] || fail 'checkout has uncommitted source changes; commit the measurement inputs first'
build_cache="${CARGO_TARGET_DIR:-${HOME}/velum-fuzzing-artifacts/build/performance-runner}"
[[ "${build_cache}" == /* ]] || fail 'CARGO_TARGET_DIR must be absolute so builds survive worktree cleanup'
[[ -z "${CARGO_BUILD_TARGET:-}" ]] || fail 'CARGO_BUILD_TARGET is unsupported by this native-host launcher; unset it for local measurements'

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
output="${artifact_root}/campaign-${timestamp}-$$"
mkdir -p "${artifact_root}"
mkdir "${output}"
export VELUM_BUILD_COMMIT_SHA="$(git rev-parse HEAD)"
export VELUM_REPORT_COMMIT_SHA="${VELUM_BUILD_COMMIT_SHA}"
export VELUM_REPORT_TREE_SHA="$(git rev-parse "${VELUM_BUILD_COMMIT_SHA}^{tree}")"
export CARGO_TARGET_DIR="${build_cache}/sources/${VELUM_BUILD_COMMIT_SHA}"
snapshot="${output}/source"
mkdir "${snapshot}"
git archive "${VELUM_BUILD_COMMIT_SHA}" | tar -x -C "${snapshot}"
export VELUM_BUILD_REPO_ROOT="${snapshot}"
export VELUM_REPORT_TIMESTAMP="${timestamp}"
export VELUM_REPORT_EVENT_NAME=local
export VELUM_REPORT_TASK='Representative performance and memory campaign'
export VELUM_REPORT_REPOSITORY=surveria/velum
unset VELUM_REPORT_RUN_ID VELUM_REPORT_RUN_ATTEMPT VELUM_REPORT_WORKFLOW VELUM_REPORT_PR_NUMBER
unset VELUM_TEST262_RUN_ALL VELUM_TEST262_DIR VELUM_QUICKJS
export VELUM_JETSTREAM_ENABLED=0
export VELUM_QUICKJS_BASELINE_PATH="${output}/quickjs-project.tsv"
export VELUM_JETSTREAM_QUICKJS_BASELINE_PATH="${output}/quickjs-jetstream.tsv"

{
  printf 'schema_version=1\ncommit=%s\ntree=%s\nstarted_utc=%s\nlane=%s\n' \
    "${VELUM_REPORT_COMMIT_SHA}" "${VELUM_REPORT_TREE_SHA}" "${timestamp}" "${lane}"
  printf 'checkout_root=%q\nsource_snapshot=%q\nartifact_root=%q\nbuild_cache=%q\n' "${repo_root}" "${snapshot}" "${output}" "${CARGO_TARGET_DIR}"
  printf 'lane_timeout_seconds=%s\n' "${lane_timeout}"
  printf 'rustflags=%q\ncargo_encoded_rustflags=%q\n' "${RUSTFLAGS:-}" "${CARGO_ENCODED_RUSTFLAGS:-}"
  for setting in OPT_LEVEL CODEGEN_UNITS LTO PANIC DEBUG DEBUG_ASSERTIONS OVERFLOW_CHECKS STRIP INCREMENTAL; do
    variable="CARGO_PROFILE_RELEASE_${setting}"
    printf '%s=%q\n' "${variable}" "${!variable:-<default>}"
  done
  cargo --version
  rustc --version --verbose
} > "${output}/provenance.txt"
printf 'Performance campaign artifacts: %s\n' "${output}"
printf 'Rebuilding current sources; compiler diagnostics: %s/build.log\n' "${output}"
cd "${snapshot}"
mkdir "${output}/bin"
binary="${output}/bin/velum-test-runner"
mkdir -p "${CARGO_TARGET_DIR}"
(
  flock -x 9
  if ! cargo build --locked --release --manifest-path runner/Cargo.toml --features reference-quickjs 2>&1 | tee "${output}/build.log"; then
    fail "release build failed; inspect ${output}/build.log; no measurements were started"
  fi
  build_binary="${CARGO_TARGET_DIR}/release/velum-test-runner"
  [[ -x "${build_binary}" ]] || fail "build did not produce executable ${build_binary}"
  cp "${build_binary}" "${binary}"
) 9> "${CARGO_TARGET_DIR}/campaign-build.lock"
sha256sum "${binary}" > "${output}/binary.sha256"

printf 'lane\texit_code\telapsed_seconds\treport\tlog\n' > "${output}/lanes.tsv"
campaign_failed=0
run_lane() {
  local selected="$1" mode=--performance start status=0
  local report="${output}/${selected}.md"
  export VELUM_REPORT_TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
  export VELUM_BENCH_SET=full
  export VELUM_QUICKJS_BASELINE=refresh
  unset VELUM_BENCH_FILTER VELUM_JETSTREAM_FILTER
  case "${selected}" in
    sentinel) export VELUM_BENCH_SET=sentinel ;;
    representative) export VELUM_BENCH_FILTER='representative_*' ;;
    holdout) export VELUM_BENCH_FILTER='holdout_*' ;;
    embedding) export VELUM_BENCH_FILTER='embedding_*' ;;
    jetstream)
      mode=--jetstream
      export VELUM_JETSTREAM_QUICKJS_BASELINE=refresh
      export VELUM_JETSTREAM_SUITE_MAX_SECONDS="${VELUM_JETSTREAM_SUITE_MAX_SECONDS:-900}"
      ;;
    memory) mode=--memory-benchmarks ;;
  esac
  printf 'Starting %s lane; report: %s\n' "${selected}" "${report}"
  start="${SECONDS}"
  if timeout --kill-after=10s "${lane_timeout}s" "${binary}" "${mode}" "${report}" > "${output}/${selected}.log" 2>&1; then
    printf 'Completed %s lane\n' "${selected}"
  else
    status=$?
    campaign_failed=1
    printf 'Lane %s returned %s; inspect %s/%s.log and its report\n' "${selected}" "${status}" "${output}" "${selected}" >&2
    if [[ "${status}" == 124 || "${status}" == 137 ]]; then
      printf 'Lane %s has a watchdog-compatible timeout/signal status; inspect its log, as worker exits can share this code\n' "${selected}" >&2
    elif [[ "${status}" == 125 ]]; then
      printf 'Lane %s has a process watchdog/tool failure or worker exit 125; its report may be incomplete\n' "${selected}" >&2
    fi
  fi
  printf '%s\t%s\t%s\t%s\t%s\n' "${selected}" "${status}" "$((SECONDS - start))" "${report}" "${output}/${selected}.log" >> "${output}/lanes.tsv"
}

if [[ "${lane}" == all ]]; then
  for selected in sentinel representative holdout embedding jetstream memory; do
    run_lane "${selected}"
  done
else
  run_lane "${lane}"
fi
printf 'Finished UTC: %s\n' "$(date -u +%Y%m%dT%H%M%SZ)" >> "${output}/provenance.txt"
printf 'Campaign lane statuses (zero means the lane completed; consult reports for unavailable/failed candidates):\n'
while IFS=$'\t' read -r selected status elapsed report log; do
  printf '%-16s %-10s %-16s %s\n' "${selected}" "${status}" "${elapsed}" "${report}"
done < "${output}/lanes.tsv"
printf 'Full artifacts: %s\n' "${output}"
exit "${campaign_failed}"
