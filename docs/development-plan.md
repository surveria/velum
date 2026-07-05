# Project Development Plan

This document is the working plan for growing `rs-quickjs` into a safe-Rust,
embeddable JavaScript engine. It describes what we are building, the rough
order of work, and the protocol every branch should follow.

The plan is intentionally operational. Each feature, compatibility,
embedding, memory, testing, or optimization task should update this document in
the same branch that implements the task. Future work should resume from
repository state instead of relying on conversation history.

This is not an optimization-only backlog. The primary sequence is product
delivery: library surface, VM isolation, host extension APIs, language
compatibility, built-ins, async integration, resource control, and
observability. Performance and memory budgets are acceptance criteria for that
work, with dedicated checkpoint tasks when measurements show debt.

## Product Direction

`rs-quickjs` is a library-first JavaScript engine for Rust applications that
need many isolated virtual machines in one process. The CLI, test runner, and
scripts are support surfaces for validation and benchmarking.

The engine should keep the parts that make QuickJS attractive for embedded
Linux devices:

- small startup footprint
- predictable memory use
- fast interpreter performance without a JIT dependency
- explicit resource limits
- deterministic teardown
- behavior checked against QuickJS and Test262

It should also provide Rust-specific control that native QuickJS bindings
cannot provide as cleanly:

- safe Rust implementation by default
- isolated VMs with no mutable process-global JavaScript state
- typed host extensions
- asynchronous host callbacks through embedder-owned executors
- structured execution events, profiling hooks, and resource accounting
- memory and CPU limits that are visible at the library API boundary

## Targets

- Make the Rust library API the primary product surface before treating the CLI
  as more than a validation tool.
- Support many independent VM instances in one Rust process without mutable
  process-global JavaScript state.
- Provide typed host extensions and an async-host-callback path that can be
  driven by an embedder-owned executor.
- Keep implemented, comparable benchmark cases within `1.10x` QuickJS latency.
- Keep implemented, comparable memory measurements within `1.10x` QuickJS
  memory use when the report has a reliable reference measurement.
- Add project-specific engine tests for behavior that matters to the engine,
  even when Test262 does not cover it.
- Add Test262 coverage for ECMAScript semantics that are implemented locally.
- Add QuickJS differential coverage when QuickJS provides comparable behavior.
- Add benchmark coverage for hot paths and embedding-facing features.
- Keep the public API stable enough that the current AST evaluator can evolve
  into `CompiledScript` and bytecode without forcing ordinary embedder code to
  change.
- Keep the implementation safe by default. `unsafe` remains forbidden unless a
  separate design review proves that a measured bottleneck cannot be solved in
  safe Rust.

## Workstreams

### Compatibility

Compatibility work grows the supported JavaScript language and built-in
surface. It should be driven by focused engine fixtures, QuickJS differential
cases, and Test262 progress reports.

Near-term compatibility should prioritize features that unlock many Test262
cases without forcing premature architectural commitments: parser and lexer
gaps, object and array semantics, basic built-ins, function semantics, and
standard error behavior.

### Embedding API

Embedding work defines the product surface. The public model should evolve
around `Engine`, `Vm`, `Context`, `CompiledScript`, and host function
registration.

Every embedding-facing feature must describe VM isolation, resource ownership,
teardown behavior, host callback behavior, queued jobs, and error propagation.
CLI-only proof is not enough for these features.

### Testing And Reporting

Testing work keeps one command as the default validation path while preserving
detailed reports:

- project-specific engine fixtures under `tests/`
- active Test262 subset for CI gates
- full Test262 progress reports
- QuickJS differential corpus
- benchmark corpus with rs-quickjs and QuickJS columns
- tracked Markdown reports under `reports/test-runs/`

Reports should stay compact: summaries and failure classifications are more
important than listing every passing or intentionally skipped case.

### Performance And Memory

Performance work keeps implemented behavior close to QuickJS instead of
letting compatibility progress accumulate hidden debt. It is a continuous
constraint on feature work, not the only purpose of the roadmap.

The major implementation directions are:

- `CompiledScript` before bytecode
- atom ids for identifiers and property keys
- slot-based locals and upvalues
- shape-based object layouts
- inline property caches after shapes exist
- dense array fast paths
- VM-owned indexed heaps instead of scattered small allocations
- explicit heap accounting and a safe collection strategy

### Resource Control

Resource control work turns limits into a first-class API. Current limits cover
source length, statement count, expression depth, runtime steps, strings, and
global bindings. Future limits should cover heap budgets, atom table budgets,
stack budgets, queued jobs, module loading, host callback quotas, and wall-clock
cancellation hooks.

### Observability

Observability work adds profiling, structured runtime events, resource usage
snapshots, teardown reports, and feature gates for constrained devices. This is
not an afterthought: the engine exists partly so embedders can inspect and
control JavaScript execution.

## Delivery Milestones

These milestones describe the intended product sequence. The task board below
is the operational view; each branch should move one board item forward while
keeping these milestones in mind.

### 1. Trustworthy Project Base

Keep the repository, CI, test runner, Test262 mirror, QuickJS reference setup,
benchmark harness, and tracked reports reliable. This milestone is never fully
finished: every later branch must preserve one-command validation and compact
progress reporting.

### 2. Library-First Execution Shell

Make the Rust library the primary interface. The engine needs explicit
`Engine`, isolated `Vm`, execution `Context`, configuration, resource usage,
and teardown reporting before the CLI can be treated as anything more than a
smoke-test surface.

### 3. Host Extension Surface

Add typed Rust host functions, contextual callback errors, VM-local callback
storage, output separation, and eventually async host callbacks. This must
remain compatible with many VMs in one process and embedder-owned executors.

### 4. Core Language Compatibility

Grow syntax, statements, expressions, lexical scopes, functions, objects,
arrays, prototypes, and error semantics in narrow Test262-driven clusters.
Each cluster should have engine tests and QuickJS differential coverage where
the reference behavior exists.

### 5. Practical Built-Ins

Expand `Object`, `Array`, `String`, `Number`, `Math`, `Boolean`, errors,
functions, and other high-value built-ins based on Test262 failure clusters and
embedding use cases. Built-ins should not hide slow storage decisions; add
benchmarks for hot methods as they land.

### 6. Reusable Compilation API

Introduce `CompiledScript` while it can still wrap the current AST. This gives
embedders a parse-once/evaluate-many contract and creates a stable boundary for
later bytecode work.

### 7. Runtime Data Model

Add atoms, slot-based locals, shape-based objects, dense array storage, indexed
VM-owned heaps, and explicit resource accounting. These changes support both
compatibility and performance, but they should be introduced as product
architecture work rather than isolated micro-optimizations.

### 8. Async JavaScript And Jobs

Add promises, the JavaScript job queue, async functions, and async Rust host
callbacks. The VM should own JavaScript jobs, while the embedding application
owns the outer executor and job-draining policy.

### 9. Bytecode And Dispatch

Add bytecode after enough language coverage exists to benchmark honestly.
Bytecode, inline caches, and compact dispatch should stay behind the
`CompiledScript` API so ordinary embedder code does not need to change.

### 10. Heap Management And Hard Limits

Grow the indexed ownership model into deterministic heap accounting and a safe
collection strategy. Hard limits, teardown, queued jobs, host callbacks, and
many isolated VMs must remain part of the design.

### 11. Production Observability

Add structured execution events, profiling hooks, resource snapshots, teardown
reports, cancellation hooks, and feature gates for constrained devices. This is
part of the product surface, not only debug tooling.

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

| Done | Status | Task | Workstream | Purpose | Current notes |
| --- | --- | --- | --- | --- | --- |
| [x] | Done | Establish persistent development plan | Planning | Create the general project plan, task board, and task protocol. | Replaces the optimization-only plan with a broader development plan. Future branches should update this board when they start and finish work. |
| [x] | Done | Generalize project roadmap scope | Planning | Rebalance the plan around product development order instead of an optimization-first backlog. | Clarifies that performance and memory are recurring guardrails, while embedding API, compatibility, host extensions, resource control, and observability are first-class roadmap tracks. |
| [x] | Done | Embedding API skeleton | Embedding API | Introduce the public direction for `Engine`, isolated `Vm`, execution `Context`, and embedder-owned configuration. | Adds `Engine`, `EngineConfig`, `Vm`, `VmConfig`, VM resource usage, teardown reports, README coverage, and direct library tests for isolated VMs, output separation, VM-specific limits, and teardown. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T144858Z.md` keeps existing benchmark exceptions tracked. |
| [x] | Done | Multi-VM isolation fixtures | Embedding API / testing | Prove that many VMs can run in one Rust process with isolated globals, output, limits, errors, and teardown. | Adds a direct library fixture that runs eight VMs in one process, verifies isolated globals and output buffers, forces a separate VM resource-limit failure, then confirms the surviving VMs continue and produce teardown reports. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T145428Z.md` keeps benchmark exceptions tracked. |
| [x] | Done | Host function API skeleton | Embedding API | Add the first typed Rust host function registration path. | Adds synchronous `Context::register_host_function`, `HostCall` checked argument accessors, contextual callback errors, VM-local callback storage, and conservative rejection of VM-owned handle return values. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T150355Z.md` keeps benchmark exceptions tracked. Async support remains for the promise/job queue task. |
| [x] | Done | Test262 feature map | Compatibility / testing | Convert full Test262 results into a feature-oriented progress map. | Adds compact full-corpus feature-area tables with pass/fail/skip counts, pass rate, active-manifest counts, top skip reasons, and an aggregated `other feature areas` row. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T151636Z.md` records 102578 executed Test262 variants, 9098 passed, 93480 failed, and keeps failed case details capped at the last 30. |
| [x] | Done | Parser and lexer Test262 cluster | Compatibility | Reduce top parser and lexer failure categories in full Test262 reports. | Adds the `numeric-literal-syntax` cluster: binary, octal, hexadecimal, decimal exponent, leading-decimal, and numeric separator support without BigInt semantics. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T152806Z.md` raises full Test262 passes from 9098 to 9378, raises `language/literals` passes from 514 to 784, lowers parser failures from 25349 to 23170, and lowers the top `lexer: unexpected character` hint from 17549 to 16369. BigInt literals remain unsupported and are now reported explicitly. |
| [x] | Done | Runtime semantics cluster | Compatibility | Expand coherent statement, expression, scope, function, and error semantics. | Adds omitted catch binding support (`try { ... } catch { ... }`) without creating a catch parameter binding. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T153644Z.md` raises full Test262 passes from 9378 to 9388, lowers parser failures from 23170 to 23160, and adds one active Test262 and one QuickJS differential case. |
| [x] | Done | Clarify general development sequence | Planning | Make the working plan clearly describe the whole product roadmap instead of looking like an optimization plan. | Adds product delivery milestones, aligns the short roadmap with the operational plan, and frames performance work as acceptance criteria plus checkpoint tasks. This was a documentation-only change validated with `git diff --check`; full CI remains the merge gate. |
| [x] | Done | Basic built-ins expansion | Compatibility | Expand high-value `Object`, `Array`, `String`, `Number`, and `Math` behavior. | Starts the built-ins track with `Number` as a function/constructor, `Number.prototype.constructor`, basic string/boolean/null/undefined conversions, and static constants such as `NaN`, infinities, safe integer bounds, `MAX_VALUE`, `MIN_VALUE`, and `EPSILON`. Adds engine, active Test262, QuickJS differential, and benchmark fixtures. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T155826Z.md` raises full Test262 passes from 9388 to 9609, raises `built-ins/Number` passes from 8 to 32, and removes `missing binding: Number` from the top missing bindings. The new `number_builtin` benchmark is tracked as a latency exception at `1.29x`; `Number.prototype` primitive-wrapper internals remain future work. |
| [x] | Done | `CompiledScript` AST wrapper | Embedding API / performance | Add a reusable compiled representation before bytecode, so embedders can parse once and evaluate repeatedly. | Adds AST-backed `CompiledScript` and `CompiledScriptUsage`, plus `compile`/`eval_compiled` on `Context`, `Vm`, and compatibility `Runtime`. The parser now reports compile usage so target VMs reject compiled scripts that exceed their stricter source, statement, or expression-depth limits. Direct library tests cover repeated eval in one VM, reuse across isolated VMs, compile-time parse errors, and stricter target VM limits. Benchmark reports now separate cold eval, compile-only, and compiled-eval columns and add `compiled_script_reuse`. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T161440Z.md` keeps full Test262 at 9609 passed, measures 51 benchmarks, and records `compiled_script_reuse` as a rounded `1.10x` latency exception while showing `3 us` compile-only, `54 us` compiled eval, and `67 us` cold eval. Future bytecode, atoms, and slot locals can replace the backing representation behind this API. |
| [ ] | Backlog | Atom interner | Performance and memory | Store identifiers, property keys, function names, and reusable string constants as compact atom ids. | The table should be engine-owned or VM-owned without mutable process-global state. |
| [ ] | Backlog | Slot-based local bindings | Performance and memory | Replace repeated string lookups for local variables with compiler-assigned local, global, and upvalue slots. | Requires scope analysis, closure/upvalue model, and migration tests for lexical bindings. |
| [ ] | Backlog | Object and prototype performance checkpoint | Performance and memory | Bring `object_prototype_root`, `prototype_constructor_property`, and `object_builtin` within the `1.10x` latency and memory budget where measurements are stable. | This is a checkpoint task driven by reports, not the whole project direction. Preserve semantics with engine tests and QuickJS differential cases. |
| [ ] | Backlog | Shape-based object layout | Performance and memory | Move ordinary objects toward shape plus slot storage instead of per-object key maps for stable layouts. | This unlocks faster property access and lower allocation pressure. |
| [ ] | Backlog | Dense array fast paths | Performance and memory | Split array storage into packed, holey, and sparse representations. | Most array-heavy benchmarks need packed or holey arrays to stay close to QuickJS. |
| [ ] | Backlog | Promise job queue and async host callbacks | Embedding API / compatibility | Add the job model needed by promises and async Rust host functions. | The embedding application must own the outer executor; the VM owns queued JavaScript jobs. |
| [ ] | Backlog | Bytecode VM | Performance and memory | Replace direct AST evaluation on hot paths with compact bytecode behind the `CompiledScript` API. | Start only after enough language coverage exists to benchmark honestly. |
| [ ] | Backlog | VM-owned heap accounting and GC | Resource control | Define mark/sweep or reference-counting plus cycle collection over indexed VM heaps. | Must preserve deterministic teardown, hard heap limits, and VM isolation. |
| [ ] | Backlog | Observability hooks | Observability | Add structured events, profiling hooks, resource usage snapshots, and teardown reports. | Useful for production embedding, debugging, and future performance work. |

## Default Execution Order

The order below is the default project direction. It can change when the
latest report shows a clearer bottleneck, compatibility gap, or embedding API
need, but branches should document why they changed priority.

1. Keep the repository and reports trustworthy.
   Guardrails, CI, test reports, QuickJS setup, Test262 setup, and benchmark
   reporting are part of the product. Do not let them drift.

2. Strengthen the embedding API.
   Add direct library tests for many VMs in one process, isolation, resource
   failures, teardown, and host output. Keep CLI behavior as smoke coverage, not
   the only proof.

3. Add the first host extension path.
   Typed synchronous host functions should land before async callbacks. This
   proves argument conversion, contextual host errors, output separation, and
   VM-boundary rules.

4. Expand core compatibility in narrow clusters.
   Use Test262 failure classifications to pick parser, runtime, and built-in
   clusters. Each branch should close a coherent area rather than mixing
   unrelated syntax and runtime changes.

5. Expand practical built-ins.
   Use top missing bindings, feature-area reports, and embedding use cases to
   prioritize `Object`, `Array`, `String`, `Number`, `Math`, `Function`, and
   standard errors. Add benchmarks for hot methods as they become implemented.

6. Introduce `CompiledScript` before bytecode.
   The first reusable compilation layer can wrap the current AST. It should
   prove the public API shape, separate parse cost from execution cost, and give
   embedders a stable contract before the evaluator is replaced.

7. Keep performance checkpoints close behind feature work.
   Benchmark exceptions should be handled as recurring checkpoint tasks, not as
   the only roadmap. When a feature makes a hot path slower, either fix it in
   the same branch or record a measured exception with a follow-up task.

8. Add atoms and slot-based locals.
   Atoms reduce repeated string allocation, cloning, and comparison. Slot-based
   locals turn variable access into checked index operations and prepare the
   runtime for bytecode.

9. Add shape-based objects and dense arrays.
   Object and array layout work should happen before long-tail built-in
   expansion makes slow storage harder to replace.

10. Add promises, job queue, and async host callbacks.
   Promise semantics and async host integration are product-critical, but they
   need the embedding model and job ownership to be clear first.

11. Add bytecode and inline caches.
   Bytecode should preserve the `CompiledScript` API. Inline caches become most
   valuable after shapes and bytecode exist.

12. Add explicit VM heap accounting and GC.
    The indexed heap model should grow into deterministic accounting and a safe
    collection strategy compatible with host callbacks, promises, queued jobs,
    and many isolated VMs.

13. Add observability and production controls.
    Profiling hooks, structured events, resource snapshots, and feature gates
    should become part of the embeddable engine surface.

## Branch Execution Protocol

Every implementation task follows this order:

1. Refresh repository context.
   Read `AGENTS.md`, `README.md`, `docs/architecture.md`,
   `docs/roadmap.md`, `docs/benchmarking.md`, and this document. Inspect the
   latest test report before choosing work.

2. Select one task.
   Pick one row from the task board, create a fresh worktree and branch from
   `origin/main`, and mark that row `In progress` in the task branch. Leave
   unrelated rows unchanged.

3. Define the outcome.
   State whether the branch is primarily compatibility, embedding API,
   testing, performance, memory, resource control, or observability work. Record
   the expected evidence before implementing.

4. Capture the baseline.
   Run the narrow tests or benchmarks that prove the current problem. For
   compatibility work, record the relevant Test262 area. For performance or
   memory work, record the current QuickJS comparison before optimizing.

5. Implement the smallest coherent step.
   Keep changes scoped to the selected task. Maintain safe Rust rules, explicit
   resource limits, VM isolation, and future compatibility with the embedding
   API.

6. Add coverage.
   Add project-specific engine tests for semantics, Test262 coverage when
   relevant, QuickJS differential coverage when the reference behavior exists,
   direct library API tests for embedding-facing behavior, and benchmark cases
   for hot paths.

7. Validate.
   Run formatting, clippy, targeted tests, and `scripts/test-all.sh` unless the
   task explicitly documents a narrower validation scope.

8. Decide on performance and memory exceptions.
   If a comparable implemented benchmark exceeds `1.10x`, either optimize it in
   the same task or record a tracked exception with the benchmark name, measured
   ratio, suspected cause, and follow-up task. If an optimization was made,
   record the latency or memory effect in the task notes.

9. Finish the task board row.
   Before the PR is ready, change the row to `Done` or `Deferred`. Add a concise
   note about what changed, problems found, validation performed, and possible
   future work.

10. Open the PR.
    The PR description must explain what changed, why it changed, validation
    results, benchmark or memory results, known exceptions, and future work.

11. Merge and clean up.
    After green CI, squash-merge the PR, update the main checkout, remove the
    task worktree, and keep the branch.

## Architecture Notes

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
objects. Packed storage is the default fast path. Holey storage preserves
JavaScript holes without forcing every array into dictionary mode. Sparse
storage remains the fallback for large or unusual indices.

### Promise Jobs And Async Host Functions

The engine must not assume a process-wide async runtime. Async integration
belongs at the embedding boundary. A VM should own JavaScript jobs, while the
embedding application owns the executor that drives Rust futures. Once promises
exist, async host callbacks should return JavaScript promises and complete them
through explicit job-draining APIs.

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
