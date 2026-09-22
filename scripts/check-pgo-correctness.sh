#!/usr/bin/env bash
# Validate the saved profile-use executable only after its timed experiment.
# This never builds, prepares corpora, updates baselines, or measures benchmarks.
set -euo pipefail
set +m
export LC_ALL=C

fail() { printf 'PGO correctness: %s\n' "$*" >&2; exit 2; }
if [[ "${1:-}" == --help || "${1:-}" == -h ]]; then
  printf '%s\n' 'usage: bash check-pgo-correctness.sh /absolute/completed-experiment /absolute/frozen-test262 /absolute/qjs' \
    'Runs the saved PGO executable without rebuilding, preparing corpora, or measuring benchmarks.'
  exit 0
fi
[[ $# == 3 ]] || fail 'usage: bash check-pgo-correctness.sh /absolute/completed-experiment /absolute/frozen-test262 /absolute/qjs'
# `git -C` does not override inherited repository, index, object, or config
# routing. Clear the complete namespace before discovering the supplied corpus.
while IFS= read -r variable; do
  case "$variable" in GIT_*) unset "$variable" ;; esac
done < <(compgen -e)
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_NO_REPLACE_OBJECTS=1 GIT_OPTIONAL_LOCKS=0
for tool in git awk sha256sum timeout mktemp find sort xargs cmp setsid realpath grep cut cp date mkdir sleep; do
  command -v "$tool" >/dev/null 2>&1 || fail "missing prerequisite: $tool"
done
[[ -x /usr/bin/time ]] || fail 'missing /usr/bin/time'
for argument in "$@"; do
  [[ "$argument" == /* && "$argument" != *$'\n'* && "$argument" != *$'\t'* ]] || fail 'all paths must be absolute and contain no tabs or newlines'
done
experiment="$(cd "$1" && pwd -P)"
corpus="$(cd "$2" && pwd -P)"
[[ "$(git -C "$corpus" rev-parse --show-toplevel)" == "$corpus" ]] || fail 'Test262 path must name the checkout root'
corpus_git_dir="$(git -C "$corpus" rev-parse --absolute-git-dir)"
corpus_git() {
  git --git-dir="$corpus_git_dir" --work-tree="$corpus" -C "$corpus" -c core.fsmonitor=false "$@"
}
quickjs="$(realpath "$3")"
source_root="$experiment/source"
executable="$experiment/bin/pgo"
profile="$experiment/merged.profdata"
verifier="$experiment/bin/ordinary"
baseline="$source_root/tests/corpora/test262/full-pass-baseline.txt"
pin="$(awk -F= '/^# test262_commit=/ {print $2; count++} END {if (count != 1) exit 1}' "$baseline")"
[[ "$pin" =~ ^[0-9a-f]{40}$ ]] || fail 'invalid frozen Test262 pin'
patches="$(awk -F= '/^# test262_patches=/ {print $2; count++} END {if (count != 1) exit 1}' "$baseline")"
[[ "$patches" == f2d1435644797268dca1f7988cad5a4e89ccd8d2,staging-annex-b-arguments-object ]] || fail 'unsupported frozen Test262 patch set'
[[ -x "$executable" && -x "$verifier" && -x "$quickjs" && -s "$profile" ]] || fail 'saved PGO executable, merged profile, or external qjs is unavailable'
grep -Fxq 'status=complete-needs-review' "$experiment/result.txt" || fail 'timed experiment has not completed; correctness must run afterwards'
grep -Fxq 'exit_code=0' "$experiment/result.txt" || fail 'timed experiment did not finish successfully'
[[ "$(corpus_git rev-parse HEAD)" == "$pin" ]] || fail 'Test262 checkout is not the pinned commit'
[[ -d "$corpus/test" && -d "$corpus/harness" ]] || fail 'Test262 test/harness directories are missing'
sha256sum --check "$executable.sha256" "$verifier.sha256" "$experiment/profile.sha256"

out="$(mktemp -d "$experiment/correctness-$(date -u +%Y%m%dT%H%M%SZ)-XXXXXXXX")"
status=incomplete
child=''
launching=0
pending_signal=0
interrupt() {
  local code="$1"
  if [[ "$launching" == 1 ]]; then pending_signal="$code"; return; fi
  status=interrupted
  exit "$code"
}
stop_child_group() {
  local owned_pid="$child"
  [[ -n "$owned_pid" ]] || return 0
  child=''
  # Only the session created below is ours. Handle the brief setsid startup race.
  if kill -0 -- "-$owned_pid" 2>/dev/null; then
    kill -TERM -- "-$owned_pid" 2>/dev/null || true
  elif kill -0 "$owned_pid" 2>/dev/null; then
    kill -TERM "$owned_pid" 2>/dev/null || true
  fi
  for ((attempt=0; attempt<5; attempt+=1)); do
    if ! kill -0 -- "-$owned_pid" 2>/dev/null && ! kill -0 "$owned_pid" 2>/dev/null; then break; fi
    sleep 0.1
  done
  kill -KILL -- "-$owned_pid" 2>/dev/null || true
  kill -KILL "$owned_pid" 2>/dev/null || true
  wait "$owned_pid" 2>/dev/null || true
}
finish() {
  local code=$? integrity=0
  trap - EXIT INT TERM
  stop_child_group
  if [[ -f "$out/command.txt" && ! -s "$out/time.txt" ]]; then
    printf 'supervisor_interrupted=true\nexit_status=%s\n' "$code" > "$out/time.txt"
  fi
  if [[ -f "$out/command.txt" && ! -f "$out/runner-exit-code.txt" ]]; then
    printf '%s\n' "$code" > "$out/runner-exit-code.txt"
  fi
  if [[ -f "$out/inputs-before.sha256" ]]; then
    sha256sum --check "$out/inputs-before.sha256" > "$out/input-integrity-after.log" 2>&1 || integrity=1
    sha256sum "$executable" "$verifier" "$profile" "$quickjs" > "$out/inputs-after.sha256" || integrity=1
  fi
  if [[ -f "$out/corpus-before.sha256" ]]; then
    (cd "$corpus" && sha256sum --check --quiet "$out/corpus-before.sha256") > "$out/corpus-integrity-after.log" 2>&1 || integrity=1
    (cd "$corpus" && find test harness -printf '%y\t%p\t%l\n' | sort) > "$out/corpus-layout-after.tsv" || integrity=1
    cmp "$out/corpus-layout-before.tsv" "$out/corpus-layout-after.tsv" >> "$out/corpus-integrity-after.log" 2>&1 || integrity=1
  fi
  if [[ -f "$out/source-integrity-before.log" ]]; then
    (cd "$source_root" && sha256sum --check --quiet "$experiment/source-files.sha256") > "$out/source-integrity-after.log" 2>&1 || integrity=1
    (cd "$source_root" && find . -printf '%y\t%p\t%l\n' | sort) > "$out/source-layout-after.tsv" || integrity=1
    cmp "$experiment/source-layout.tsv" "$out/source-layout-after.tsv" >> "$out/source-integrity-after.log" 2>&1 || integrity=1
  fi
  if [[ "$integrity" != 0 ]]; then code=2; status=integrity-failed; fi
  if [[ "$code" != 0 && "$status" == passed ]]; then status=failed; fi
  printf 'status=%s\nexit_code=%s\nintegrity_failed=%s\nfinished_utc=%s\n' \
    "$status" "$code" "$integrity" "$(date -u +%Y%m%dT%H%M%SZ)" > "$out/result.txt"
  printf 'PGO correctness artifacts: %s; status: %s; exit: %s\n' "$out" "$status" "$code" >&2
  exit "$code"
}
trap finish EXIT
trap 'interrupt 130' INT
trap 'interrupt 143' TERM
cp "${BASH_SOURCE[0]}" "$out/launcher.sh"
sha256sum "$executable" "$verifier" "$profile" "$quickjs" > "$out/inputs-before.sha256"

# Build the expected patched index using experiment-owned Git storage only.
# The frozen corpus checkout, its index, and its objects are never modified.
mkdir "$out/corpus-objects"
corpus_objects="$(corpus_git rev-parse --path-format=absolute --git-path objects)"
expected_git() {
  GIT_INDEX_FILE="$out/corpus.index" GIT_OBJECT_DIRECTORY="$out/corpus-objects" \
    GIT_ALTERNATE_OBJECT_DIRECTORIES="$corpus_objects" corpus_git "$@"
}
expected_git read-tree "$pin"
patch_root="$source_root/tests/corpora/test262/patches"
expected_git apply --cached "$patch_root/f2d1435644797268dca1f7988cad5a4e89ccd8d2.patch" \
  "$patch_root/staging-annex-b-arguments-object.patch"
expected_git diff --no-ext-diff --no-textconv --exit-code -- test harness > "$out/corpus-diff.txt" || fail 'Test262 content differs from the exact pinned commit plus two tracked patches'
expected_git ls-files --others -- test harness > "$out/corpus-untracked.txt"
[[ ! -s "$out/corpus-untracked.txt" ]] || fail 'unexpected untracked files in Test262 test/harness'
(cd "$corpus" && find test harness -type f -print0 | sort -z | xargs -0 sha256sum) > "$out/corpus-before.sha256"
(cd "$corpus" && find test harness -printf '%y\t%p\t%l\n' | sort) > "$out/corpus-layout-before.tsv"
(cd "$source_root" && sha256sum --check --quiet "$experiment/source-files.sha256") > "$out/source-integrity-before.log"

read_identity() {
  local key="$1" value
  value="$(awk -F= -v key="$key" '$1 == key {print $2; count++} END {if (count != 1) exit 1}' "$experiment/provenance.txt")"
  [[ "$value" =~ ^[0-9a-f]{40}$ ]] || fail "invalid frozen $key identity"
  printf '%s' "$value"
}
commit="$(read_identity commit)"
tree="$(read_identity tree)"
printf '%s\t%s\n' "$commit" "$tree" > "$out/identity.tsv"

# Do not inherit filters, baseline-output paths, report routing, or profile output.
while IFS= read -r variable; do
  case "$variable" in VELUM_*|LLVM_PROFILE_*) unset "$variable" ;; esac
done < <(compgen -e)
export VELUM_TEST262_DIR="$corpus" VELUM_TEST262_RUN_ALL=1 VELUM_TEST_JOBS=30
export VELUM_TEST262_UPDATE_PASS_BASELINE=0 VELUM_QUICKJS="$quickjs"
export VELUM_TEST262_PASS_CANDIDATE_PATH="$out/pass-candidate.txt"
export VELUM_JETSTREAM_ENABLED=0 VELUM_REPORT_EXHAUSTIVE=0
export VELUM_REPORT_COMMIT_SHA="$commit" VELUM_REPORT_TREE_SHA="$tree"
export VELUM_REPORT_EVENT_NAME=local VELUM_REPORT_REPOSITORY=surveria/velum
export VELUM_REPORT_TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
export VELUM_REPORT_TASK='Exact saved profile-use executable correctness validation after timed PGO experiment'
cd "$source_root"
{
  printf 'cd %q\n' "$PWD"
  printf 'executable=%q\nprofile=%q\ntest262=%q\nquickjs=%q\n' "$executable" "$profile" "$corpus" "$quickjs"
  printf 'git_environment=cleared\ntest262_git_dir=%q\ngit_work_tree=%q\n' "$corpus_git_dir" "$corpus"
  printf 'test262_commit=%s\nworkers=30\nfilters=none\nbaseline_update=disabled\nLLVM_PROFILE_FILE=unset\n' "$pin"
  printf 'command: %q --correctness %q\n' "$executable" "$out/report.md"
  printf 'timeout_seconds=3600\nkill_grace_seconds=10\n'
} > "$out/command.txt"
launching=1
setsid /usr/bin/time -f 'elapsed_seconds=%e\nuser_seconds=%U\nsystem_seconds=%S\nmax_rss_kib=%M\nexit_status=%x' \
  -o "$out/time.txt" timeout --foreground --kill-after=10s 3600s "$executable" --correctness "$out/report.md" \
  <&0 > "$out/stdout.log" 2> "$out/stderr.log" &
child=$!
launching=0
if [[ "$pending_signal" != 0 ]]; then interrupt "$pending_signal"; fi
runner_code=0
wait "$child" || runner_code=$?
stop_child_group
printf '%s\n' "$runner_code" > "$out/runner-exit-code.txt"
[[ "$runner_code" == 0 ]] || fail "saved PGO runner failed with exit $runner_code; inspect $out"
(cd "$source_root" && sha256sum --check --quiet "$experiment/source-files.sha256") > "$out/source-integrity-after.log"

"$verifier" --pgo-correctness-check "$experiment" "$out/report-component.yaml" "$out/validated.json" \
  > "$out/verification.log" 2> "$out/verification.stderr"
status=passed
