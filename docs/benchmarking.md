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
