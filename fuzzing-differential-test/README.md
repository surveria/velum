# Velum V8 Differential Fuzzing

This directory contains an opt-in local fuzzing lane for comparing generated
JavaScript programs between Velum and V8 through Node.js. It is intentionally
not used by CI.

The existing `fuzzing-test/` lane is optimized for coverage, crashes, sanitizer
failures, and persistent hangs in Velum. This lane reuses the same pinned
Fuzzilli generator, but runs it against a dedicated differential target. For
each generated program the target:

1. executes the program in a fresh Velum runtime;
2. executes the same program in a persistent Node/V8 worker;
3. compares normalized status and `fuzzilli('FUZZILLI_PRINT', value)` output;
4. records per-case timing and the Velum/V8 ratio;
5. saves mismatch, timeout, crash, and slow cases immediately.

## Artifact storage

By default, sessions are stored outside the repository:

```text
/home/user/velum-fuzzing-artifacts/differential-v8/session-<timestamp>
```

Set `VELUM_DIFF_ARTIFACT_ROOT` or pass `--artifact-root` to choose another
absolute shared directory. The session contains:

- `cases/*.jsonl` with one record per compared program;
- `findings/<kind>/*.js` for mismatches, V8 timeouts/crashes, and slow cases;
- `node-stderr/*.log` for persistent Node/V8 worker exits and async leftovers;
- `all/*.js` only when `--save-all` is requested;
- `fuzzilli/` with Fuzzilli corpus, crashes, timeouts, and statistics;
- `summary.txt` and the final detailed `fuzzilli-*.log`.

Generated scripts that matter for follow-up triage are saved as JavaScript
files, so later agents can reproduce them directly without relying on a stable
Fuzzilli global script number.

## Run

```bash
./fuzzing-differential-test/run.sh --duration 10m --jobs 1
```

Useful options:

```bash
./fuzzing-differential-test/run.sh \
    --duration 10m \
    --jobs 4 \
    --engine-timeout 4s \
    --slow-ratio 10 \
    --slow-min 5ms
```

The launcher rebuilds the differential target from the current checkout before
each run. Use `--skip-build` only when the existing local binaries are known to
match the checkout.

`--resume PATH` resumes the Fuzzilli corpus in an existing session and keeps
appending comparison records under the same shared artifact directory.

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
