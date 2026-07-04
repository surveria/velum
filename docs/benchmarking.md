# Benchmarking

The target is not to beat desktop JIT engines. The target is to stay close to QuickJS-style footprint and startup while keeping the implementation safe and controllable.

## Baselines

Track these against QuickJS on every supported device class:

- hello-world resident memory
- runtime creation and teardown latency
- cold eval latency
- arithmetic loops
- object and array allocation
- string concatenation
- JSON parse and stringify when implemented
- selected `bench-v8` cases when coverage is sufficient

## QuickJS Reference

`scripts/test-all.sh` prepares a pinned QuickJS reference binary before running the Rust test runner. The setup order is:

1. use `RSQJS_QUICKJS` when it points to an executable file;
2. use `qjs` from `PATH` when available;
3. download, checksum, and build QuickJS `2026-06-04` under `target/quickjs`.

Set `RSQJS_QUICKJS_AUTO_SETUP=0` to disable automatic download and build. In that mode, differential checks and QuickJS benchmark columns are reported as skipped unless `RSQJS_QUICKJS` or `qjs` is available.

The standard test script builds `target/release/rsqjs` and exposes it to the runner through `RSQJS_ENGINE`. Benchmark rows compare the release `rsqjs` CLI with the QuickJS `qjs` CLI sequentially.

## Initial Targets

- hello-world memory: within roughly 2x QuickJS on ARM Linux devices
- startup latency: within roughly 2x QuickJS for small scripts
- no unbounded allocations without a runtime limit path

These targets are directional until a repeatable benchmark harness exists in the repository.

## Measurement Rules

- Keep benchmark scripts checked in.
- Record target CPU, RAM, kernel, compiler, and optimization flags.
- Report both median and tail latency.
- Separate parser, compiler, VM, and host callback costs where possible.
- Compare release builds only.
