# Benchmark Corpus

This corpus contains benchmark scripts used by `velum-test-runner`.

Benchmark cases are executed sequentially. Local engine measurements always run. QuickJS measurements run when the runner is built with the `reference-quickjs` feature.

The default `full` set preserves all project cases. `VELUM_BENCH_SET=sentinel` runs the small prepared merge-to-merge set. Filters are comma-separated exact ids; use a trailing `*` only when a prefix is intentional.

Prepared sources under `prepared/` must define three global functions:

- `__velumBenchSetup()` builds reusable state outside the measured interval.
- `__velumBenchRun()` performs the thematic operation and returns the same primitive checksum on every invocation.
- `__velumBenchVerify()` returns the known checksum without repeating the workload.

The runner compiles and sets up the source once, validates every sampled `run()` result, compares the checksum with QuickJS, and records teardown separately. Do not use `performance.now()` to define the canonical interval; the Rust runner times only the prepared run call.

Active benchmark scripts must be workload benchmarks, not tiny semantic smoke tests. By default the runner rejects a row when either engine reports a median operation below `1 ms`, when sample variation is above `10%`, or when calibration reaches the iteration cap. Invalid benchmark rows count as failures.

Scale the useful work inside the script with explicit loops or larger data sets. Do not rely on the runner's outer iteration calibration to make a nanosecond or low-microsecond script meaningful. Avoid benchmark stdout; accumulate or return a final value so the work remains observable without polluting reports.

Use ordinary engine tests for cases that are too small, too noisy, or only meant to validate semantics.

When a prepared source, protocol, sampling configuration, reference engine, or benchmark host changes, refresh and review `quickjs-baseline.tsv` with `VELUM_QUICKJS_BASELINE=refresh`. Read mode never reuses a partial key match.
