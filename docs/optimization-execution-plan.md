# Optimization Execution Plan

This document is the working plan for growing `rs-quickjs` into a safe-Rust,
embeddable JavaScript engine while keeping implemented behavior near the
QuickJS performance and memory class.

The plan is intentionally operational. Each feature or optimization task should
update this document in the same branch that implements the task, so future
work can resume from repository state instead of relying on conversation
history.

## Targets

- Keep implemented, comparable benchmark cases within `1.10x` QuickJS latency.
- Keep implemented, comparable memory measurements within `1.10x` QuickJS
  memory use when the report has a reliable reference measurement.
- Keep the public product shape library-first: many isolated VMs in one
  process, typed host extensions, async host callbacks, explicit resource
  limits, deterministic teardown, and no mutable process-global JavaScript
  state.
- Keep the implementation safe by default. `unsafe` remains forbidden unless a
  separate design review proves that a measured bottleneck cannot be solved in
  safe Rust.
- Add project-specific engine tests, Test262 coverage, QuickJS differential
  coverage, and benchmarks for every implemented feature that affects semantics
  or hot paths.

## Status Legend

Use these status values in the task board:

- `Backlog`: the task is known but not started.
- `In progress`: exactly one task should normally have this status in a branch.
- `Done`: the task was implemented, validated, and documented in the branch.
- `Deferred`: the task was examined, but the branch intentionally leaves it for
  later with a written reason.

The `Done` column uses Markdown checkboxes. A completed task must keep a short
note about what changed, what was difficult, and what remains possible later.

## Task Board

| Done | Status | Task | Purpose | Current notes |
| --- | --- | --- | --- | --- |
| [x] | Done | Establish optimization execution plan | Create the persistent plan and task protocol. | Documentation-only task. Future branches should update this board when they start and finish work. |
| [ ] | Backlog | Object and prototype benchmark debt | Bring `object_prototype_root`, `prototype_constructor_property`, and `object_builtin` within the `1.10x` latency and memory budget where measurements are stable. | Start from the latest report and preserve semantics with engine tests and QuickJS differential cases. |
| [ ] | Backlog | `CompiledScript` AST wrapper | Add a reusable compiled representation before bytecode, so embedders can parse once and evaluate repeatedly. | Must include direct library API tests and benchmarks that separate parse cost from eval cost. |
| [ ] | Backlog | Slot-based local bindings design | Replace repeated string lookups for local variables with compiler-assigned slots. | Requires scope analysis, closure/upvalue model, and migration tests for lexical bindings. |
| [ ] | Backlog | Atom interner | Store identifiers, property keys, function names, and reusable string constants as compact atom ids. | The table should be engine-owned and share immutable atoms safely across isolated VMs. |
| [ ] | Backlog | Shape-based object layout | Move ordinary objects toward shape plus slot storage instead of per-object key maps for stable layouts. | This unlocks faster property access and lower allocation pressure. |
| [ ] | Backlog | Inline property caches | Add safe inline caches for repeated property reads and writes once shapes exist. | Cache entries must be invalidated or bypassed when the shape does not match. |
| [ ] | Backlog | Dense array fast paths | Split array storage into packed, holey, and sparse representations. | Most array-heavy benchmarks need packed or holey arrays to stay close to QuickJS. |
| [ ] | Backlog | VM-owned heap arenas | Replace scattered small allocations with Vec-backed per-VM heaps for objects, functions, shapes, atoms, and strings. | This must stay safe Rust and expose accounting for resource limits and teardown reports. |
| [ ] | Backlog | Garbage collection and memory model | Define mark/sweep or reference-counting plus cycle collection over indexed VM heaps. | Must preserve deterministic teardown, hard heap limits, and VM isolation. |

## Strategic Order

The order below is the preferred direction, but each task still starts by
checking the latest test report and benchmark exceptions.

1. Stabilize current hot paths.
   Object/prototype and array benchmark exceptions should be reduced before
   widening language coverage too aggressively. This keeps performance debt
   visible and prevents future features from depending on slow layouts.

2. Introduce `CompiledScript` before bytecode.
   The first reusable compilation layer can wrap the current AST. It should
   prove the public API shape, separate parse cost from execution cost, and give
   embedders a stable contract before the evaluator is replaced.

3. Add atom ids.
   Atoms reduce repeated string allocation, cloning, and comparison. They are a
   prerequisite for efficient slots, shapes, and property caches.

4. Add slot-based locals.
   Variable access should become an indexed lookup into local, global, and
   upvalue arrays. This requires compile-time scope analysis, but it can still
   execute through the current interpreter before bytecode exists.

5. Add shape-based objects.
   Objects with the same property layout should share a shape, while object
   instances store values in compact slots. Dictionary storage remains the
   fallback for unusual or heavily mutated objects.

6. Add dense array storage.
   Arrays should use packed storage first, then holey storage, then sparse
   dictionary fallback. Array methods and indexed property access should use the
   fastest representation they can prove.

7. Add bytecode.
   Bytecode should arrive after enough syntax, scopes, objects, arrays, and
   benchmarks exist to measure it honestly. It should preserve the existing
   `CompiledScript` API.

8. Add inline caches.
   Inline caches are most useful after shapes and bytecode exist. The first
   version can be interpreter-owned cache entries for property access sites,
   without JIT or unsafe code.

9. Add explicit VM heap accounting and GC.
   The indexed heap model should grow into deterministic accounting and a safe
   collection strategy. The GC design must be compatible with host callbacks,
   promises, queued jobs, and many isolated VMs.

## Task Execution Protocol

Every implementation task follows this order:

1. Refresh repository context.
   Read `AGENTS.md`, `README.md`, `docs/architecture.md`,
   `docs/roadmap.md`, `docs/benchmarking.md`, and this document. Inspect the
   latest test report before choosing work.

2. Select one task.
   Pick one row from the task board, create a fresh worktree and branch from
   `origin/main`, and mark that row `In progress` in the task branch. Leave
   unrelated rows unchanged.

3. Capture the baseline.
   Run the narrow tests or benchmarks that prove the current problem. For
   performance work, record the current QuickJS comparison and memory result
   before optimizing.

4. Implement the smallest coherent step.
   Keep changes scoped to the selected task. Maintain safe Rust rules, explicit
   resource limits, VM isolation, and future compatibility with the embedding
   API.

5. Add coverage.
   Add project-specific engine tests for semantics, Test262 coverage when
   relevant, QuickJS differential coverage when the reference behavior exists,
   and benchmark cases for hot paths.

6. Validate.
   Run formatting, clippy, targeted tests, and `scripts/test-all.sh` unless the
   task explicitly documents a narrower validation scope.

7. Decide on performance and memory exceptions.
   If a comparable implemented benchmark exceeds `1.10x`, either optimize it in
   the same task or record a tracked exception with the benchmark name, measured
   ratio, suspected cause, and follow-up task. If an optimization was made,
   record the latency or memory effect in the task notes.

8. Finish the task board row.
   Before the PR is ready, change the row to `Done` or `Deferred`. Add a concise
   note about what changed, problems found, validation performed, and possible
   future work.

9. Open the PR.
   The PR description must explain what changed, why it changed, validation
   results, benchmark or memory results, known exceptions, and future work.

10. Merge and clean up.
    After green CI, squash-merge the PR, update the main checkout, remove the
    task worktree, and keep the branch.

## Design Notes

### `CompiledScript` And Bytecode

The public API should not expose whether a compiled script is backed by an AST
or bytecode. The first implementation should make repeated evaluation cheaper
by reusing lexing and parsing output. Later bytecode can improve instruction
dispatch, resource accounting, and cache locality behind the same API.

### Slot-Based Locals

String-keyed binding maps are simple but expensive. A compiler pass should
assign local, global, and upvalue slots before execution. The runtime should
then read and write `Vec<Value>` entries through checked newtype indices. This
also gives the engine a natural place to account for stack and closure memory.

### Atom Interner

Identifiers and property keys should use `AtomId` values instead of repeated
owned strings on hot paths. The atom table belongs to the engine or VM boundary
and must not create mutable global state. Atoms should be usable by parser,
compiled scripts, object shapes, function metadata, and diagnostics.

### Shapes And Inline Caches

Shapes describe object layouts. Object instances store values in slots, and
shape transitions describe property additions or layout changes. Once shapes
exist, property access sites can cache `(ShapeId, offset)` and fall back to the
generic lookup path when the shape does not match.

### Dense Arrays

Array storage should distinguish packed arrays, holey arrays, and sparse
objects. Packed storage is the default fast path. Holey storage preserves JavaScript
holes without forcing every array into dictionary mode. Sparse storage remains
the fallback for large or unusual indices.

### VM-Owned Heaps

The engine should prefer indexed, Vec-backed heaps over many small shared
allocations. Handles such as `ObjectId`, `FunctionId`, `ShapeId`, `AtomId`, and
future `StringId` values make ownership explicit, keep VM teardown simple, and
support resource accounting without a custom unsafe allocator.

### Garbage Collection

The current indexed handle direction is compatible with a safe collector. A GC
design should start from explicit VM roots, stack slots, globals, closures,
objects, arrays, promises, and host callback handles. It must report memory
usage and reclaim all VM-owned state during teardown.
