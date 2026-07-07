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

`scripts/test-all.sh` prepares a pinned QuickJS reference binary before running the Rust test runner. By default it writes full reports, benchmark rollups, summary charts, and artifact metadata under `target/rsqjs-reports/` so local agents and ready pull requests can inspect benchmark progress without changing tracked report history. The script prints the exact markdown report path after generation and warns that ordinary feature branches must not commit that generated report. CI uploads `target/rsqjs-reports/` as an artifact named by the tested tree SHA. After a PR is merged, GitHub Actions downloads the artifact for the tested tree, copies the single markdown report into `reports/test-runs/`, and regenerates `reports/benchmark-rollup.md` plus `reports/benchmark-summary.jpg` in one report-only commit. Current canonical reports still preserve the PR artifact commit in their run metadata; on GitHub pull requests this can be a synthetic merge commit that is tree-equivalent to the eventual squash merge commit but may not remain directly reachable after PR refs or task branches are cleaned up. Set `RSQJS_TRACKED_REPORT=1` or `RSQJS_TEST_REPORT_PATH=reports/test-runs/<name>.md` only for intentional manual canonical report refreshes. The setup order is:

1. use `RSQJS_QUICKJS` when it points to an executable file;
2. use `qjs` from `PATH` when available;
3. download, checksum, and build QuickJS `2026-06-04` under `target/quickjs`.

Set `RSQJS_QUICKJS_AUTO_SETUP=0` to disable automatic download and build. In that mode, differential checks and QuickJS benchmark columns are reported as skipped unless `RSQJS_QUICKJS` or `qjs` is available.

The standard test script builds `target/release/rsqjs` and runs `runner/Cargo.toml` with the `reference-quickjs` feature. Current benchmark rows compare the release `rsqjs` CLI with the QuickJS `qjs` CLI sequentially. Each row reports average in-process cold eval latency for the Rust library, compile-only latency, eval latency for a reused `CompiledScript`, average CLI latency, peak process RSS when GNU `time` is available, latency ratio, memory ratio, and the current QuickJS parity budget status.

CLI benchmarks are useful integration smoke tests, but they include process startup and argument handling. The in-process column removes that CLI startup cost for rs-quickjs and should guide local optimization work. Future in-process rows should separate parser, compiler, VM execution, host callback, and teardown costs as those subsystems become explicit.

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

## Coverage Expectations

- Every implemented feature should have project-specific engine tests.
- Every implemented feature with relevant ECMAScript semantics should be represented in Test262 reporting.
- Every performance-sensitive feature should have a benchmark case.
- Benchmark cases should compare rs-quickjs and QuickJS whenever QuickJS supports the same behavior.
- Embedding features need benchmarks for both direct Rust API use and CLI smoke coverage when applicable.

## Measurement Rules

- Keep benchmark scripts checked in.
- Record target CPU, RAM, kernel, compiler, and optimization flags.
- Report both median and tail latency.
- Separate parser, compiler, VM, and host callback costs where possible.
- Compare release builds only.
- Run benchmark cases sequentially.
- Report memory alongside latency once memory measurement is implemented.
- Keep ordinary PR benchmark reports as CI artifacts. Commit tracked report files only through the post-merge report publisher or through intentional report-refresh tasks.
