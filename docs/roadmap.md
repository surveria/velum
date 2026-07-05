# Product Roadmap

The operational task board and branch protocol live in
[Project Roadmap And Execution Plan](project-plan.md). Update that document in
the same branch that starts or completes a compatibility, embedding, testing,
runtime-architecture, resource-control, observability, performance, or memory
task.

This roadmap is a short product-level view. It is not an optimization plan and
should not be read as a queue of runtime micro-optimizations. The engine must
become a safe, embeddable Rust library first, then grow compatibility,
built-ins, modules, async integration, resource controls, and observability.
Runtime architecture, performance, and memory work support those product goals
while keeping QuickJS-like size and speed as acceptance criteria.

## Current Product Queue

The default order for new work is:

1. keep CI, reports, Test262, QuickJS differential checks, and benchmarks
   reliable;
2. keep the Rust library API useful for many isolated VMs, typed host
   extensions, resource failures, teardown, and reusable compiled scripts;
3. expand compatibility through narrow Test262-visible parser, runtime,
   object, function, error, and built-in clusters;
4. add practical built-ins by report evidence and embedding needs, starting
   with JSON before broader object descriptors, arrays, functions, errors,
   Date, RegExp, Map, and Set;
5. improve diagnostics, modules, jobs, promises, async host callbacks, resource
   controls, and observability;
6. pull runtime data model work forward only when it supports the product path
   above or addresses measured performance and memory debt.

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

## Phase 6: Diagnostics And Error Model

- Make syntax, runtime, host callback, and resource-limit errors precise.
- Preserve contextual error chains for embedders.
- Report enough source, feature, and resource context for production logs.
- Keep diagnostics stable enough that applications can act on them.

## Phase 7: Modules, Jobs, And Async

- Design module loading around embedder-owned I/O and policy.
- Add promises and the JavaScript job queue.
- Add async functions when the job model can support them.
- Add async Rust host callbacks through explicit job-draining APIs.
- Keep the embedding application in control of the outer executor.

## Phase 8: Resource Control

- Expand hard limits from source and runtime counters toward heap, stack, atom,
  job, module, host callback, and wall-clock controls.
- Make every limit visible through the library API.
- Preserve deterministic teardown reporting.

## Phase 9: Production Observability

- Add structured execution events.
- Add profiling hooks and resource snapshots.
- Add per-context and per-callback quotas.
- Add interrupt hooks for watchdogs.
- Add feature gates for constrained devices.

## Phase 10: Runtime Data Model

- Add atom ids for identifiers, property keys, function names, and reusable
  string constants.
- Replace repeated string-keyed local lookups with compiler-assigned slots.
- Move ordinary objects toward shape plus slot storage.
- Split array storage into packed, holey, and sparse representations.
- Prefer VM-owned indexed heaps over scattered small allocations.

## Phase 11: Bytecode And Dispatch

- Add bytecode after enough language coverage exists to benchmark honestly.
- Keep opcodes compact and cache-friendly.
- Preserve interpreter fallback tests so bytecode generation has an oracle.
- Add inline property caches after shapes exist.

## Phase 12: Heap Management And Collection

- Grow indexed VM ownership into deterministic heap accounting.
- Evaluate a safe collector design over explicit VM roots.
- Keep collection compatible with host callbacks, promises, queued jobs, hard
  limits, and many isolated VMs.

## Continuous: Performance And Memory Guardrails

- Keep implemented comparable benchmarks within the current project budget.
- Add benchmarks when a feature creates a hot path.
- Treat performance and memory regressions as measured checkpoint tasks.
- Record exceptions in reports and the project plan instead of hiding them.
