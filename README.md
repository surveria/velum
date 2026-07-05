# rs-quickjs

`rs-quickjs` is a safe-Rust JavaScript engine prototype. The long-term goal is to keep the parts that make `QuickJS` attractive for embedded Linux devices, while replacing the native C engine with code that can be audited and evolved as Rust.

This repository is intentionally starting small. It is not a drop-in replacement for `QuickJS` or `rquickjs` yet.

## Goals

- Safe Rust core: no `unsafe` blocks in the engine crate.
- Small footprint: keep startup and hello-world memory use close to the `QuickJS` class of engines.
- Predictable library embedding: make the Rust API the primary product surface, with many isolated virtual machines per process, explicit resource limits, deterministic teardown, typed host extensions, async host-callback support, and inspectable execution state.
- Reference-driven compatibility: use `QuickJS` behavior, focused Test262 subsets, and full-corpus Test262 progress reports instead of inventing a new language.
- Device-oriented performance: optimize for ARM Linux systems with tens of megabytes of RAM, and keep implemented benchmark cases within 1.10x of `QuickJS` unless a tracked exception explains the gap.

## Current MVP

The first implementation provides a tiny interpreter for a JavaScript-like subset:

- number, string, bool, `null`, and `undefined` values
- arithmetic, comparison, equality, unary, and logical expressions
- `let`, `const`, and `var` bindings
- assignment to mutable bindings
- a host `print(...)` function
- configurable runtime limits for source size, statement count, expression depth, runtime steps, strings, and bindings

The MVP exists to make CI, API shape, resource limits, and test infrastructure real from day one.

## Quick Start

```sh
cargo test
cargo run --bin rsqjs -- -e 'let x = 40 + 2; print("answer", x); x'
```

## Reference Projects

- [QuickJS](https://bellard.org/quickjs/) remains the behavioral and footprint reference.
- [rquickjs](https://docs.rs/rquickjs/latest/rquickjs/) is the current Rust binding approach around the native `QuickJS` engine.

## Project Docs

- [Project Rules](AGENTS.md)
- [Architecture](docs/architecture.md)
- [Roadmap](docs/roadmap.md)
- [Benchmarking](docs/benchmarking.md)
