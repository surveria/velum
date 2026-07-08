# JetStream Shell Workload Snapshot

This directory contains a minimized, pinned subset of the official JetStream
benchmark workloads used by `rsqjs-test-runner`.

- Upstream repository: `https://github.com/WebKit/JetStream`
- Upstream commit: `b7babdf323e64e69bd2f6c376189c15825f5c73a`
- Snapshot date: `2026-07-08`
- Source license file: `LICENSE.txt`

The full upstream tree is intentionally not vendored here. The official
JetStream snapshot is large because it includes WebAssembly payloads, browser
workloads, compressed assets, and tooling bundles. This repository tracks only
the shell-compatible JavaScript workload files that are useful for the current
engine surface.

## Active Cases

- `simple/hash-map.js`: synchronous JavaScript workload with no browser or
  WebAssembly dependency.

## Tracked But Skipped Cases

- `simple/doxbee-promise.js`: requires promise-completion support in the runner
  shell bridge.
- `simple/doxbee-async.js`: requires async function completion support in the
  runner shell bridge.

Skipped cases are kept in the snapshot so the report can show explicit coverage
gaps instead of silently omitting known JetStream shell workloads.
