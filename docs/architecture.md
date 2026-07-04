# Architecture

The engine starts as a small safe-Rust interpreter and should evolve toward a compact bytecode VM only when the language surface and benchmark suite justify it.

## Layers

1. `lexer`: converts source text into tokens.
2. `parser`: builds a small AST for the currently supported language subset.
3. `runtime`: owns globals, host functions, output, and resource counters.
4. `value`: defines the current JavaScript value model.

The current AST evaluator is deliberately simple. It lets us validate resource accounting, embedding API shape, and language tests before adding a bytecode compiler.

## Safety Policy

The crate uses `#![forbid(unsafe_code)]` and the CI repeats this with `RUSTFLAGS=-Dunsafe-code`.

If future performance work appears to require unsafe code, it must go through a separate design document with:

- the exact measured bottleneck
- the safe alternative that was rejected
- memory and aliasing invariants
- fuzzing and sanitizer coverage
- a review path that keeps unsafe code isolated from the public embedding API

The default answer should remain safe Rust.

## Resource Model

`RuntimeLimits` is part of the public API and is checked by the parser and evaluator. The goal is to make resource use explicit at the embedding boundary instead of relying on global process limits.

Current limits cover:

- source length
- statement count
- expression nesting depth
- runtime evaluation steps
- string length
- number of global bindings

Future limits should cover heap budgets, atom table budgets, stack budgets, module loading, and host callback quotas.

## Compatibility Strategy

QuickJS is the reference engine, not code to transliterate line by line. The compatibility path should be:

1. add focused language tests for a feature
2. compare behavior against QuickJS
3. implement the safe Rust model
4. add performance and memory baselines
5. only then widen the supported Test262 subset
