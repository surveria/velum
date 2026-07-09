# Benchmarking

The target is not to beat desktop JIT engines. The target is to stay close to QuickJS-style footprint and startup while keeping the implementation safe and controllable.

## Validation Lanes

Correctness and performance are deliberately separate:

- `scripts/check-fast.sh` is the local iteration loop. It does not download external corpora or run benchmarks.
- `scripts/check-correctness.sh` is the required ready-PR and merge-queue gate. It runs all formatting, lint, test, documentation, active-fixture, QuickJS differential, and full Test262 checks, but no project or JetStream benchmarks.
- After a merge, CI checks out the exact merge commit and runs the prepared project sentinel set with the committed QuickJS baseline. This preserves a merge-to-merge performance history without putting sequential measurements on the merge critical path.
- `scripts/test-all.sh` is the explicit full/manual lane and includes JetStream. It is not a routine local prerequisite for an ordinary feature PR.

All test-oriented entrypoints acquire a shared `flock` through `scripts/with-host-lock.sh`; benchmark entrypoints acquire the exclusive form of the same lock. Correctness runs can therefore overlap with each other, while timing work cannot overlap with tests or another benchmark. Local worktrees use `/run/lock/rsqjs/host-performance.lock`. The runner deployment bind-mounts that host directory at `/host/rsqjs-lock`, and CI sets `RSQJS_HOST_LOCK_PATH=/host/rsqjs-lock/host-performance.lock`, so all runner containers and interactive agents lock the same inode. Container-private `/tmp` and `/run` paths are not suitable. The sibling `.owner` file is diagnostic only; lock release follows the kernel file descriptor and remains safe after a crash.

Feature and compatibility work should run focused tests plus `scripts/check-fast.sh`, push the draft branch, and let required CI perform the complete correctness pass once. Performance work should use an exact-id or explicit-prefix project filter while iterating and leave the canonical per-merge sentinel measurement to CI. Use `RSQJS_BENCH_SET=full` only when the complete legacy project corpus is intentionally required.

## Structured Report Artifacts

Every correctness or full runner invocation writes four coordinated outputs from one typed report model:

- `rsqjs-test-report-<timestamp>.yaml` is the compact, schema-versioned source for tracked history and rollups. It contains run/build metadata, host environment, effective configuration, numeric `duration_ns` fields, typed statuses, suite totals, feature summaries, and all project/JetStream benchmark rows.
- `rsqjs-test-report-<timestamp>-details.yaml` uses the same schema and additionally contains every case row. It remains in the tree-keyed CI artifact so the 100,000-plus Test262 variants do not inflate git history.
- `rsqjs-test-report-<timestamp>.md` is the human-readable view rendered from the typed model.
- `rsqjs-test-report-<timestamp>-timings.tsv` remains available during migration for existing analysis tools.

Schema version `1` stores durations as integer nanoseconds and statuses as enums rather than presentation strings. New rollups read the compact YAML directly; historical reports without YAML retain a Markdown compatibility fallback. The post-merge publisher validates both YAML artifacts, commits only the compact YAML plus derived Markdown, and leaves the detailed YAML and TSV in the downloadable CI artifact.

## Baselines

Track these against QuickJS on every supported device class:

- hello-world resident memory
- engine, VM, and context creation and teardown latency
- cold eval latency
- parse-only latency
- eval of a precompiled script
- arithmetic loops
- object and array allocation
- property lookup
- function calls
- synchronous host callbacks
- asynchronous host callbacks once the promise/job model exists
- string concatenation
- JSON parse and stringify when implemented
- selected `bench-v8` cases when coverage is sufficient

## QuickJS Reference

The report scripts prepare a pinned QuickJS reference binary before running differential checks or reference measurements. Generated reports, rollups, charts, and artifact metadata live under `target/rsqjs-reports/`; ordinary feature branches must not commit them. Required CI uploads a correctness artifact named by the tested tree. The separate post-merge job measures the exact merge commit, uploads the performance artifact consumed by the publisher, stores that source commit on the hidden `refs/rsqjs/ci-tested-sources` archive ref, copies the compact YAML source and its derived Markdown into `reports/test-runs/`, and regenerates `reports/benchmark-rollup.md` plus `reports/benchmark-summary.jpg` in a signed report-only commit. The publisher can read the legacy `refs/heads/ci-tested-sources` branch as a migration base, but new archive commits are pushed only to the hidden ref. Set `RSQJS_TRACKED_REPORT=1` or `RSQJS_TEST_REPORT_PATH=reports/test-runs/<name>.md` only for intentional manual canonical report refreshes. The setup order is:

1. use `RSQJS_QUICKJS` when it points to an executable file;
2. use `qjs` from `PATH` when available;
3. download, checksum, and build QuickJS `2026-06-04` under `target/quickjs`.

Set `RSQJS_QUICKJS_AUTO_SETUP=0` to disable automatic download and build. In that mode, differential checks and QuickJS benchmark columns are reported as skipped unless `RSQJS_QUICKJS` or `qjs` is available.

The runner has two explicit project benchmark modes. Legacy `cold_eval` cases retain the complete script-evaluation diagnostic, including VM creation, compilation, execution, and teardown. Prepared cases follow protocol `prepared-v1`: load source, compile the harness, create and set up one VM, validate one result, warm up, time repeated `__rsqjsBenchRun()` calls, verify the checksum, and tear the VM down. The Rust runner owns every canonical interval with `Instant`; `performance.now()` is available to JavaScript applications but is not a benchmark clock.

Every prepared `run()` returns a primitive deterministic checksum. The runner compares it with the preflight result on every sampled invocation, checks it again through `__rsqjsBenchVerify()`, and requires rs-quickjs and QuickJS to produce equivalent values before reporting a latency ratio. Source loading, compilation (including parsing), setup, warmup, timed run, verification, and teardown durations are recorded separately in the lifecycle column; only the timed `run()` call contributes to per-operation latency.

The default benchmark set remains `full` for backward compatibility. `RSQJS_BENCH_SET=sentinel` selects five prepared arithmetic, array-index, property-read, function-call, and string-scan cases used by the post-merge lane. `RSQJS_BENCH_FILTER` accepts comma-separated exact ids, or an explicit trailing `*` for prefix selection; a selector that matches nothing is an error instead of a silent empty run.

Prepared QuickJS measurements use `tests/corpora/benchmarks/quickjs-baseline.tsv` by default. Each entry is content-addressed by case id, source digest, harness digest, protocol version, complete sampling configuration, reference-engine identity, and host profile (architecture, OS, CPU model, logical CPU count, kernel, and governor). `read` mode uses only an exact key match and measures QuickJS live on a missing or stale key. `refresh` mode remeasures and deterministically replaces the stale entry for the same case; `off` disables the store. Refresh the sentinel reference after a benchmark, harness, sampling, reference-engine, or target-host change:

```bash
RSQJS_BENCH_SET=sentinel \
RSQJS_QUICKJS_BASELINE=refresh \
./scripts/with-host-lock.sh exclusive -- \
  cargo run --release --manifest-path runner/Cargo.toml \
    --features reference-quickjs -- \
    --benchmarks target/rsqjs-reports/sentinel-refresh.md
```

Every direct local `--benchmarks` invocation must use the exclusive `scripts/with-host-lock.sh` wrapper shown above. The rs-quickjs benchmark adapter uses only the public runtime API and applies a benchmark-only resource envelope that is larger than default embedder limits. The five prepared sentinels are the first migrated tranche; `RSQJS_BENCH_SET=full` still includes legacy cold-eval cases while they are converted deliberately instead of being reinterpreted silently.

## JetStream Shell Benchmarks

The runner also executes a pinned, minimized JetStream shell workload snapshot from `tests/external/jetstream/`. The full upstream JetStream repository is intentionally not vendored because it includes browser workloads, WebAssembly payloads, compressed assets, and tooling bundles that are outside the current embedded shell engine surface. The checked-in snapshot records the upstream commit and keeps only selected JavaScript workload files that can be audited in this repository without repeated network downloads.

JetStream shell reports belong to the explicit full/manual lane rather than the required or per-merge lane. They are generated under `target/rsqjs-reports/jetstream-runs/`; intentional tracked refreshes use `reports/jetstream-runs/`. `scripts/test-all.sh` prints both report paths. Ordinary feature branches must not commit either generated path. A canonical JetStream refresh feeds the shared `reports/benchmark-rollup.md` and `reports/benchmark-summary.jpg`, but its long-running shell workloads never delay a normal merge.

JetStream shell rows compare rs-quickjs and QuickJS on the same vendored workload source. The reported `latency_ratio` is `rsqjs_median / quickjs_median`, so `1.00x` means QuickJS parity and lower is better. A `28.00x` row means rs-quickjs took about 28 times as long as QuickJS for that workload. Rows above `1.00x` are tracked exceptions while the baseline is still below target. Unsupported, failing, or invalid JetStream candidates stay visible in the JetStream table, but they are non-blocking coverage rows so expanding the external benchmark set does not make ordinary CI fail only because the current engine lacks a feature.

The current integration does not run the official JetStream `cli.js` driver. That driver and several official workloads require JavaScript syntax and async completion behavior that are not implemented in the local shell runner yet. Until those gaps are closed, supported JetStream shell cases use a runner-owned synchronous harness over vendored official workload files, and unsupported shell cases are reported as skipped with concrete reasons.

## Test262 Reference

`scripts/check-correctness.sh` and `scripts/test-all.sh` prepare a pinned checkout of the official Test262 corpus before running the Rust test runner. Test files execute through a bounded Rayon pool controlled by `RSQJS_TEST_JOBS` (default four), while all variants for a file stay on one worker and report rows are sorted back into deterministic path order. The setup order is:

1. use `RSQJS_TEST262_DIR` when it points to a directory;
2. materialize Test262 commit `64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1` under `target/test262`.

Set `RSQJS_TEST262_AUTO_SETUP=0` to disable automatic materialization. In that mode, upstream rows that need source files are reported as skipped.

The committed `tests/corpora/test262/full-pass-baseline.txt` records every variant that passes at the pinned upstream commit. A complete unfiltered run fails if a known pass regresses or if a new pass is not acknowledged. Refresh it only for an intentional compatibility change with `RSQJS_TEST262_UPDATE_PASS_BASELINE=1 ./scripts/check-correctness.sh`, inspect the changed IDs, and commit the baseline with the implementation. The active fixture registry is checked independently so adding a JavaScript fixture without registering it cannot silently reduce coverage.

## Performance Targets

- implemented benchmark cases should run at or below 1.00x of QuickJS on the same device class
- hello-world resident memory should stay at or below 1.00x of QuickJS once memory measurement is available
- VM creation and teardown latency should stay at or below 1.00x of QuickJS once in-process measurements are available
- no unbounded allocations without a runtime limit path

The 1.00x budget applies to features that are implemented locally and have comparable QuickJS behavior. A slower result is allowed only when the report marks it as a tracked exception with the suspected cause, affected benchmark, and follow-up work. The current CI report records over-budget benchmark rows as tracked exceptions rather than hard failures until the baseline is below the target; once that happens, the same metrics should become a regression gate.

Memory reporting should track both peak resident memory and engine-owned heap counters where available. The current report uses process-level maximum resident set size for CLI parity. The long-term target is VM-level accounting exposed through the library API.

## Measurement Quality Gate

Benchmark cases are allowed into the active corpus only when they produce stable, measurable operations. By default the runner marks a row as an invalid benchmark when either engine reports a median operation below `1 ms`, when the sample coefficient of variation is above `10%`, or when calibration reaches the per-sample iteration cap. Invalid benchmark rows count as failed rows and make the benchmark command exit non-zero.

Use regular engine tests for cheap semantic coverage. Active benchmark scripts should scale the workload inside the script until one measured operation is large enough to clear the quality gate. The runner still calibrates outer iterations and samples, but it must not be asked to interpret nanosecond or low-microsecond operations as meaningful performance signals.

The timing and quality thresholds can be adjusted for local diagnosis:

- `RSQJS_BENCH_WARMUP_MS` controls warmup duration before sampling.
- `RSQJS_BENCH_MIN_TIME_MS` controls the target minimum time for one sample.
- `RSQJS_BENCH_SAMPLES` controls the number of measured samples.
- `RSQJS_BENCH_MIN_OP_US` controls the minimum valid median operation time.
- `RSQJS_BENCH_MAX_CV_PERCENT` controls the maximum valid sample coefficient of variation.
- `RSQJS_BENCH_MAX_OP_MS` rejects an operation after its first over-limit execution so it is not repeated blindly.
- `RSQJS_BENCH_MAX_TOTAL_MS` bounds retries and adaptively reduces the requested sample count after the minimum robust sample count is collected.
- `RSQJS_BENCH_SET` selects `full` (the default) or the prepared `sentinel` set.
- `RSQJS_BENCH_FILTER` selects exact case ids or explicit trailing-`*` prefixes.
- `RSQJS_QUICKJS_BASELINE` selects `read` (the default), `refresh`, or `off`.
- `RSQJS_QUICKJS_BASELINE_PATH` overrides the committed baseline path.

Do not weaken these thresholds in CI to make a benchmark pass. Fix the benchmark workload or move the case back to tests if it is not a meaningful performance measurement.

## In-VM Clock

The engine exposes a minimal `performance.now()` for application
instrumentation and portable workload phase markers. Its origin and
non-decreasing state belong to one VM, and embedders can inject a duration
reader through `Engine::create_vm_with_clock`, `Vm::with_config_and_clock`, or
`Runtime::context_with_clock` when deterministic time is required. A source
regression is clamped to the VM's previous reading.

`performance.now()` is not the canonical benchmark timer. Calling it from
JavaScript adds native dispatch overhead and lets workload code choose the
measured boundary. Canonical reports must time prepared execution from the
external Rust harness and record compile, setup, validation, execution, and
teardown phases separately. In-script readings may be retained as diagnostic
metadata, but they must not replace the harness measurement used for QuickJS
comparison or regression decisions.

## Coverage Expectations

- Every implemented feature should have project-specific engine tests.
- Every implemented feature with relevant ECMAScript semantics should be represented in Test262 reporting.
- Every performance-sensitive feature should have a benchmark case.
- Benchmark cases should compare rs-quickjs and QuickJS whenever QuickJS supports the same behavior.
- Embedding features need benchmarks for both direct Rust API use and CLI smoke coverage when applicable.

## Measurement Rules

- Keep benchmark scripts checked in.
- Keep active benchmark scripts large enough to pass the measurement quality gate.
- Use regular tests, not benchmarks, for tiny semantic smoke checks.
- Record target CPU, RAM, kernel, compiler, and optimization flags.
- Report median latency and sample variation.
- Separate parser, compiler, VM, and host callback costs where possible.
- Use a deterministic primitive checksum and keep setup/verification outside the timed interval.
- Treat JavaScript clock readings as diagnostic data, never as the canonical timer.
- Compare release builds only.
- Run benchmark cases sequentially.
- Avoid benchmark stdout. Return or accumulate a final value so the measured work stays observable without polluting reports.
- Report memory alongside latency once memory measurement is implemented.
- Keep required-PR correctness reports and post-merge benchmark reports as separate CI artifacts. Commit tracked report files only through the post-merge report publisher or through intentional report-refresh tasks.
