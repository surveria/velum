# Benchmark Corpus

This corpus contains benchmark scripts used by `rsqjs-test-runner`.

Benchmark cases are executed sequentially. Local engine measurements always run. QuickJS measurements run when the runner is built with the `reference-quickjs` feature.

Active benchmark scripts must be workload benchmarks, not tiny semantic smoke tests. By default the runner rejects a row when either engine reports a median operation below `1 ms`, when sample variation is above `10%`, or when calibration reaches the iteration cap. Invalid benchmark rows count as failures.

Scale the useful work inside the script with explicit loops or larger data sets. Do not rely on the runner's outer iteration calibration to make a nanosecond or low-microsecond script meaningful. Avoid benchmark stdout; accumulate or return a final value so the work remains observable without polluting reports.

Use ordinary engine tests for cases that are too small, too noisy, or only meant to validate semantics.
