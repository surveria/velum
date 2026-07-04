# Roadmap

## Phase 0: Repository and Guardrails

- Safe-Rust crate with `unsafe_code = forbid`.
- CLI for smoke testing.
- CI for format, clippy, tests, docs, and unsafe-code denial.
- Initial docs for architecture, resource limits, and benchmarks.

## Phase 1: Small Interpreter

- Expand statements: blocks, `if`, loops, `break`, `continue`, and `return`.
- Add function objects and lexical scopes.
- Add objects, arrays, property lookup, and prototypes.
- Add a minimal standard library surface needed by embedding use cases.
- Start differential tests against QuickJS for every implemented feature.

## Phase 2: Compact VM

- Introduce bytecode only after the interpreter has enough language coverage to benchmark honestly.
- Keep opcodes compact and cache-friendly.
- Measure startup time, bytecode size, peak memory, and steady-state runtime.
- Keep interpreter fallback tests so bytecode generation has an oracle.

## Phase 3: Garbage Collection

- Start with a safe ownership model for values and objects.
- Evaluate reference counting plus cycle collection, mirroring the memory predictability that makes QuickJS attractive.
- Add hard heap limits and deterministic teardown reporting.

## Phase 4: ECMAScript Coverage

- Add modules, promises, async functions, generators, regular expressions, typed arrays, and BigInt based on product need.
- Track Test262 pass rate per feature instead of reporting a single misleading total.
- Keep unsupported features explicit in docs and errors.

## Phase 5: Embedding Extensions

- Profiling hooks.
- Structured event logging.
- Per-context and per-callback resource quotas.
- Interrupt hooks for watchdogs.
- Feature gates for constrained devices.
