# Roadmap

The operational task board and execution sequence live in
[Development Plan](development-plan.md). Update that document in the same
branch that starts or completes a compatibility, embedding, testing,
performance, memory, resource-control, or observability task.

## Phase 0: Repository and Guardrails

- Safe-Rust crate with `unsafe_code = forbid`.
- CLI for smoke testing.
- CI for format, clippy, tests, docs, and unsafe-code denial.
- Initial docs for architecture, resource limits, and benchmarks.
- Project rules that treat the library API, isolated VMs, host extensions, and direct API benchmarks as first-class requirements.

## Phase 1: Small Interpreter

- Expand statements: blocks, `if`, loops, `break`, `continue`, and `return`.
- Add function objects and lexical scopes.
- Add objects, arrays, property lookup, and prototypes.
- Add a minimal standard library surface needed by embedding use cases.
- Avoid internal shortcuts that would make isolated VM instances, host bindings, or future bytecode compilation harder.
- Start differential tests against QuickJS for every implemented feature.
- Add project-specific engine tests for every implemented behavior, including resource-limit and embedding edge cases that Test262 does not cover.
- Add benchmark cases for implemented language and runtime features, with QuickJS comparison wherever the feature exists in QuickJS.

## Phase 2: Embedding API

- Define the public library API around isolated virtual machines rather than the CLI runner.
- Support many independent VM instances in one Rust process without shared mutable JavaScript state.
- Add direct API tests for parallel VM creation, isolation, resource-limit failures, teardown reporting, and output separation.
- Add a host function registration API for synchronous Rust callbacks.
- Design the async host callback contract around VM-owned jobs and embedder-owned executors.
- Add direct API benchmarks for VM creation, script compilation or evaluation, host callback dispatch, job draining, and teardown.
- Add explicit teardown reports, resource usage snapshots, and structured execution events.
- Keep the API compatible with a future bytecode backend by introducing a `CompiledScript` abstraction before bytecode is required for performance.

## Phase 3: Compact VM

- Introduce bytecode only after the interpreter has enough language coverage to benchmark honestly.
- Keep opcodes compact and cache-friendly.
- Measure startup time, bytecode size, peak memory, and steady-state runtime.
- Keep interpreter fallback tests so bytecode generation has an oracle.

## Phase 4: Garbage Collection

- Start with a safe ownership model for values and objects.
- Evaluate reference counting plus cycle collection, mirroring the memory predictability that makes QuickJS attractive.
- Add hard heap limits and deterministic teardown reporting.

## Phase 5: ECMAScript Coverage

- Add promises and the job queue early enough to support async host functions.
- Add modules, async functions, generators, regular expressions, typed arrays, and BigInt based on product need.
- Track Test262 pass rate per feature instead of reporting a single misleading total.
- Keep unsupported features explicit in docs and errors.

## Phase 6: Observability Extensions

- Profiling hooks.
- Structured event logging.
- Per-context and per-callback resource quotas.
- Interrupt hooks for watchdogs.
- Feature gates for constrained devices.
