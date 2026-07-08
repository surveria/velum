# Benchmarking

The target is not to beat desktop JIT engines. The target is to stay close to QuickJS-style footprint and startup while keeping the implementation safe and controllable.

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

`scripts/test-all.sh` prepares a pinned QuickJS reference binary before running the Rust test runner. By default it writes full reports, benchmark rollups, summary charts, and artifact metadata under `target/rsqjs-reports/` so local agents and ready pull requests can inspect benchmark progress without changing tracked report history. The script prints the exact markdown report path after generation and warns that ordinary feature branches must not commit that generated report. CI uploads `target/rsqjs-reports/` as an artifact named by the tested tree SHA. After a PR is merged, GitHub Actions downloads the artifact for the tested tree, stores the tested source commit on the single `ci-tested-sources` archive branch, copies the single markdown report into `reports/test-runs/`, and regenerates `reports/benchmark-rollup.md` plus `reports/benchmark-summary.jpg` in one report-only commit created through GitHub's signed commit path. Canonical reports preserve the PR artifact commit in their run metadata; on GitHub pull requests this can be a synthetic merge commit that is tree-equivalent to the eventual squash merge commit, so `ci-tested-sources` keeps that commit reachable after PR refs or task branches are cleaned up. Set `RSQJS_TRACKED_REPORT=1` or `RSQJS_TEST_REPORT_PATH=reports/test-runs/<name>.md` only for intentional manual canonical report refreshes. The setup order is:

1. use `RSQJS_QUICKJS` when it points to an executable file;
2. use `qjs` from `PATH` when available;
3. download, checksum, and build QuickJS `2026-06-04` under `target/quickjs`.

Set `RSQJS_QUICKJS_AUTO_SETUP=0` to disable automatic download and build. In that mode, differential checks and QuickJS benchmark columns are reported as skipped unless `RSQJS_QUICKJS` or `qjs` is available.

The standard test script runs `runner/Cargo.toml` with the `reference-quickjs` feature. Current project benchmark rows compare rs-quickjs and QuickJS through the same in-process `eval` adapter, so they measure engine work instead of process startup, shell argument parsing, or report formatting. Each row reports median per-operation latency, calibrated iteration counts, sample coefficient of variation, QuickJS latency ratio when the reference is available, and the current benchmark quality status.

The rs-quickjs benchmark adapter uses only the public runtime API. It applies a benchmark-only resource envelope that is larger than the default library limits, so active workload cases can be large enough for stable timing without changing embedder defaults. Future benchmark rows should separate parser, compiler, VM execution, host callback, and teardown costs as those subsystems become explicit.

## JetStream Shell Benchmarks

The runner also executes a pinned, minimized JetStream shell workload snapshot from `tests/external/jetstream/`. The full upstream JetStream repository is intentionally not vendored because it includes browser workloads, WebAssembly payloads, compressed assets, and tooling bundles that are outside the current embedded shell engine surface. The checked-in snapshot records the upstream commit and keeps only selected JavaScript workload files that can be audited in this repository without repeated network downloads.

JetStream shell reports are generated in addition to the main full report. Local and CI runs write them under `target/rsqjs-reports/jetstream-runs/`; intentional tracked report refreshes write them under `reports/jetstream-runs/`. `scripts/test-all.sh` prints both paths after generation. Ordinary feature branches must not commit either generated report path. The post-merge publisher copies the already-tested JetStream artifact into `reports/jetstream-runs/` and then regenerates the shared `reports/benchmark-rollup.md` and `reports/benchmark-summary.jpg` from both `reports/test-runs/` and `reports/jetstream-runs/`.

JetStream shell rows compare rs-quickjs and QuickJS on the same vendored workload source. The reported `latency_ratio` is `rsqjs_median / quickjs_median`, so `1.00x` means QuickJS parity and lower is better. A `28.00x` row means rs-quickjs took about 28 times as long as QuickJS for that workload. Rows above `1.00x` are tracked exceptions while the baseline is still below target. Unsupported, failing, or invalid JetStream candidates stay visible in the JetStream table, but they are non-blocking coverage rows so expanding the external benchmark set does not make ordinary CI fail only because the current engine lacks a feature.

The current integration does not run the official JetStream `cli.js` driver. That driver and several official workloads require JavaScript syntax and async completion behavior that are not implemented in the local shell runner yet. Until those gaps are closed, supported JetStream shell cases use a runner-owned synchronous harness over vendored official workload files, and unsupported shell cases are reported as skipped with concrete reasons.

## Test262 Reference

`scripts/test-all.sh` also prepares a pinned checkout of the official Test262 corpus before running the Rust test runner. The setup order is:

1. use `RSQJS_TEST262_DIR` when it points to a directory;
2. materialize Test262 commit `64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1` under `target/test262`.

Set `RSQJS_TEST262_AUTO_SETUP=0` to disable automatic materialization. In that mode, upstream rows that need source files are reported as skipped.

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

Do not weaken these thresholds in CI to make a benchmark pass. Fix the benchmark workload or move the case back to tests if it is not a meaningful performance measurement.

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
- Compare release builds only.
- Run benchmark cases sequentially.
- Avoid benchmark stdout. Return or accumulate a final value so the measured work stays observable without polluting reports.
- Report memory alongside latency once memory measurement is implemented.
- Keep ordinary PR benchmark reports as CI artifacts. Commit tracked report files only through the post-merge report publisher or through intentional report-refresh tasks.
