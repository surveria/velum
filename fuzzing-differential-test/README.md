# Velum Differential Fuzzing

This directory contains an opt-in local fuzzing tool for comparing generated
JavaScript programs between Velum, Engine262, and V8 through Node.js. It is
intentionally not used by CI.

The existing `fuzzing-test/` lane is optimized for coverage, crashes, sanitizer
failures, and persistent hangs in Velum. This lane reuses the same pinned
Fuzzilli generator, but runs it against a dedicated differential target. For
each generated program the target:

1. executes the program in a fresh Velum runtime;
2. executes the same program in Engine262 as the correctness oracle;
3. executes the same program in V8/Node for performance ratio and diagnostics;
4. compares normalized status and `fuzzilli('FUZZILLI_PRINT', value)` output;
5. records per-case timing and the Velum/V8 ratio;
6. saves correctness, timeout, crash, performance, and reference-unsupported
   cases immediately.

The default workflow stops after 10 Velum-vs-Engine262 correctness mismatches.
V8 timeouts and V8 crashes are recorded, but they do not stop the run by
default.

## Artifact storage

By default, sessions are stored outside the repository:

```text
/home/user/velum-fuzzing-artifacts/differential-v8/session-<timestamp>
```

Set `VELUM_DIFF_ARTIFACT_ROOT` or pass `--artifact-root` to choose another
absolute shared directory. The session contains:

- `cases/*.jsonl` with one record per compared program;
- `findings/correctness-mismatches/*.js` for Velum-vs-Engine262 differences;
- `findings/performance-slow/*.js` for cases where Velum is slower than V8 by
  the configured ratio;
- `findings/velum-resource-limits/*.js` for cases where Velum stops execution
  through its configured resource limits before reaching a semantic result;
- `findings/engine262-unsupported/*.js` for programs that require APIs missing
  from the pinned Engine262 oracle, such as ECMA-402 `Intl`;
- `findings/*-timeouts/*.js` and `findings/*-crashes/*.js` for engine-specific
  timeouts and crashes;
- `pending/*.js` for scripts that were being executed if the Velum target is
  killed before writing JSONL;
- `engine262-stderr/*.log` and `v8-stderr/*.log` for reference worker exits;
- `all/*.js` only when `--save-all` is requested;
- `fuzzilli/` with Fuzzilli corpus, crashes, timeouts, and statistics;
- `summary.txt`, `slowest.tsv`, and the final detailed `fuzzilli-*.log`.

Generated scripts that matter for follow-up triage are saved as JavaScript
files, so later agents can reproduce them directly without relying on a stable
Fuzzilli global script number.

## Run

```bash
./fuzzing-differential-test/run.sh
```

The default config is `config/default.json`:

- stop after 10 correctness mismatches;
- one Fuzzilli worker;
- Engine262 timeout: 30 seconds;
- V8 timeout: 4 seconds;
- save performance cases where Velum/V8 ratio is at least 2x and Velum took at
  least 5 ms.

Useful overrides:

```bash
./fuzzing-differential-test/run.sh \
    --duration 1h \
    --jobs 1 \
    --engine262-timeout 30s \
    --v8-timeout 4s \
    --slow-ratio 2 \
    --stop-after-mismatches 10
```

Use `--stop-after-mismatches 0` to disable that stop criterion for replay or
long-running diagnostics.

The launcher rebuilds the differential target from the current checkout before
each run. Use `--skip-build` only when the existing local binaries are known to
match the checkout.

`--resume PATH` resumes the Fuzzilli corpus in an existing session and keeps
appending comparison records under the same shared artifact directory.

## Replay

Saved `.js` artifacts can be replayed after a fix:

```bash
./fuzzing-differential-test/run.sh \
    --replay /home/user/velum-fuzzing-artifacts/differential-v8/session-123/findings/correctness-mismatches
```

Replay creates a new session directory, executes the exact saved scripts, and
writes the same `summary.txt`, `cases/*.jsonl`, and categorized findings.

The differential target is built with LLVM sanitizer coverage so Fuzzilli can
guide generation from Velum execution edges. The recorded Velum/V8 ratios are
therefore best used to rank generated programs and select optimization
candidates. They are not a replacement for the normal release benchmark lane.

## Determinism and reproduction

Fuzzilli itself is still the generator and does not expose a stable
script-number interface in the pinned CLI. This lane therefore makes the
comparison reproducible by saving every actionable generated program as an
exact `.js` artifact and recording its SHA-256 hash in JSONL. Use `--save-all`
for short diagnostic runs when every generated script must be retained.

## CI policy

No GitHub Actions job invokes this lane, downloads Fuzzilli, starts Node, or
uploads the external artifacts. It remains a manual or separately scheduled
optimization and differential-testing workflow.
