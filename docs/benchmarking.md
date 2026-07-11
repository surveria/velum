# Benchmarking

The target is not to beat desktop JIT engines. The target is to stay close to QuickJS-style footprint and startup while keeping the implementation safe and controllable.

## Validation Lanes

Correctness and performance are deliberately separate:

- `scripts/check-fast.sh` is the local iteration loop. It does not download external corpora or run benchmarks.
- `scripts/check-correctness.sh` is the required ready-PR and merge-queue gate. It runs all formatting, lint, test, documentation, active-fixture, QuickJS differential, and full Test262 checks, but no project or JetStream benchmarks.
- After a merge, CI checks out the exact merge commit and runs only the prepared project sentinel set with the committed QuickJS baseline. It does not prepare QuickJS shell/Test262 inputs or rerun correctness. The publisher composes this performance model with the already-required exact-tree correctness model, preserving both Test262 progress and merge-to-merge performance history.
- `scripts/test-all.sh` is the explicit combined/manual lane. It defaults to the five project sentinels with JetStream disabled; set `RSQJS_BENCH_SET`, a focused filter, or the dedicated JetStream switch explicitly when deeper diagnostics are needed. It is not a routine local prerequisite for an ordinary feature PR.
- `scripts/run-jetstream.sh` is the standalone JetStream lane. It does not run engine fixtures, Test262, differential checks, or project benchmarks.

Correctness, preparation, and compilation do not acquire a host lock. The Rust runner acquires an exclusive `flock` only immediately around measured project or JetStream benchmark execution and releases it before report rendering. Benchmark cases remain sequential inside that interval, while correctness runs may overlap freely. Local worktrees use `/run/lock/rsqjs/host-performance.lock`. The runner deployment bind-mounts that host directory at `/host/rsqjs-lock`, and CI sets `RSQJS_HOST_LOCK_PATH=/host/rsqjs-lock/host-performance.lock`, so all runner containers and interactive agents lock the same inode. Container-private `/tmp` and `/run` paths are not suitable. The sibling `.owner` file is diagnostic only; lock release follows the kernel file descriptor and remains safe after a crash. `scripts/with-host-lock.sh` remains available for explicit maintenance operations that must cover an entire command.

Feature and compatibility work should run focused project tests plus `scripts/check-test262-focused.sh <path-fragment>` and `scripts/check-fast.sh`, push the draft branch, and let required CI perform the complete correctness pass once. Do not run the complete Test262 corpus before pushing: ready-PR and merge-queue CI already provide the required gate. Performance work should use an exact-id or explicit-prefix project filter while iterating and leave the canonical per-merge sentinel measurement to CI. Use `RSQJS_BENCH_SET=full` only when the complete legacy project corpus is intentionally required.

## Structured Report Artifacts

Every correctness, performance, combined, or standalone JetStream invocation writes coordinated bounded outputs from one typed report model:

- `rsqjs-test-report-<timestamp>.yaml` is the compact, schema-versioned source for tracked history and rollups.
- `rsqjs-test-report-<timestamp>-component.yaml` is the typed artifact used for correctness/performance composition. It retains complete counts, the single full-corpus feature map, exact failure-category totals, benchmark rows, and at most 30 deterministic actionable groups with a representative case, source, reason, and detail.
- `rsqjs-test-report-<timestamp>.md` is the human-readable view derived from the same bounded component.
- `rsqjs-test-report-<timestamp>-timings.tsv` is a bounded migration view; it never expands back into all Test262 rows.

Both ordinary YAML files have an executable 1,000-line limit, and diagnostic groups have an executable limit of 30. Feature/category totals are computed before truncation, so the bounded document remains numerically complete. `RSQJS_REPORT_EXHAUSTIVE=1` additionally writes local-only `*-exhaustive.yaml` and `*-exhaustive-timings.tsv`; GitHub Actions rejects this flag, and exhaustive paths never enter artifact metadata, uploads, publication, or git.

Schema version `1` stores durations as integer nanoseconds and statuses as enums rather than presentation strings. Prepared benchmark methodology is carried from the typed runner outcome without a table-text round trip: mode and reference source are enums, lifecycle phases have exact raw `duration_ns` values or explicit boundary kinds, and checksums retain their primitive kind and exact number bits/value. Required CI uploads `rsqjs-correctness-<tree>`, while the benchmark-only post-merge lane uploads `rsqjs-reports-<tree>`. The publisher accepts only trusted successful `.github/workflows/ci.yml` runs and composes bounded `correctness` and `performance` components for one requested tree. Correctness remains bound to the trusted run head and artifact commit tree. Performance is bound to the current closed-PR workflow run, the merge commit selected by that event, and the artifact commit tree; the workflow API may still expose the pre-merge PR head when the base advanced before a squash merge. The publisher commits compact YAML, derived Markdown, charts, and the validated exact-tree Test262 pass baseline. New rollups read compact YAML directly; historical Markdown remains a compatibility fallback.

The shared summary chart uses one complete first-parent `main` commit domain for every panel rather than assigning each benchmark family its own report ordinal. A metric point is placed at the commit that added its canonical report, so commits without that measurement retain their horizontal spacing and a later JetStream history starts at its true repository position. Multiple reports added by one commit share one X coordinate and the latest value for each metric wins. While a publisher is preparing a new report before its signed report commit exists, that report uses the single next-commit slot; a retry rebuilds the chart from the refreshed `main` head. If the available `main` history itself crosses a shallow boundary, the renderer falls back to one shared report-order domain instead of independently compressing each panel; unrelated shallow PR refs do not compress a complete `main` history.

If a manual merge admits a newer base than the ready-PR correctness run, dispatch `CI` on `main` with `correctness=true` and the actual 40-character merge commit in `source_commit`. The trusted current workflow checks out that exact historical source, runs the same unfiltered correctness gate, and uploads `rsqjs-correctness-<tree>` without benchmarks. Pull-request and merge-group correctness remain bound to their workflow head. Recovery correctness is instead bound to the completed trusted dispatch plus the artifact commit and tree because the workflow head intentionally remains current `main`. Rerun the failed post-merge publish job only after that exact-tree artifact exists. Do not substitute a correctness artifact from the older PR merge tree.

Standalone JetStream uses the same schema-versioned contract with the `rsqjs-jetstream-report-<timestamp>` prefix. Its compact projection retains every selected official id, status, engine and reference medians/CVs, latency ratio, reference provenance, and reason while keeping both ordinary YAML files below 1,000 lines. The human-readable Markdown keeps case, sampling-wall, latency-budget, and quality columns from the same in-memory outcome. Canonical history commits only the derived Markdown and compact summary YAML; component YAML and bounded TSV remain workflow artifacts, and exhaustive output remains local-only.

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

Correctness/combined scripts prepare a pinned QuickJS reference binary for differential checks or explicitly requested live measurements. The performance-only post-merge path skips shell and Test262 setup, does not compile the in-process reference, and requires every sentinel to match the committed content-addressed baseline exactly; a missing or stale entry is a hard failure. Generated reports, rollups, charts, and artifact metadata live under `target/rsqjs-reports/`; ordinary feature branches must not commit them. Required CI and post-merge CI upload separate exact-tree bounded artifacts, and the publisher composes them before storing the source commit on the hidden `refs/rsqjs/ci-tested-sources` archive ref, copying compact YAML plus derived Markdown into `reports/test-runs/`, and regenerating the rollup and chart.

1. use `RSQJS_QUICKJS` when it points to an executable file;
2. use `qjs` from `PATH` when available;
3. download, checksum, and build QuickJS `2026-06-04` under `target/quickjs`.

Set `RSQJS_QUICKJS_AUTO_SETUP=0` to disable automatic download and build. In that mode, differential checks and QuickJS benchmark columns are reported as skipped unless `RSQJS_QUICKJS` or `qjs` is available.

The runner has two explicit project benchmark modes. Legacy `cold_eval` cases retain the complete script-evaluation diagnostic, including VM creation, compilation, execution, and teardown. Prepared cases follow protocol `prepared-v1`: load source, compile the harness, create and set up one VM, validate one result, warm up, time repeated `__rsqjsBenchRun()` calls, verify the checksum, and tear the VM down. The Rust runner owns every canonical interval with `Instant`; `performance.now()` is available to JavaScript applications but is not a benchmark clock.

Every prepared `run()` returns a primitive deterministic checksum. The runner compares it with the preflight result on every sampled invocation, checks it again through `__rsqjsBenchVerify()`, and requires rs-quickjs and QuickJS to produce equivalent values before reporting a latency ratio. Source loading, compilation (including parsing), setup, warmup, timed run, verification, and teardown durations are recorded separately in the lifecycle column; only the timed `run()` call contributes to per-operation latency.

The low-level runner keeps `full` as its compatibility default, but `scripts/test-all.sh` deliberately defaults to `RSQJS_BENCH_SET=sentinel`. The sentinel set contains five prepared arithmetic, array-index, property-read, function-call, and string-scan cases used by the post-merge lane. `RSQJS_BENCH_FILTER` accepts comma-separated exact ids, or an explicit trailing `*` for prefix selection; a selector that matches nothing is an error instead of a silent empty run. Run the legacy full set in focused filtered chunks so an ordinary bounded YAML report remains within its enforced size contract.

Prepared QuickJS measurements use `tests/corpora/benchmarks/quickjs-baseline.tsv` by default. Each entry is content-addressed by case id, source digest, harness digest, protocol version, complete sampling configuration, reference-engine identity, and host profile. `read` mode may fall back to a compiled live reference on a missing key, `require` rejects every miss and is mandatory for per-merge CI, `refresh` remeasures and replaces the case entry, and `off` disables the store. Refresh the sentinel reference explicitly after a benchmark, harness, sampling, reference-engine, or target-host change:

```bash
RSQJS_BENCH_SET=sentinel \
RSQJS_QUICKJS_BASELINE=refresh \
cargo run --release --manifest-path runner/Cargo.toml \
  --features reference-quickjs -- \
  --benchmarks target/rsqjs-reports/sentinel-refresh.md
```

Every direct local `--benchmarks` invocation is locked by the runner only for measured execution. The rs-quickjs benchmark adapter uses only the public runtime API and applies a benchmark-only resource envelope that is larger than default embedder limits. The five prepared sentinels are the first migrated tranche; `RSQJS_BENCH_SET=full` still includes legacy cold-eval cases while they are converted deliberately instead of being reinterpreted silently.

## JetStream Shell Benchmarks

The runner also executes a pinned, minimized JetStream shell workload snapshot from `tests/external/jetstream/`. The full upstream JetStream repository is intentionally not vendored because it includes browser workloads, WebAssembly payloads, compressed assets, and tooling bundles that are outside the current embedded shell engine surface. The checked-in snapshot records the upstream commit and keeps only selected JavaScript workload files that can be audited in this repository without repeated network downloads.

JetStream shell reports belong to an independent lane rather than the required or per-merge lane. `scripts/run-jetstream.sh` prepares and compiles without a lock; the runner locks only the measured suite and then writes Markdown, compact schema-v1 YAML, bounded component YAML/TSV, and artifact metadata under `target/rsqjs-reports/`. `.github/workflows/jetstream.yml` runs weekly at 03:17 UTC on Sunday and on explicit dispatch; it is not a dependency of `CI`, a pull request, a merge group, or post-merge sentinel publication. An unfiltered read-only run may publish only canonical Markdown plus compact YAML under `reports/jetstream-runs/` and update the shared rollup. Rollup discovery is independent of ordinary test-report timestamps, so a weekly JetStream result keeps its own task/commit provenance. Filtered and QuickJS-refresh runs remain downloadable artifacts for review, and a partial incremental baseline is uploaded even when a refresh command fails later.

`RSQJS_JETSTREAM_FILTER` accepts comma-separated exact ids or explicit trailing-`*` prefixes. Every selector must match, and unfiltered runs retain all official candidate rows, including statically unsupported cases. Candidate execution failures, invalid measurements, and unsupported rows remain visible but non-blocking. Infrastructure errors and a missing or stale reference baseline fail the standalone command after writing the diagnostic report.

Normal runs read QuickJS data from `tests/corpora/jetstream/quickjs-baseline.tsv`, do not enable the runner's `reference-quickjs` feature, and never fall back to live QuickJS. The typed configuration records that fact together with the 90-second operation cap, 240-second adaptive-sampling cap, and 120/900-second suite wall budget. Each entry is content-addressed by case id, vendored workload bytes, the exact reference prelude and shell harness, protocol version, the complete JetStream sampling configuration, reference engine identity, and host profile. Entries are tagged as either a measured snapshot or a deterministic unavailable/error outcome, so unsupported reference candidates cannot trigger accidental live work. Refresh is explicit and uses the same exclusive lock:

```bash
# Short reviewable refresh while editing one workload.
RSQJS_JETSTREAM_FILTER=hash-map \
RSQJS_JETSTREAM_QUICKJS_BASELINE=refresh \
./scripts/run-jetstream.sh

# Full refresh only after workload, harness, config, reference, or host changes.
RSQJS_JETSTREAM_QUICKJS_BASELINE=refresh \
./scripts/run-jetstream.sh
```

Inspect and commit the deterministic TSV change separately from generated reports. A full reference refresh is intentionally much slower than the normal read lane; do not run it as routine validation.

JetStream shell rows compare rs-quickjs and QuickJS on the same vendored workload source. The reported `latency_ratio` is `rsqjs_median / quickjs_median`, so `1.00x` means QuickJS parity and lower is better. A `28.00x` row means rs-quickjs took about 28 times as long as QuickJS for that workload. Rows above `1.00x` are tracked exceptions while the baseline is still below target. Unsupported, failing, or invalid JetStream candidates stay visible in the JetStream table, but they are non-blocking coverage rows so expanding the external benchmark set does not make ordinary CI fail only because the current engine lacks a feature.

The current integration does not run the official JetStream `cli.js` driver. That driver and several official workloads require JavaScript syntax and async completion behavior that are not implemented in the local shell runner yet. Until those gaps are closed, supported JetStream shell cases use a runner-owned synchronous harness over vendored official workload files, and unsupported shell cases are reported as skipped with concrete reasons.

JetStream whole-iteration measurements have their own defaults instead of inheriting project microbenchmark budgets: one warmup call, at least three samples, one attempt, a 15% maximum coefficient of variation, a 90-second per-operation cap, and bounded adaptive sampling. `RSQJS_JETSTREAM_SUITE_MAX_SECONDS` caps the suite at 120 seconds in read/off mode and 900 seconds in explicit refresh mode. Once exhausted, all remaining selected official rows are emitted as skipped with a concrete suite-budget reason. The worst case is therefore the suite budget plus the currently executing bounded operation, not the number of candidates multiplied by the per-operation limit.

The corresponding overrides are `RSQJS_JETSTREAM_WARMUP_MS`, `RSQJS_JETSTREAM_MIN_TIME_MS`, `RSQJS_JETSTREAM_SAMPLES`, `RSQJS_JETSTREAM_MIN_OP_US`, `RSQJS_JETSTREAM_MAX_CV_PERCENT`, `RSQJS_JETSTREAM_ATTEMPTS`, `RSQJS_JETSTREAM_MAX_OP_MS`, and `RSQJS_JETSTREAM_MAX_TOTAL_MS`. Do not weaken canonical workflow settings merely to turn an invalid measurement green.

## Test262 Reference

`scripts/check-correctness.sh` and `scripts/test-all.sh` prepare a pinned checkout of the official Test262 corpus before running the Rust test runner. Test files execute through a bounded Rayon pool controlled by `RSQJS_TEST_JOBS`: ready-PR and merge-queue CI defaults to 30 workers on the 32-logical-CPU runner, while direct local full runs retain a conservative default of four. All variants for a file stay on one worker and report rows are sorted back into deterministic path order. The setup order is:

1. use `RSQJS_TEST262_DIR` when it points to a directory;
2. materialize Test262 commit `64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1` under `target/test262`.

Set `RSQJS_TEST262_AUTO_SETUP=0` to disable automatic materialization. In that mode, upstream rows that need source files are reported as skipped.

The committed `tests/corpora/test262/full-pass-baseline.txt` records every variant that passes at the pinned upstream commit. A complete unfiltered run fails only when a known pass regresses or disappears. New passes remain visible in the report but do not force an agent to repeat the complete corpus locally: correctness CI writes an exact-tree `test262-pass-baseline.txt` candidate into its artifact, and the trusted post-merge publisher validates and commits that candidate together with the canonical report. `RSQJS_TEST262_UPDATE_PASS_BASELINE=1` remains an explicit maintenance escape hatch, not the normal feature workflow. The active fixture registry is checked independently so adding a JavaScript fixture without registering it cannot silently reduce coverage.

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
- `RSQJS_BENCH_SET` selects `full` (the low-level runner default) or `sentinel` (the repository script and per-merge default).
- `RSQJS_BENCH_FILTER` selects exact case ids or explicit trailing-`*` prefixes.
- `RSQJS_QUICKJS_BASELINE` selects `read` (the local default), strict `require`, `refresh`, or `off`.
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
