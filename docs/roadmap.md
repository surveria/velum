# Roadmap

The operational task board and branch protocol live in
[Development Plan](development-plan.md). Update that document in the same
branch that starts or completes a compatibility, embedding, testing,
performance, memory, resource-control, or observability task.

This roadmap is a short product-level view. It is intentionally broader than a
performance plan: the engine must become a safe, embeddable Rust library first,
then grow compatibility, runtime architecture, resource controls, and
observability while keeping QuickJS-like size and speed as acceptance criteria.

## Phase 0: Repository And Guardrails

- Safe-Rust crate with `unsafe_code = deny`.
- CLI for smoke testing, differential checks, and benchmark orchestration.
- CI for format, clippy, tests, docs, and unsafe-code denial.
- One-command local validation through `scripts/test-all.sh`.
- Tracked reports for engine tests, Test262 progress, QuickJS differential
  checks, and benchmark comparisons.

## Phase 1: Library-First Execution Shell

- Define the public library API around isolated virtual machines rather than
  the CLI runner.
- Support many independent VM instances in one Rust process without shared
  mutable JavaScript state.
- Add direct API tests for VM creation, isolation, resource-limit failures,
  teardown reporting, and output separation.
- Keep the public API compatible with future `CompiledScript` and bytecode
  backends.

## Phase 2: Host Extension API

- Add typed synchronous Rust host functions.
- Preserve contextual `Result` errors across the host/JavaScript boundary.
- Keep host callbacks VM-local and resource-accounted.
- Design async host callbacks around VM-owned JavaScript jobs and
  embedder-owned executors.

## Phase 3: Core Interpreter Compatibility

- Expand lexer and parser coverage in narrow Test262-driven clusters.
- Expand statements, expressions, lexical scopes, functions, objects, arrays,
  prototypes, and error semantics.
- Add project-specific engine tests for every implemented behavior.
- Add QuickJS differential tests where the reference behavior is available.

## Phase 4: Practical Built-Ins

- Expand high-value built-ins such as `Object`, `Array`, `String`, `Number`,
  `Math`, `Boolean`, `Function`, and standard errors.
- Prioritize built-ins with evidence from Test262 failure maps and embedding
  use cases.
- Add benchmarks for hot built-in paths as they become implemented.

## Phase 5: Reusable Compilation API

- Introduce `CompiledScript` before bytecode.
- Reuse lexing and parsing work for repeated evaluation.
- Separate parse, compile, execute, host-callback, and teardown measurements.
- Keep the API stable enough for a later bytecode backend.

## Phase 6: Runtime Data Model

- Add atom ids for identifiers, property keys, function names, and reusable
  string constants.
- Replace repeated string-keyed local lookups with compiler-assigned slots.
- Move ordinary objects toward shape plus slot storage.
- Split array storage into packed, holey, and sparse representations.
- Prefer VM-owned indexed heaps over scattered small allocations.

## Phase 7: Async JavaScript And Jobs

- Add promises and the JavaScript job queue.
- Add async functions when the job model can support them.
- Add async Rust host callbacks through explicit job-draining APIs.
- Keep the embedding application in control of the outer executor.

## Phase 8: Bytecode And Dispatch

- Add bytecode after enough language coverage exists to benchmark honestly.
- Keep opcodes compact and cache-friendly.
- Preserve interpreter fallback tests so bytecode generation has an oracle.
- Add inline property caches after shapes exist.

## Phase 9: Heap Management And Hard Limits

- Grow indexed VM ownership into deterministic heap accounting.
- Evaluate a safe collector design over explicit VM roots.
- Add hard heap, stack, atom, job, host callback, and wall-clock controls.
- Preserve deterministic teardown reporting.

## Phase 10: Production Observability

- Add structured execution events.
- Add profiling hooks and resource snapshots.
- Add per-context and per-callback quotas.
- Add interrupt hooks for watchdogs.
- Add feature gates for constrained devices.
