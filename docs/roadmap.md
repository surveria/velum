# Product Roadmap

The operational task board and branch protocol live in
[Project Development Plan](project-plan.md). Update that document in the same
branch that starts or completes a compatibility, embedding, testing,
runtime-architecture, resource-control, observability, performance, or memory
task.

Architecture prerequisites and the methodology for balancing compatibility,
runtime foundations, and optimization live in [Architecture Stabilization And
Development Strategy](architecture-stabilization-plan.md). Product priority
comes from this roadmap and the project plan; architecture gates for affected
work come from the stabilization plan.

This roadmap is a short product-level view. It is not an optimization plan and
should not be read as a queue of runtime micro-optimizations. The engine must
become a safe, embeddable Rust library first, then grow compatibility,
built-ins, modules, async integration, resource controls, and observability.
Runtime architecture, performance, and memory work support those product goals
while keeping QuickJS-like size and speed as acceptance criteria.

Read runtime ideas from benchmark reviews as methods, not as the product
queue. A branch should normally start from a compatibility, embedding,
built-in, diagnostics, async, resource-control, observability, memory, or
measured-performance need, then choose atoms, slots, shapes, dense arrays,
bytecode, inline caches, or heap work only when they are the right tool.

## Current Product Queue

The default order for new work is:

1. keep CI, reports, Test262, QuickJS differential checks, and benchmarks
   reliable;
2. keep the Rust library API useful for many isolated VMs, typed host
   extensions, resource failures, teardown, and reusable compiled scripts;
3. expand compatibility through narrow Test262-visible parser, runtime,
   object, function, error, and built-in clusters;
4. add practical built-ins by report evidence and embedding needs, with object
   descriptors, arrays, functions, errors, Date, RegExp, Map, and Set following
   the first JSON tranche;
5. improve diagnostics, modules, jobs, promises, async host callbacks, resource
   controls, and observability;
6. add runtime profiling when it is needed to choose an architecture branch;
7. pull runtime data model work forward only when it supports the product path
   above or addresses measured performance and memory debt.

Runtime profiling, atomization, slots, shapes, dense arrays, bytecode, inline
caches, heap compaction, and collection are implementation methods. They are
important, but they should normally be selected because they unblock the queue
above or protect a measured latency or memory budget.

The detailed project plan also keeps a product capability backlog. Use that
backlog to choose the next branch before treating the task board as a priority
list; the board records history and known work, so recent runtime-heavy rows do
not define the whole roadmap.

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
- Use `OwnedValue` for portable primitives and generation-checked
  `RetainedValue` roots for data that survives across VM calls.
- Keep `CompiledScript` bytecode-owned and hidden behind the public API so
  bytecode operands and quickening can evolve without exposing VM internals.

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

## Phase 5: Reusable Compilation And Bytecode API

- Keep `CompiledScript` as the reusable, bytecode-owned execution artifact.
- Reuse lexing, parsing, binding analysis, and compilation work for repeated
  evaluation.
- Separate parse, compile, execute, host-callback, and teardown measurements.
- Keep the parser AST as compile-time front-end IR only; runtime execution must
  not retain AST bodies or reparse from execution layers.

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

## Phase 11: Bytecode Dispatch And Quickening

- Extend bytecode coverage as compatibility grows and benchmark evidence
  identifies hot paths.
- Keep opcodes and operands compact, cache-friendly, and bytecode-owned.
- Preserve bytecode regression tests and QuickJS differential checks as the
  execution oracle. Parser-AST execution fallback is forbidden.
- Add direct operands, inline caches, dense-array loop instructions, and native
  call specializations only behind explicit guards with ordinary bytecode
  semantic slow paths.

## Phase 12: Heap Management And Collection

- Grow indexed VM ownership into deterministic heap accounting.
- Evaluate a safe collector design over explicit VM roots.
- Keep legacy raw-result calls collector-disabled until they are replaced by
  owned, borrowed, or retained handle boundaries.
- Keep collection compatible with host callbacks, promises, queued jobs, hard
  limits, and many isolated VMs.

## Continuous: Performance And Memory Guardrails

- Keep implemented comparable benchmarks within the current project budget.
- Add benchmarks when a feature creates a hot path.
- Treat performance and memory regressions as measured checkpoint tasks.
- Record exceptions in reports and the project plan instead of hiding them.
