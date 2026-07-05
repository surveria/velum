# Architecture

The engine starts as a small safe-Rust interpreter and should evolve toward a compact bytecode VM only when the language surface and benchmark suite justify it.

## Layers

1. `lexer`: converts source text into tokens.
2. `parser`: builds a small AST for the currently supported language subset.
3. `runtime`: owns globals, host functions, output, and resource counters.
4. `value`: defines the current JavaScript value model.

The current AST evaluator is deliberately simple. It lets us validate resource accounting, embedding API shape, and language tests before adding a bytecode compiler.

## Embedding Model

The library API is the product surface. The CLI exists for smoke tests, differential checks, and benchmark orchestration, but engine architecture must optimize for embedding inside a larger Rust application.

The public model should evolve around these roles:

- `Engine`: shared immutable configuration, feature flags, parser caches, atom tables, and other data that can be reused safely across isolated virtual machines.
- `Vm`: one isolated JavaScript virtual machine with its own heap, globals, job queue, resource counters, and teardown report.
- `Context`: an execution view into a `Vm`, used to evaluate scripts, inspect values, and register host bindings.
- `CompiledScript`: a reusable compiled representation. It can start as an AST wrapper and later become bytecode without changing the embedding API.
- `HostFunctionRegistry`: synchronous and asynchronous Rust callbacks exposed to JavaScript as functions.

Multiple `Vm` instances must be able to run in the same Rust process without sharing mutable JavaScript state. A failure, resource-limit hit, pending job, or global mutation in one VM must not affect another VM. Shared data is allowed only when it is immutable or protected by explicit synchronization and resource accounting.

Host extensions are a first-class design concern:

- Rust code must be able to register typed host functions under explicit names.
- Host functions must receive arguments through checked conversions and return `Result` values with contextual errors.
- Asynchronous host functions must integrate with a VM-owned job queue and return JavaScript promises once promises exist.
- The embedding application, not the engine, should own the outer async executor. The engine should expose explicit polling or job-draining APIs instead of depending on a specific runtime.
- Host callbacks must have per-callback quotas for runtime steps, allocations, output, and wall-clock cancellation hooks.
- Host callbacks must never bypass VM isolation or leak values across VM boundaries without an explicit serialization or transfer step.

## Safety Policy

The crate uses `#![deny(unsafe_code)]`, and the lint is also declared in `Cargo.toml`.

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

Every new VM-facing feature should define how it participates in limits before it is considered complete. This includes parser work, runtime steps, heap growth, host callback calls, queued jobs, module loads, and output buffering.

## Compatibility Strategy

QuickJS is the reference engine, not code to transliterate line by line. The compatibility path should be:

1. add focused engine-specific tests for a feature
2. compare behavior against QuickJS
3. add or identify the relevant Test262 coverage
4. implement the safe Rust model
5. add performance and memory baselines
6. only then widen the supported Test262 subset or mark the feature as covered

Feature work should not rely on Test262 alone. Project-specific tests must cover embedding behavior, resource limits, host extension behavior, and regressions that are important for surveillance-device workloads even when those cases are not represented in upstream ECMAScript tests.
