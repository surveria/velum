# JetStream Shell Workload Snapshot

This directory contains a minimized, pinned subset of the official JetStream
benchmark workloads used by `rsqjs-test-runner`.

- Upstream repository: `https://github.com/WebKit/JetStream`
- Upstream commit: `b7babdf323e64e69bd2f6c376189c15825f5c73a`
- Snapshot date: `2026-07-08`
- Source license file: `LICENSE.txt`

The full upstream tree is intentionally not vendored here. The official
JetStream snapshot is large because it includes WebAssembly payloads, browser
workloads, compressed assets, compressed data files, and tooling bundles. This
repository tracks the JavaScript workload files that can be audited and run or
reported from the current shell harness without repeated network downloads.

## Included Workload Families

- `ARES-6/`
- `Octane/`
- `RexBench/`
- `SeaMonster/`
- `SunSpider/`
- `bigint/`
- `cdjs/`
- `class-fields/`
- `code-load/`
- `generators/`
- `proxy/`
- `simple/`
- `validatorjs/`

## Report Semantics

The runner reports every configured JetStream case explicitly. Cases that run
under both rs-quickjs and QuickJS get a `latency_ratio` equal to
`rsqjs_median / quickjs_median`, matching the main benchmark report semantics.
Cases that need unsupported syntax, async completion, browser APIs, preload
resources, or WebAssembly remain visible as failed or skipped JetStream rows.

JetStream candidate failures are non-blocking coverage rows. They are kept in
the report so unsupported official workloads are visible instead of silently
omitted while the engine is still growing toward broader compatibility.
