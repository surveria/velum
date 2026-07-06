# Project Development Plan

This document is the canonical product roadmap and execution plan for growing
`rs-quickjs` into a safe-Rust, embeddable JavaScript engine. It describes what
we are building, the rough order of work, and the protocol every branch should
follow.

The plan is intentionally operational. Each repository, embedding API,
compatibility, built-in, async, testing, resource-control, observability,
runtime-architecture, performance, or memory task should update this document
in the same branch that implements the task. Future work should resume from
repository state instead of relying on conversation history.

This is a whole-project development plan, not an optimization backlog. Read it
as the delivery order for the engine: keep the validation base reliable, keep
the Rust library API useful for embedders, expand language compatibility, add
practical built-ins, design modules and async jobs, expand resource control,
add production observability, and evolve runtime internals behind stable public
interfaces. Performance and memory budgets are acceptance criteria for all of
that work, with dedicated checkpoint tasks only when measurements show debt.

## How To Read This Plan

This document answers three different questions:

1. What are we building?
   A safe-Rust, embeddable JavaScript engine with many isolated VMs, typed host
   extensions, async integration, resource controls, observability, and
   QuickJS/Test262 validation.

2. What do we do next?
   Follow the default delivery order and the current delivery queue. Pick one
   small branch that moves product capability, compatibility, embedding support,
   reliability, resource control, observability, or measured performance
   forward, then update the task board with the evidence from that branch.

3. How do we execute the branch?
   Follow the branch protocol: refresh requirements, mark one task in progress,
   implement narrowly, validate, record compatibility and performance evidence,
   open a PR, wait for CI, squash-merge, update `main`, and remove the worktree.

Runtime architecture notes are not the project goal by themselves. Atoms,
slots, shapes, dense arrays, bytecode, inline caches, and GC are foundations
that we pull forward when they unblock compatibility, embedding API behavior,
resource accounting, observability, or measured performance and memory debt.

## Plan Scope

This document intentionally covers the whole project, not only speed work:

- product surface: library API, VM isolation, host functions, async callbacks,
  resource limits, teardown, and observability
- compatibility: parser, runtime semantics, object model, functions, errors,
  built-ins, Test262 progress, and QuickJS differential behavior
- infrastructure: CI, one-command validation, corpus management, reports, and
  benchmark reliability
- runtime foundations: atoms, slots, shapes, arrays, bytecode, inline caches,
  indexed heaps, and collection strategy when they support the product path
- performance and memory: continuous budgets plus explicit checkpoint tasks
  when a report shows measured debt

When a task is mainly an internal architecture or performance task, it must
still explain which product capability, compatibility cluster, resource-control
need, observability feature, or measured budget exception it protects.

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

## Default Delivery Order

Use this order for ordinary branch selection when the latest report does not
show a stronger reason to do something else:

1. Preserve repository and measurement trust.
   CI, one-command validation, Test262 setup, QuickJS setup, compact reports,
   and benchmark reporting must keep working before feature work expands.

2. Keep the library surface usable for embedders.
   Direct Rust API coverage should stay ahead of CLI-only behavior for isolated
   VMs, resource limits, typed host functions, output separation, teardown, host
   errors, and reusable compiled scripts.

3. Expand compatibility by visible Test262 clusters.
   Parser, runtime semantics, object semantics, functions, errors, and built-ins
   should land in narrow branches that improve measured corpus progress.

4. Add practical built-ins by usage and report evidence.
   Prioritize `Object`, `Array`, `String`, `Number`, `Math`, `Boolean`,
   `Function`, standard errors, JSON, Date, RegExp, Map, and Set by
   missing-binding counts, feature-area failures, and embedding needs.

5. Improve diagnostics before errors become API debt.
   Syntax, runtime, host callback, and resource-limit errors should become
   stable enough for embedders to log, classify, and act on.

6. Keep host extension support close to real embedding use.
   Improve value conversion, VM-local callback ownership, examples, cancellation
   points, and direct library benchmarks before adding advanced async behavior.

7. Add modules, jobs, promises, and async host callbacks.
   These features should build on the synchronous embedding surface and keep the
   embedder in control of I/O policy and the outer executor.

8. Expand resource control and observability.
   Heap, stack, atom, job, module, host callback, cancellation, event, profiling,
   and teardown data should become first-class library behavior.

9. Change runtime data structures when they unblock product work or measured
   debt.
   Atoms, slots, shapes, dense arrays, indexed heaps, bytecode, inline caches,
   and GC are architecture work in service of compatibility, embedding,
   observability, resource control, performance, and memory footprint.

10. Run performance and memory checkpoint tasks when evidence calls for them.
    Checkpoints can preempt the order only when measurements show a regression
    or budget exception that blocks the next feature tranche, embedding API
    promise, or device-footprint target.

## Current Delivery Queue

This is the rough product order for the next branches. It is intentionally not
an optimization queue. Runtime architecture work appears here only when it
protects compatibility, embedding behavior, resource accounting, observability,
or measured performance and memory budgets.

1. Keep report triage current.
   Before choosing each branch, summarize the newest Test262, QuickJS
   differential, benchmark, and memory signals. The latest report is the input
   to the next task, not a side artifact.

2. Keep compatibility moving in visible clusters.
   Prioritize syntax, functions, lexical environments, `this`, exceptions,
   equality, prototype behavior, iteration, and coercion when they unlock
   visible Test262 areas, practical built-ins, or embedding API behavior.

3. Continue object, function, and error semantics.
   The current product need is still the object/function property model:
   descriptors, own-property queries, prototype behavior, function metadata,
   standard errors, and operations needed by `Object`, `Array`, `Function`,
   errors, JSON, Date, RegExp, Map, and Set. These branches should improve
   Test262-visible behavior and add QuickJS differential coverage where
   reference behavior exists.

4. Grow arrays and practical built-ins.
   Add high-value Array methods, function callability semantics, standard error
   objects, and remaining practical built-ins in narrow clusters. Pull dense
   array storage or faster call paths forward only when correctness, resource
   accounting, or measured hot paths justify it.

5. Tighten the embedding API and documentation.
   Keep examples, crate docs, direct API tests, typed host functions,
   multi-VM isolation, resource failures, teardown, compiled-script reuse, and
   output behavior aligned with the actual public API.

6. Improve diagnostics and error classification.
   Stabilize syntax, runtime, host callback, and resource-limit errors before
   many more API surfaces depend on ad-hoc messages.

7. Design modules, jobs, promises, and async callbacks.
   The VM should own JavaScript jobs. Embedders should own I/O policy, module
   loading policy, cancellation, job draining, and the outer executor.

8. Expand resource control and observability.
   Make heap, stack, atom, job, module, host callback, wall-clock cancellation,
   structured events, profiling, and teardown data visible at the library API.

9. Profile runtime hot paths when architecture work is being selected.
   Recent benchmark reports show low compile times on many heavy rows while
   compiled evaluation remains much larger. Treat arrays, property lookup,
   prototype traversal, built-in calls, descriptor paths, and binding lookup as
   measured runtime debt, but capture profiles as evidence for a concrete
   product or compatibility branch rather than as the roadmap by itself.

10. Pull runtime data-model foundations forward when they are needed.
    Compiler-assigned slots, property-key atoms, shapes, dense arrays, indexed
    heaps, bytecode, inline caches, and GC are foundation work for the product
    queue above, not isolated speed experiments. The likely order is complete
    atomization, compiler-assigned slots and upvalues, shape-based object
    layouts, dense array storage, bytecode dispatch, inline caches, and compact
    VM-owned heaps.

11. Add bytecode and heap collection after enough semantics exist.
    Bytecode should stay behind the `CompiledScript` API. Heap accounting and a
    safe collection strategy should preserve host callbacks, queued jobs,
    deterministic teardown, hard limits, and many isolated VMs.

12. Run performance and memory checkpoints continuously.
    Checkpoint branches are allowed when reports show a budget exception, but
    they should name the affected product path and leave compatibility coverage
    intact.

## Workstreams

### Repository And Validation

Repository work keeps the project reliable enough to measure progress. CI,
`scripts/test-all.sh`, Test262, the QuickJS reference setup, benchmark
orchestration, report generation, and documentation links are product
infrastructure, not side work.

Every branch should preserve one-command validation and compact reports. A
feature that cannot be measured, compared, or reproduced should be treated as
incomplete.

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

### Modules, Jobs, And Async

Modules, promises, the JavaScript job queue, async functions, and async Rust
host callbacks are product features for embedders. The VM owns JavaScript jobs,
while embedding applications own I/O policy, module loading policy, executor
choice, cancellation, and job-draining policy.

This work should happen after the synchronous embedding surface and enough core
runtime semantics exist to make async behavior meaningful.

### Runtime Architecture

Runtime architecture work changes the internal model behind the public API.
These tasks should be introduced when they unlock compatibility, resource
control, observability, or measured performance and memory debt. They are not a
separate product direction.

Recent benchmark reviews point to runtime execution rather than parsing as the
main measured debt. Many heavy rows now spend only a few microseconds in
compile, while compiled evaluation stays in the hundreds of microseconds. The
hot areas are arrays, descriptor-heavy objects, prototype traversal, built-in
constructor/prototype calls, property lookup, `in`, `for...in`, and binding
lookup.

The major implementation directions are:

- `CompiledScript` before bytecode
- atom ids for identifiers and property keys
- slot-based locals and upvalues
- shape-based object layouts
- inline property caches after shapes exist
- dense array fast paths
- VM-owned indexed heaps instead of scattered small allocations
- explicit heap accounting and a safe collection strategy

Current architecture status:

| Direction | Status | Next step |
| --- | --- | --- |
| `CompiledScript` | Partial. The public API exists and currently stores an AST plus usage counters. | Keep the API stable; replace the backing representation with bytecode only after runtime hot paths are profiled. |
| Slot-based bindings | Partial. `BindingScope` stores cells in a `Vec` behind an atom-to-slot map, the AST separates parser-interned name text from per-occurrence `StaticBinding` operands, compiled scripts build a scope-aware binding layout with `GlobalSlot`, `LocalSlot`, and `UpvalueSlot` metadata, compiled function parameter scopes can be populated at compiled `LocalSlot` offsets, compiled lexical declarations, lexical `for...in` bindings, catch parameter scopes, and function-local hoisted `var` declarations can best-effort populate compiled `LocalSlot` offsets, and layout-backed local/upvalue cache hits can skip ordinary shadowing scans. Global cache hits remain guarded because Annex B behavior can still expose runtime shadowing cases. | Extend compiled frame construction to globals and closure cells; then route reads, writes, compound assignments, updates, and constructor lookups through checked slot operands. |
| Atom interner | Partial. VM-local atoms cover lexical/global binding names and the atom table now uses a sorted vector index instead of a tree map; static AST names are typed and deduplicated per parsed script so later property and binding operands can attach atom/slot metadata without changing parser surfaces again. Static object literal creation uses borrowed parser-known property names while storing and resolving ordinary object properties through atom-backed `PropertyKey` values. | Extend atoms to the remaining built-in names, function metadata, dynamic string literal keys, shapes, prototype lookup, and diagnostics so hot paths do not fall back to owned `String`. |
| Shapes / hidden classes | Partial foundation. Ordinary object and function properties are atom-keyed, slot-backed, vector-indexed, ordinary object named-property lookups resolve slots through VM-owned `ShapeId` metadata, shapes include data-property descriptor attributes, cacheable named-property lookup snapshots exist for object/prototype reads and `in`, and a VM-local prototype lookup version guards structural chain changes. | Add compiler-assigned property operands and use cacheable lookup snapshots from inline caches when bytecode/interpreter sites exist. |
| Inline caches | Not started. Cacheable shape/prototype lookup snapshots exist, but there is no bytecode site or per-site cache storage yet. | After shapes and compiled instruction sites exist, cache property load/store/call and `in` lookups by shape and prototype version. |
| Dense arrays | Partial foundation. Arrays now use a dedicated `ArrayStorage` with packed and holey dense elements plus sparse index keys. Guarded packed fast paths exist for scan/copy-style reads, concat, join, slice, and default-attribute reverse. | Add guarded fast paths for shift and unshift, broaden reverse only when descriptor semantics are modeled explicitly, and preserve fallback for holes, sparse indices, descriptors, and prototype index properties. |
| Built-in intrinsics | Partial. Constructors and prototypes are lazily materialized per VM. | Separate immutable intrinsic metadata from mutable JS objects, pre-resolve atoms/shapes, and add direct native-call paths for common built-ins. |
| Bytecode quickening | Not started. Compiled evaluation still executes the AST. | Add generic bytecode first, then safe quickening from generic operations to specialized number, property, and native-call instructions with fallback. |
| Memory layout | Partial indexed handles for objects, functions, native functions, and host functions. `Value::String` still owns strings directly. | Add `StringId`, compact handles, Vec-backed heaps, free lists, boxed immutable constants, and captured-variable cells only where closures need them. |
| Parallel execution model | Partial at the product/API level. Independent VMs can exist, but no parallel execution contract is documented for compiled scripts, pools, or cleanup. | Parallelize independent VMs and compilation jobs, share immutable compiled scripts, and avoid trying to parallelize one arbitrary JavaScript context. |

### Performance And Memory Guardrails

Performance and memory guardrails keep implemented behavior close to QuickJS
while the engine grows into a complete embeddable library. They are continuous
acceptance criteria for feature work, not the only purpose of the roadmap.

Performance or memory branches should be checkpoint tasks with measured
baselines and a clear target. Ordinary compatibility and embedding branches
should still add benchmarks for hot paths and record measured exceptions when a
new feature exceeds the budget.

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

### 6. Diagnostics And Error Model

Make syntax, runtime, host callback, and resource-limit errors precise and
stable enough for embedders. Good diagnostics are part of the library contract,
not just developer convenience.

### 7. Reusable Compilation API

Introduce `CompiledScript` while it can still wrap the current AST. This gives
embedders a parse-once/evaluate-many contract and creates a stable boundary for
later bytecode work.

### 8. Modules, Jobs, And Async

Design module loading around embedder-owned I/O and policy. Add the JavaScript
job queue before promises, async functions, and async Rust host callbacks. The
VM should own JavaScript jobs, while the embedding application owns the outer
executor and job-draining policy.

### 9. Resource Control

Turn limits into a complete library API: heap budgets, atom table budgets,
stack budgets, queued jobs, module loading, host callback quotas, and
wall-clock cancellation hooks. Every new limit must appear in tests and
teardown/resource reports.

### 10. Observability

Add structured execution events, profiling hooks, resource snapshots, teardown
reports, cancellation hooks, and feature gates for constrained devices. This is
part of the product surface, not only debug tooling.

### 11. Runtime Data Model

Add atoms, slot-based locals, shape-based objects, dense array storage, indexed
VM-owned heaps, and explicit resource accounting. These changes support both
compatibility and performance, but they should be introduced as product
architecture work rather than isolated micro-optimizations.

### 12. Bytecode And Dispatch

Add bytecode after enough language coverage exists to benchmark honestly.
Bytecode, inline caches, and compact dispatch should stay behind the
`CompiledScript` API so ordinary embedder code does not need to change.

### 13. Heap Management And Collection

Grow the indexed ownership model into deterministic heap accounting and a safe
collection strategy. Hard limits, teardown, queued jobs, host callbacks, and
many isolated VMs must remain part of the design.

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

The board is both history and backlog. It is not sorted by priority. Completed
rows record what a branch actually delivered; a completed tranche row does not
mean the whole workstream is complete. Use the current delivery queue first,
then choose one unchecked row that fits the latest report evidence.

| Done | Status | Task | Workstream | Purpose | Current notes |
| --- | --- | --- | --- | --- | --- |
| [x] | Done | Establish persistent development plan | Planning | Create the general project plan, task board, and task protocol. | Replaces the earlier narrow plan with a broader development plan. Future branches should update this board when they start and finish work. |
| [x] | Done | Generalize project roadmap scope | Planning | Rebalance the plan around product development order instead of an optimization-first backlog. | Clarifies that performance and memory are recurring guardrails, while embedding API, compatibility, host extensions, resource control, and observability are first-class roadmap tracks. |
| [x] | Done | Embedding API skeleton | Embedding API | Introduce the public direction for `Engine`, isolated `Vm`, execution `Context`, and embedder-owned configuration. | Adds `Engine`, `EngineConfig`, `Vm`, `VmConfig`, VM resource usage, teardown reports, README coverage, and direct library tests for isolated VMs, output separation, VM-specific limits, and teardown. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T144858Z.md` keeps existing benchmark exceptions tracked. |
| [x] | Done | Multi-VM isolation fixtures | Embedding API / testing | Prove that many VMs can run in one Rust process with isolated globals, output, limits, errors, and teardown. | Adds a direct library fixture that runs eight VMs in one process, verifies isolated globals and output buffers, forces a separate VM resource-limit failure, then confirms the surviving VMs continue and produce teardown reports. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T145428Z.md` keeps benchmark exceptions tracked. |
| [x] | Done | Host function API skeleton | Embedding API | Add the first typed Rust host function registration path. | Adds synchronous `Context::register_host_function`, `HostCall` checked argument accessors, contextual callback errors, VM-local callback storage, and conservative rejection of VM-owned handle return values. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T150355Z.md` keeps benchmark exceptions tracked. Async support remains for the promise/job queue task. |
| [x] | Done | Test262 feature map | Compatibility / testing | Convert full Test262 results into a feature-oriented progress map. | Adds compact full-corpus feature-area tables with pass/fail/skip counts, pass rate, active-manifest counts, top skip reasons, and an aggregated `other feature areas` row. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T151636Z.md` records 102578 executed Test262 variants, 9098 passed, 93480 failed, and keeps failed case details capped at the last 30. |
| [x] | Done | Parser and lexer Test262 cluster | Compatibility | Reduce top parser and lexer failure categories in full Test262 reports. | Adds the `numeric-literal-syntax` cluster: binary, octal, hexadecimal, decimal exponent, leading-decimal, and numeric separator support without BigInt semantics. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T152806Z.md` raises full Test262 passes from 9098 to 9378, raises `language/literals` passes from 514 to 784, lowers parser failures from 25349 to 23170, and lowers the top `lexer: unexpected character` hint from 17549 to 16369. BigInt literals remain unsupported and are now reported explicitly. |
| [x] | Done | Runtime semantics cluster | Compatibility | Expand coherent statement, expression, scope, function, and error semantics. | Adds omitted catch binding support (`try { ... } catch { ... }`) without creating a catch parameter binding. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T153644Z.md` raises full Test262 passes from 9378 to 9388, lowers parser failures from 23170 to 23160, and adds one active Test262 and one QuickJS differential case. |
| [x] | Done | Clarify general development sequence | Planning | Make the working plan clearly describe the whole product roadmap instead of looking like a single-workstream plan. | Adds product delivery milestones, aligns the short roadmap with the operational plan, and frames performance work as acceptance criteria plus checkpoint tasks. This was a documentation-only change validated with `git diff --check`; full CI remains the merge gate. |
| [x] | Done | Basic built-ins expansion | Compatibility | Expand high-value `Object`, `Array`, `String`, `Number`, and `Math` behavior. | Starts the built-ins track with `Number` as a function/constructor, `Number.prototype.constructor`, basic string/boolean/null/undefined conversions, and static constants such as `NaN`, infinities, safe integer bounds, `MAX_VALUE`, `MIN_VALUE`, and `EPSILON`. Adds engine, active Test262, QuickJS differential, and benchmark fixtures. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T155826Z.md` raises full Test262 passes from 9388 to 9609, raises `built-ins/Number` passes from 8 to 32, and removes `missing binding: Number` from the top missing bindings. The new `number_builtin` benchmark is tracked as a latency exception at `1.29x`; `Number.prototype` primitive-wrapper internals remain future work. |
| [x] | Done | `CompiledScript` AST wrapper | Embedding API / performance | Add a reusable compiled representation before bytecode, so embedders can parse once and evaluate repeatedly. | Adds AST-backed `CompiledScript` and `CompiledScriptUsage`, plus `compile`/`eval_compiled` on `Context`, `Vm`, and compatibility `Runtime`. The parser now reports compile usage so target VMs reject compiled scripts that exceed their stricter source, statement, or expression-depth limits. Direct library tests cover repeated eval in one VM, reuse across isolated VMs, compile-time parse errors, and stricter target VM limits. Benchmark reports now separate cold eval, compile-only, and compiled-eval columns and add `compiled_script_reuse`. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T161440Z.md` keeps full Test262 at 9609 passed, measures 51 benchmarks, and records `compiled_script_reuse` as a rounded `1.10x` latency exception while showing `3 us` compile-only, `54 us` compiled eval, and `67 us` cold eval. Future bytecode, atoms, and slot locals can replace the backing representation behind this API. |
| [x] | Done | Atom interner | Performance and memory | Store identifiers, property keys, function names, and reusable string constants as compact atom ids. | Starts atomization with a VM-owned `AtomTable`, `AtomId`, and lexical/global bindings keyed by atoms instead of owned strings. Public APIs still accept string names, missing-name lookups do not create atoms, and `VmResourceUsage` now reports `atom_count`. Adds direct embedding coverage for atom accounting and a binding-heavy `atomized_bindings` benchmark. Property keys, function metadata, reusable string constants, and atom budgets remain follow-up work so this branch does not mix atomization with shape/object-layout changes. Validation passed with `cargo fmt -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T162515Z.md` keeps full Test262 at 9609 passed, measures 52 benchmarks, brings `compiled_script_reuse` within budget at `1.09x`, and records `atomized_bindings` as a `1.17x` latency exception. |
| [x] | Done | Project plan naming and sequence | Planning | Make the canonical plan name and task order reflect the whole product roadmap. | Renames the canonical plan to `docs/project-plan.md`, updates README and roadmap links, broadens the backlog around API, compatibility, async, resources, observability, and runtime architecture, and moves performance work into recurring checkpoint and runtime-architecture tracks. Validation passed with stale-link checks and `git diff --check`; full CI remains the merge gate. |
| [x] | Done | Binding slot storage foundation | Runtime architecture / performance | Move runtime binding storage from map-held cells toward slot-indexed storage before compiler-assigned slots. | Adds a checked `BindingSlot` newtype and makes each `BindingScope` store `BindingCell` values in a `Vec`, with the atom map acting as an atom-to-slot index. Direct embedding coverage verifies assignment updates and lexical shadowing through the slot-backed scope. Full compiler-assigned local, global, and upvalue slots remain the follow-up `Slot-based local bindings` task. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T164512Z.md` keeps full Test262 at 9609 passed, active Test262 and QuickJS differential at 100%, `compiled_script_reuse` within budget at `1.08x`, and `atomized_bindings` as a tracked latency exception at `1.15x`. |
| [x] | Done | Embedding API stability pass | Embedding API | Review `Engine`, `Vm`, `Context`, `CompiledScript`, configuration, teardown, and error surfaces before more features depend on them. | Adds VM-level wrappers for common embedder operations so ordinary callers do not need to route through `Context` for eval, globals, output, or host registration. The README and direct embedding test now exercise `Vm::register_host_function_typed`, `Vm::register_host_function`, `Vm::eval`, `Vm::compile`, `Vm::eval_compiled`, `Vm::get_global`, `Vm::output`, and `Vm::take_output`. `Context` remains available for lower-level control. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T170105Z.md` keeps full Test262 at 9609 passed, active Test262 and QuickJS differential at 100%, and leaves existing benchmark exceptions to checkpoint tasks. |
| [x] | Done | Host value conversion layer | Embedding API | Add typed conversions between Rust values and JavaScript values for host functions. | Adds `IntoJsValue`, `FromJsValue`, generic `HostCall::argument<T>()`, and `Context::register_host_function_typed` while keeping the existing `Value`-returning callback API compatible. Direct embedding tests cover typed argument extraction, typed returns for `String`, `f64`, `bool`, and `()`, contextual host errors, VM-local callbacks, and VM-owned handle rejection. The README embedding example now uses the typed host API. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T165400Z.md` keeps full Test262 at 9609 passed, active Test262 and QuickJS differential at 100%, and leaves existing benchmark exceptions to checkpoint tasks. |
| [x] | Done | General product roadmap order | Planning | Make the canonical plan read as a whole-project delivery order, not as an optimization backlog. | Reframes the plan around repository reliability, embedding API, compatibility, built-ins, diagnostics, modules/jobs/async, resources, observability, and only then deeper runtime architecture. Performance and memory remain acceptance criteria and recurring checkpoint tasks rather than the main roadmap label. This documentation-only change was validated with `git diff --check`; full CI remains the merge gate. |
| [x] | Done | Roadmap first-screen refresh | Planning | Make the top of the canonical plan show the whole-product order before optimization details. | Renames the document heading to a roadmap and execution plan, adds a near-term product order, and clarifies that atoms, slots, shapes, bytecode, inline caches, and GC are architecture work in service of compatibility, embedding, resources, observability, performance, and memory footprint. Documentation-only validation passed with `git diff --check`, README target file checks, and `cargo fmt --all -- --check`; full CI remains the merge gate. |
| [x] | Done | Whole-project delivery queue | Planning | Make the plan show what the project will build and in what rough order, not only where optimization work may happen. | Adds explicit guidance for reading the plan, separates product delivery order from runtime foundation work, and adds a concrete near-term queue led by report triage, compatibility, built-ins, embedding API, diagnostics, async, resources, and observability. Documentation-only validation passed with `git diff --check` and `cargo fmt --all -- --check`; full CI remains the merge gate. |
| [x] | Done | Project-wide sequence refresh | Planning | Make the current plan read as the whole project order instead of an optimization-oriented queue. | Renames the document heading to `Project Development Plan`, adds an explicit plan-scope section, refreshes the current delivery queue after the JSON tranche, and clarifies that the task board is historical plus backlog rather than the priority order. Documentation-only validation passed with `git diff --check`; full CI remains the merge gate. |
| [x] | Done | Engine case registry split | Testing / maintenance | Keep the engine fixture registry below the project file-size limit before adding more compatibility tranches. | Moves runtime/error/built-in engine fixture registration out of the central `cases.rs` file into `cases_engine_runtime.rs`, reducing `cases.rs` from 789 to 745 lines so future built-in cases can be added without pushing the main registry over 800 lines. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T184709Z.md` keeps engine fixtures at 57/57, active Test262 at 57/57, full Test262 at 9782/102578, and QuickJS differential at 55/55. Benchmark counts are behavior-equivalent but show normal measurement noise with latency exceptions at 26 and memory exceptions at 2. |
| [x] | Done | Runtime performance architecture review | Planning / runtime architecture | Incorporate the latest runtime-performance review into the general project plan without turning the plan into an optimization-only backlog. | Records that compile time is no longer the main measured debt for many heavy benchmarks, adds a status matrix for slots, atoms, shapes, inline caches, dense arrays, built-in intrinsics, bytecode quickening, memory layout, and parallel execution, and updates the delivery queue so profiling and runtime data-model work can be pulled forward when reports justify it. Documentation-only validation passed with `git diff --check` and `cargo fmt --all -- --check`; full CI remains the merge gate. |
| [x] | Done | Whole-project sequence correction | Planning | Keep the canonical plan organized around the whole project order rather than the latest optimization discussion. | Reorders the current delivery queue around compatibility, built-ins, embedding API, diagnostics, async, resources, observability, runtime foundations, and recurring performance checkpoints. Keeps the architecture review details, but makes profiling a supporting evidence step rather than the apparent next mandatory project goal. Documentation-only validation passed with `git diff --check` and `cargo fmt --all -- --check`; full CI remains the merge gate. |
| [ ] | Backlog | Report triage cadence | Testing / planning | Keep the next work item grounded in the latest Test262, QuickJS differential, benchmark, and memory evidence. | Before selecting each compatibility or architecture tranche, summarize the newest report signals and record why the chosen task is next. |
| [ ] | Backlog | Runtime hot-path profiling pass | Performance and memory / runtime architecture | Profile benchmark groups before broad runtime data-structure rewrites. | Use after the next selected product or compatibility branch needs runtime architecture evidence. The current benchmark table shows debt in descriptors, object/prototype traversal, arrays, built-ins, `in`, and compiled evaluation, but profiling is an evidence step for broader work rather than the whole project direction. |
| [ ] | Backlog | Library API documentation pass | Embedding API / documentation | Keep crate docs, README examples, and direct library tests aligned with the current public API. | Do this whenever API shape changes enough that embedders could be confused by stale examples. |
| [x] | Done | JSON built-in tranche | Compatibility | Add the first useful `JSON` object surface for embedders and Test262 progress. | Adds `JSON.parse` and `JSON.stringify` for primitives, arrays, and plain objects, including non-enumerable `parse`/`stringify`, function metadata, array omission/null handling, object omission handling, non-finite numbers, and negative zero stringification. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, six upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T203225Z.md` raises engine fixtures to 65/65, active Test262 to 63/63, QuickJS differential to 61/61, full Test262 passes from 10371 to 10432, and `built-ins/JSON` to 52/330 passed. The new `json_builtin` benchmark is tracked as a latency and memory exception at `1.60x` and `1.74x`; reviver, replacer, spacing, `toJSON`, raw JSON, SyntaxError typing, property descriptors, and global configurable delete semantics remain follow-up work. |
| [x] | Done | Object property and descriptor tranche | Compatibility | Improve object semantics that many built-ins and Test262 cases depend on. | Adds data-property attributes, non-configurable delete behavior, non-writable assignment behavior, `Object.getOwnPropertyDescriptor`, `Object.defineProperty`, `Object.keys`, and `Object.hasOwn`, with non-enumerable built-in static methods that do not consume user property limits. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, five upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T205739Z.md` raises engine fixtures to 66/66, active Test262 to 64/64, QuickJS differential to 62/62, full Test262 passes from 10432 to 10854, and `built-ins/Object` to 428/6802 passed. The new `object_descriptors` benchmark is tracked as a latency and memory exception at `2.21x` and `1.42x`; accessor descriptors, symbols, freeze/seal/preventExtensions, full global-object descriptors, function-object descriptors, and shape-based layout remain follow-up work. |
| [ ] | Backlog | Array built-in tranche | Compatibility | Grow practical array behavior without prematurely rewriting storage. | Add high-value prototype methods and semantics in small clusters. Pull dense array storage forward only when the current representation blocks correctness, resource accounting, or measured hot paths. |
| [x] | Done | Function descriptor tranche | Compatibility / embedding API | Allow descriptor APIs to operate on ordinary and native function values. | Extends `Object.defineProperty` and `Object.getOwnPropertyDescriptor` to `Function` and `NativeFunction` values for data descriptors, including custom enumerable/non-writable/non-configurable properties and descriptor queries for `name`, `length`, and `prototype`. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T211411Z.md` raises engine fixtures to 67/67, active Test262 to 65/65, QuickJS differential to 63/63, and full Test262 passes from 10854 to 10872. The new `function_descriptors` benchmark is tracked as a latency and memory exception at `1.85x` and `1.54x`; full intrinsic reconfiguration and delete semantics for `name`/`length`/`prototype`, upstream `built-ins/Function` manifest coverage, accessor descriptors, and standard error expansion remain follow-up work. |
| [x] | Done | Standard errors and function intrinsic semantics tranche | Compatibility / embedding API | Improve standard error objects and complete configurable function intrinsic behavior. | Narrows the branch to full configurable `name` and `length` redefinition/delete behavior for ordinary and native functions, including standard error constructors such as `TypeError`. Adds intrinsic descriptor override/delete state, custom property fallback after deletion, enumerable key handling for redefined intrinsics, direct Rust smoke coverage, engine fixture, active Test262 fixture, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T215331Z.md` raises engine fixtures to 68/68, active Test262 to 66/66, QuickJS differential to 64/64, and keeps full Test262 at 10872/102578. The new `function_intrinsic_descriptors` benchmark is tracked as a latency and memory exception at `1.93x` and `1.36x`; `Function.prototype` fallback after deleting own `name`/`length`, upstream `built-ins/Function` manifest coverage, accessor descriptors, and broader standard error prototype/descriptors remain follow-up work. |
| [x] | Done | Parser and syntax compatibility tranches | Compatibility | Continue reducing Test262 parser and lexer failure clusters. | Adds the `no-substitution-template-literal` cluster: backtick string literals without `${...}` interpolation, including common escapes and line terminator normalization. Template substitutions now fail with an explicit unsupported-feature lexer error. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, upstream Test262 manifest row, and QuickJS differential coverage. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T172449Z.md` raises full Test262 passes from 9609 to 9616, raises `language/expressions` passes from 4153 to 4157, raises active Test262 to 55/55, raises QuickJS differential to 53/53, and lowers the top `lexer: unexpected character` hint from 16369 to 6531. Some cases now advance from lexer failures into parser/runtime failure classes, which is expected until interpolation, tagged templates, BigInt, and related syntax are implemented. No new benchmark case was added because this is parser-only literal support; existing benchmark exceptions remain tracked in the report. |
| [ ] | Backlog | Runtime semantics tranches | Compatibility | Expand statements, functions, lexical environments, `this`, exceptions, equality, iteration, and prototype behavior. | Each tranche should improve a visible Test262 feature area without mixing unrelated semantics. |
| [x] | Done | Practical built-ins tranches | Compatibility | Expand `Object`, `Array`, `String`, `Number`, `Math`, `Boolean`, `Function`, errors, JSON, Date, RegExp, Map, Set, and other high-value built-ins. | Adds the basic `String` built-in: callable `String(value)` conversion, constructable `new String(value)` wrapper objects with `String.prototype`, non-enumerable `length`, enumerable character indices, primitive string `length`/index lookup, `in`, and `for...in` support. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T173929Z.md` raises full Test262 passes from 9616 to 9722, raises `built-ins/String` passes from 4 to 62, raises active Test262 to 56/56, raises QuickJS differential to 54/54, and removes `String` from the top missing bindings table. The new `string_builtin` benchmark is tracked as a latency exception at `1.50x` while staying within memory budget at `1.07x`; full String prototype methods, wrapper internal primitive semantics, abstract equality/object-to-primitive coercion, and string Unicode code-unit accuracy remain follow-up work. |
| [x] | Done | Boolean built-in tranche | Compatibility | Promote `Boolean` from a special call-path into a normal built-in binding and constructor. | Adds `Boolean` as a normal native built-in binding with call and constructor paths, `Boolean.prototype.constructor`, function `name`/`length`/`prototype` metadata, and shadowing-correct call behavior by removing the old direct-call special case. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T183517Z.md` raises full Test262 passes from 9722 to 9782, raises active Test262 to 57/57, raises QuickJS differential to 55/55, and removes `Boolean` from the top missing bindings table. The new `boolean_builtin` benchmark is tracked as a latency exception at `1.47x` while staying within memory budget at `0.94x`; Boolean wrapper internal primitive semantics and `Boolean.prototype.valueOf`/`toString` remain follow-up work. |
| [x] | Done | Global numeric constants tranche | Compatibility | Add standard global numeric value properties that remove the remaining `NaN` missing-binding cluster. | Adds immutable lazy global bindings for `NaN` and `Infinity`, materializes lazy built-ins before assignment, compound assignment, and delete operations, and adds Rust smoke coverage plus engine, active Test262, upstream Test262 manifest, QuickJS differential, and benchmark fixtures. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T190135Z.md` raises full Test262 passes from 9782 to 10003, raises engine fixtures to 60/60, raises active Test262 to 58/58, raises QuickJS differential to 56/56, raises `built-ins/Number` passes from 32 to 116, raises `built-ins/String` passes from 62 to 76, and removes `NaN` from the top missing bindings table. The new `global_numeric_constants` benchmark is tracked as a latency exception at `1.22x` while staying within memory budget at `0.91x`; full global object property descriptors remain follow-up work. |
| [x] | Done | Math built-in tranche | Compatibility | Add the first standard `Math` object surface and reduce the `missing binding: Math` cluster. | Adds the `Math` object, standard numeric constants, and `abs`, `ceil`, `floor`, `max`, `min`, `pow`, `round`, `sqrt`, and `trunc`, with signed-zero handling for `max`, `min`, and `round`. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, six upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T192241Z.md` raises engine fixtures to 61/61, active Test262 to 59/59, QuickJS differential to 57/57, full Test262 passes from 10003 to 10193, and `built-ins/Math` to 154/654 passed with `Math` removed from top missing bindings. The new `math_builtin` benchmark is tracked as a latency and memory exception at `1.59x` and `1.24x`; full property descriptor semantics, remaining Math methods, and faster native-call/property paths remain follow-up work. |
| [x] | Done | Math methods tranche | Compatibility | Add the next standard `Math` method cluster after the basic object surface. | Adds safe `f64`-backed numeric methods: `acos`, `acosh`, `asin`, `asinh`, `atan`, `atan2`, `atanh`, `cbrt`, `cos`, `cosh`, `exp`, `expm1`, `hypot`, `log`, `log10`, `log1p`, `log2`, `sign`, `sin`, `sinh`, `tan`, and `tanh`. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, 23 upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T194126Z.md` raises engine fixtures to 62/62, active Test262 to 60/60, QuickJS differential to 58/58, full Test262 passes from 10193 to 10343, and `built-ins/Math` to 282/654 passed. The new `math_methods` benchmark is tracked as a latency exception at `2.28x` while staying within memory budget at `1.00x`; integer coercion methods (`clz32`, `imul`), float32 rounding, `random`, descriptor semantics, `Symbol.toStringTag`, and faster native-call/property paths remain follow-up work. |
| [x] | Done | Math integer methods tranche | Compatibility | Add deterministic remaining `Math` numeric methods before `random` and descriptor work. | Adds `Math.clz32`, `Math.imul`, and `Math.fround`, with shared safe ToUint32 conversion and a locally justified binary32 cast for ECMAScript `fround` semantics. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, 12 upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T195229Z.md` raises engine fixtures to 63/63, active Test262 to 61/61, QuickJS differential to 59/59, full Test262 passes from 10343 to 10367, and `built-ins/Math` to 306/654 passed. The new `math_integer_methods` benchmark is tracked as a latency exception at `2.79x` while staying within memory budget at `1.01x`; `Math.random`, descriptor semantics, `Symbol.toStringTag`, and faster native-call/property paths remain follow-up work. |
| [x] | Done | Math random tranche | Compatibility | Add `Math.random` without mixing in descriptor semantics. | Adds a deterministic per-VM xorshift PRNG for `Math.random`, returning numbers in `[0, 1)` without unsafe code or global state. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, one upstream Test262 manifest row, QuickJS differential property coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T200145Z.md` raises engine fixtures to 64/64, active Test262 to 62/62, QuickJS differential to 60/60, full Test262 passes from 10367 to 10371, and `built-ins/Math` to 308/654 passed. The new `math_random` benchmark is tracked as a latency exception at `1.69x` while staying within memory budget at `0.98x`; explicit seeding, stronger PRNG selection, descriptor semantics, `Symbol.toStringTag`, and faster native-call/property paths remain follow-up work. |
| [ ] | Backlog | Diagnostics and error model | Compatibility / embedding API | Make syntax, runtime, host callback, and resource-limit errors precise and stable enough for embedders. | Preserve error chains and report enough location/context data for production logging. |
| [ ] | Backlog | Module loading design | Compatibility / embedding API | Design how modules are resolved, loaded, cached, limited, and observed by embedding applications. | Keep the embedder in control of I/O and policy; document what is intentionally unsupported in the first implementation. |
| [ ] | Backlog | Promise job queue | Compatibility / embedding API | Add the JavaScript job model required by promises and async functions. | The VM owns JavaScript jobs; the embedding application controls when jobs are drained. |
| [ ] | Backlog | Async host callbacks | Embedding API | Allow Rust host callbacks to complete asynchronously through embedder-owned executors. | Should build on the promise job queue and preserve VM isolation, cancellation, and resource accounting. |
| [ ] | Backlog | Resource limit expansion | Resource control | Extend limits from source and runtime counters toward heap, stack, atom table, jobs, host callbacks, modules, and cancellation. | Every new limit needs library tests, error reporting, and teardown accounting. |
| [ ] | Backlog | Observability hooks | Observability | Add structured execution events, profiling hooks, resource usage snapshots, and teardown reports. | Useful for production embedding, debugging, and future performance work. |
| [x] | Done | Full atomization hot path | Runtime architecture / performance | Extend atoms beyond binding names so hot runtime paths stop returning to owned strings. | First architecture tranche moves ordinary object property storage and property order from owned `String` keys to VM-owned atom keys, while preserving string names only at API boundaries for array index parsing, `length`, and `__proto__`. Adds `PropertyKey`, `PropertyLookup`, and `ObjectPropertyInit`, keeps missing property reads from interning new atoms, tracks numeric prototype properties for array index lookup through `sparse_array_keys`, and splits object modules to keep `runtime_object.rs` below the file-size limit. Direct embedding coverage verifies property atom accounting and missing-property behavior. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T222712Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark exceptions remain in object/prototype and `in` paths, so shapes, compiler-assigned property operands, inline caches, and function/native property atomization remain follow-up work. |
| [x] | Done | Well-known property atom cache | Runtime architecture / performance | Stop repeated atom-table string lookup for standard property keys used by built-ins, prototypes, and function metadata. | Adds a VM-local cache for well-known `PropertyKey` values such as `constructor`, `length`, `name`, and `prototype`, routes `intern_property_key` plus `property_lookup` through it before falling back to the atom table, and returns function/native intrinsic `length`, `name`, and `prototype` values before building fallback custom-property lookups. Dynamic property names, JSON keys, and string indices remain on the generic interning path so this stays a narrow atomization tranche before shared shapes and immutable intrinsic metadata. Direct embedding coverage verifies repeated well-known builtin/function/string paths do not grow the atom table after first materialization. Validation passed with `cargo fmt --all -- --check`, targeted embedding and function smoke tests, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T002800Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains tracked runtime-model debt: object/prototype, function custom-property, built-in, binding, and array rows still need shared shapes, compiler-assigned property operands, direct native-call paths, and deeper dense-array mutation fast paths. |
| [x] | Done | Atom table vector index tranche | Runtime architecture / performance | Remove tree-map lookup from the VM atom table before deeper property operands and slot compiler work. | Replaces the internal `BTreeMap<String, AtomId>` with a sorted `Vec<AtomEntry>` plus checked binary search while preserving stable `AtomId` allocation order and `name()` lookup through the existing string storage. Direct smoke coverage verifies out-of-order property-name atoms, repeated property reads without atom growth, missing-property reads without interning, and later insertion of a new property name. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T015816Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. This is a data-structure foundation; `atomized_bindings` remains a tracked latency exception at `1.21x`, so compiler-assigned slots and property operands remain the real hot-path follow-up. |
| [x] | Done | Static AST name operands tranche | Runtime architecture / performance | Stop representing parser-known identifiers and static property names as untyped raw strings before compiler-assigned slots and property operands. | Adds a `StaticName` newtype backed by `Rc<str>` and uses it for declaration names, identifier expressions, function names and parameters, object literal keys, static member names, assignment targets, `new` constructor names, catch bindings, and for-in bindings. Runtime string APIs still receive `&str` at the boundary, and object creation still materializes owned names where enumeration/diagnostics require them. Direct embedding coverage compiles and reuses a script that exercises static binding, function parameter, constructor, `this` member assignment, object method, member call, for-in, and catch-name paths while verifying repeated compiled evaluation does not grow the atom table. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T020845Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal is unchanged runtime-model debt: `compiled_script_reuse` is `1.24x` latency and `1.12x` memory, and `atomized_bindings` is `1.26x` latency, so `LocalSlot`, `GlobalSlot`, `UpvalueSlot`, and shape/property operands remain the next hot-path work. |
| [x] | Done | Static name table tranche | Runtime architecture / memory | Deduplicate parser-known names within each compiled script before adding binding and property operand resolvers. | Adds a parser-local sorted `StaticNameTable` so repeated identifiers, function parameters, object literal keys, static member names, constructor names, catch bindings, and for-in bindings share the same `StaticName` handle inside the AST instead of allocating separate `Rc<str>` values. `CompiledScriptUsage` now exposes `static_name_count()` so embedders and future resolver tests can observe the script-local name set. Direct embedding coverage verifies that a repeated binding/property/method script compiles to four unique static names and still evaluates correctly. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T022158Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. This is still pre-resolver architecture work: `compiled_script_reuse` remains a tracked exception at `1.21x` latency, and `atomized_bindings` remains `1.23x` latency and `1.10x` memory, so `LocalSlot`, `GlobalSlot`, `UpvalueSlot`, and shape/property operands remain next. |
| [x] | Done | Function property atomization tranche | Runtime architecture / performance | Remove the remaining string-keyed custom property storage from ordinary and native functions. | Converts `FunctionProperties` custom property maps and property order from owned `String` keys to VM-owned `PropertyKey` values shared with object storage. Preserves `name`, `length`, and `prototype` intrinsic behavior at string API boundaries, makes custom ordinary/native function reads use `PropertyLookup` so missing property reads do not intern atoms, and changes the property set path to borrow property names instead of passing owned `String` values. Splits `runtime_function_properties.rs` out of `runtime_function.rs` to keep files under the project line limit, and adds embedding coverage for ordinary and native function property atom accounting. Validation passed with `cargo fmt --all`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T224246Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal: `function_properties` is within budget at `1.08x` latency and `1.02x` memory, while `function_custom_properties` remains a tracked exception at `1.13x` latency and `1.11x` memory; remaining follow-up work belongs to shapes, compiler-assigned property operands, inline caches, and compact heap strings. |
| [x] | Done | Function property vector index tranche | Runtime architecture / performance | Remove the tree-map storage/index from ordinary and native function custom properties before shapes and direct native-call paths. | Replaces `FunctionProperties` custom-property `BTreeMap<PropertyKey, FunctionProperty>` with a sorted `Vec<FunctionPropertyEntry>` and checked binary search while preserving insertion-order enumeration through `property_order`. Direct function custom-property coverage verifies out-of-order lookup, descriptor reads, deletion/reinsertion, `for...in`, and `in`; intrinsic `name`, `length`, and `prototype` behavior stays separate. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, targeted function property tests, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T234624Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains shape/native-call debt: `function_properties` is `1.12x`, `function_custom_properties` is `1.15x`, `function_descriptors` is `1.86x`, `function_intrinsic_descriptors` is `2.05x`, `constructor_prototypes` is `1.28x`, and `prototype_constructor_property` is `1.24x`, so shared shapes, prototype versioning, direct native calls, and inline caches remain follow-up work. |
| [x] | Done | Object property slot storage tranche | Runtime architecture / performance | Move ordinary object named properties from map-owned records toward slot-array storage before shared shapes. | Replaces `BTreeMap<PropertyKey, ObjectProperty>` plus a separate property-order vector with `BTreeMap<PropertyKey, PropertySlot>` pointing into checked `Vec<NamedProperty>` storage. Enumeration now walks slot order directly, delete/reinsert compacts and reindexes slots, descriptors and sparse-array fallback use the same slot helpers, and a focused object descriptor smoke test verifies descriptor lookup after slot deletion, reindexing, reinsertion, `Object.keys`, `for...in`, and `in`. This is a shape-layout foundation rather than full hidden classes: objects still own their slot layout, and shared `ShapeId` transitions remain a follow-up. Validation passed with `cargo fmt --all`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T225403Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal is mixed as expected for a storage refactor without shape caches: `object_literals` and shorthand methods remain within budget, while `object_prototypes`, `object_prototype_root`, `object_builtin`, `object_descriptors`, `for_in`, and `in_operator` still need shared shapes, prototype versioning, compiler-assigned property operands, and inline caches. |
| [x] | Done | Object property vector index tranche | Runtime architecture / performance | Remove the remaining tree-map index from ordinary object named-property lookup before shared shapes. | `Object` already stored named properties in insertion-order `Vec<NamedProperty>` storage; this branch replaces its `BTreeMap<PropertyKey, PropertySlot>` index with a sorted `Vec<PropertyIndexEntry>` and checked binary search. Direct object descriptor coverage verifies out-of-order property lookup, descriptor reads, deletion/reinsertion, insertion-order `Object.keys`, `for...in`, and `in` semantics. This remains a foundation for shared `ShapeId` layouts and inline caches rather than a full hidden-class implementation. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T233854Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains shape-driven debt: `object_prototypes` is `1.16x`, `object_prototype_root` is `1.34x`, `object_builtin` is `1.29x`, `object_descriptors` is `2.08x`, `for_in_statements` is `1.24x`, and `in_operator` is `1.19x`, so shared shapes, prototype versioning, compiler-assigned property operands, and inline caches remain the next object-path work. |
| [x] | Done | Function parameter atom layout tranche | Runtime architecture / performance | Stop re-interning function parameter names on every ordinary function call. | Ordinary functions now store an `Rc<[AtomId]>` parameter layout when the function object is created, and call-scope creation binds arguments through those atoms instead of interning `String` parameter names at each invocation. Direct embedding coverage verifies that repeated calls do not grow the atom table after function creation. This is a narrow prerequisite for compiler-assigned `LocalSlot`, `GlobalSlot`, and `UpvalueSlot` operands rather than the final slot compiler. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T230407Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. |
| [x] | Done | Function parameter arity layout tranche | Runtime architecture / memory | Stop retaining source parameter strings in runtime function objects after atom layout creation. | Ordinary runtime functions now store a `FunctionArity` newtype plus the existing `Rc<[AtomId]>` parameter layout instead of retaining the parser's `Rc<[String]>` parameter list. Function `length` still comes from checked arity conversion, and direct embedding coverage verifies `length` plus repeated-call atom accounting. This keeps the AST representation unchanged until compiler-assigned slots and bytecode replace it. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T231207Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. |
| [x] | Done | Function name atom layout tranche | Runtime architecture / memory | Store ordinary runtime function names as atoms instead of owned strings. | Ordinary functions now store `FunctionName(Option<AtomId>)`; named functions intern their name once at creation, anonymous functions do not intern an empty string, and `name` property reads materialize a JS string only at the API boundary. Direct embedding coverage verifies named and anonymous `name` behavior plus repeated name reads without atom-table growth. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T232023Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. |
| [x] | Done | Binding scope vector index tranche | Runtime architecture / performance | Remove the remaining tree-map index from runtime binding scopes before compiler-assigned slots. | `BindingScope` still stores cells in a `Vec`, but its atom-to-slot index is now a sorted `Vec<BindingEntry>` with checked binary search instead of a `BTreeMap`. Direct embedding coverage verifies out-of-order declaration lookup, updates, lexical shadowing, function call scope use, and global reads. This remains a foundation for the larger compiler-assigned `LocalSlot`, `GlobalSlot`, and `UpvalueSlot` task rather than the final slot compiler. Validation passed with `cargo fmt --all`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T232944Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal is still a tracked debt: `atomized_bindings` remains `1.15x` latency and `1.11x` memory, so the next real binding win still needs compiler-assigned slots and upvalues. |
| [x] | Done | Static name id operands tranche | Runtime architecture / performance | Give every parser-known name a stable script-local id before adding binding and property operand resolvers. | Adds a checked `StaticNameId` newtype, makes `StaticName` carry both the script-local id and shared text, and changes the parser `StaticNameTable` to keep first-seen name storage separate from the sorted lookup index. Runtime evaluation still uses the existing `as_str()` boundary, so this branch prepares future `LocalSlot`, `GlobalSlot`, `UpvalueSlot`, static property operands, and bytecode operands without changing runtime resolution yet. Direct embedding coverage now verifies repeated out-of-order static names through compiled evaluation. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, targeted static-name tests, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T023251Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark debt remains runtime-model driven: `compiled_script_reuse` is a tracked latency exception at `1.10x`, and `atomized_bindings` remains `1.22x`, so compiler-assigned slots are still the next binding-path step. |
| [x] | Done | Static name atom cache tranche | Runtime architecture / performance | Stop repeatedly resolving parser-known static names through string lookup before compiler-assigned slots. | Adds a VM-local lazy `StaticNameId -> AtomId` cache for compiled-script evaluation, stores the cache with escaped functions, and uses a compact `Rc<[Cell<Option<AtomId>>]>` slot array so hot static-name access does not take locks. Binding identifiers, assignments, declarations, function names, function parameters, constructor names, and static property operands now resolve through the cache while missing-name lookups still do not intern atoms. Direct coverage verifies missing compiled identifiers, repeated compiled static-name paths, and escaped compiled functions after the original eval frame is gone. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, targeted compiled-static-name tests, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T025217Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark exceptions improved from the previous local report to 39 latency and 19 memory rows; `atomized_bindings` is down to `1.12x`, while `compiled_script_reuse` remains a near-threshold tracked latency exception at `1.10x`, so full `LocalSlot`, `GlobalSlot`, and `UpvalueSlot` operands remain next. |
| [x] | Done | Static binding slot cache tranche | Runtime architecture / performance | Route repeated parser-known binding access through checked binding-slot locations before the full scope resolver lands. | Adds a script-local `StaticNameId -> BindingLocation` cache for compiled evaluation, with guarded global/local `BindingSlot` locations that validate the current lexical stack, slot atom, and shadowing before reading or assigning. `BindingScope` keeps direct slot access safe through a compact parallel slot-atom vector, compiled functions capture the binding cache alongside the atom cache, and declaration hoisting seeds locations after bindings are created. This is intentionally not the final compiler-assigned scope layout: validation still falls back to runtime resolution when scopes change, and full `LocalSlot`, `GlobalSlot`, and `UpvalueSlot` operands remain the next binding-path task. Direct coverage verifies block shadowing, captured parameter shadowing, missing compiled names without atom growth, and escaped compiled functions. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, targeted compiled-static-name tests, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T030638Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains binding-runtime debt rather than compile debt: `function_parameters_scope` and `closure_environments` are within budget, `atomized_bindings` is a latency exception at `1.12x` with memory within budget, and `compiled_script_reuse` is a latency exception at `1.15x`. |
| [x] | Done | Static binding occurrence operands tranche | Runtime architecture / performance | Give every parser-known binding use its own checked operand id before assigning compile-time local/global/upvalue slots. | Adds `StaticBindingId` and `StaticBinding` so identifier reads, assignments, declarations, parameters, catch bindings, for-in bindings, shorthand object reads, and `new` constructor lookups no longer share one cache slot only because their text matches. `CompiledScriptUsage` now reports `static_binding_count`, runtime static-binding caches are sized and indexed by binding occurrence rather than `StaticNameId`, and parser speculation for ordinary `for` loops now restores static-name and static-binding tables when the `for-in` parse path does not match. Property keys and function display names remain `StaticName` operands because they are not binding lookups. Direct coverage verifies deduplicated static names with distinct binding occurrences and the `for` parser checkpoint. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, targeted static-name/static-binding tests, targeted compiled-static-name tests, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T032116Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains runtime-model debt: `atomized_bindings` is a latency and memory exception at `1.16x` and `1.10x`, `compiled_script_reuse` is a latency exception at `1.16x`, and the next binding-path step is still assigning `LocalSlot`, `GlobalSlot`, and `UpvalueSlot` from these occurrence operands. |
| [x] | Done | Compiled binding layout tranche | Runtime architecture / performance | Build a scope-aware binding operand table before replacing runtime binding lookup with direct slot access. | Adds a separate compiled binding layout pass behind `CompiledScript`. The pass walks the parsed AST after parsing, builds lexical/function scope metadata, assigns checked `GlobalSlot`, `LocalSlot`, and `UpvalueSlot` operands to `StaticBindingId` occurrences, keeps unresolved external names explicit, and exposes slot/unresolved counts through `CompiledScriptUsage`. `eval_compiled` now sizes the static binding cache from the compiled layout rather than from parser usage counters, while runtime reads and writes intentionally keep the existing guarded lookup semantics until the next direct-slot tranche. Direct coverage verifies global/local/upvalue counts for closures, shadowed lexical slots, and unresolved bindings. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, targeted binding-layout tests, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T034122Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal confirms this is a layout foundation rather than the direct-slot speedup: 40 latency rows and 23 memory rows remain over budget, `function_parameters_scope` and `closure_environments` are within budget, `compiled_script_reuse` is a latency exception at `1.19x`, and `atomized_bindings` remains a latency and memory exception at `1.12x` and `1.10x`. The follow-up is to use this layout to create local/global/upvalue frames directly, then retire ordinary runtime shadowing scans from hot binding access. |
| [x] | Done | Binding layout cache seeding tranche | Runtime architecture / performance | Use compiled binding operands to widen the existing safe runtime binding cache without assuming layout slot order matches runtime insertion order. | `Context` now carries the active `BindingLayout` alongside compiled static-name and binding caches, escaped functions capture the same layout, and runtime binding resolution seeds every `StaticBindingId` occurrence with the same compiled operand once the actual runtime `BindingLocation` is known. Function parameter bindings seed inside the function's own cache/layout context, and lexical `for...in` bindings seed after their per-iteration scope is active. This keeps existing guarded lookup validation and fallback semantics while reducing repeated same-operand cache misses. Direct coverage verifies repeated compiled `for...in` lexical bindings and cross-eval function parameter/name atom accounting. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, targeted binding/static-name/for-in tests, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T035848Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains runtime-model debt: 40 latency rows and 23 memory rows remain over budget, `compiled_script_reuse` is `1.21x`, and `atomized_bindings` is `1.19x` latency plus `1.13x` memory, so direct runtime frames and slot operands remain the next binding-path step. |
| [x] | Done | Function parameter frame slots tranche | Runtime architecture / performance | Start building runtime binding frames from compiled local-slot layout instead of relying only on insertion order. | `BindingLayout` now exposes checked binding operands for runtime frame construction, `BindingScope` can insert or replace a cell at an explicit checked `BindingSlot`, and ordinary function call scopes populate parameters at their compiled `LocalSlot` offsets when layout metadata is available. Static binding cache entries now distinguish guarded locations from layout-backed exact local/upvalue locations, allowing local/upvalue cache hits to skip shadowing scans while keeping global hits guarded. An initial broader exact-global attempt reduced full Test262 by six Annex B language cases, so this tranche deliberately leaves global cache validation conservative until global frame construction models those cases explicitly. Direct coverage verifies compiled function parameter frame slots with shadowing and repeated compiled eval atom accounting. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, targeted binding/static-name/embedding tests, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T041631Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal is mixed but directionally useful for binding-heavy rows: `function_parameters_scope` and `closure_environments` remain within budget at `0.91x`, `atomized_bindings` is a latency exception at `1.14x`, and `compiled_script_reuse` remains a latency exception at `1.21x`. |
| [x] | Done | Lexical declaration frame slots tranche | Runtime architecture / performance | Extend compiled local-slot frame construction from function parameters to runtime lexical declaration paths. | `BindingScope` now has a best-effort optional-slot insertion path for compiled local slots, and `Context` exposes checked `StaticBinding` to `LocalSlot` lookup from the active `BindingLayout`. Ordinary `let` and `const` declarations, lexical `for...in` bindings, and catch parameter scopes try their compiled `LocalSlot` first, then fall back to ordinary runtime insertion when the current scope does not match the compiled frame shape. Strict slot insertion remains reserved for function parameters, where frame construction must match the layout. Direct coverage verifies function body locals after parameter slots, lexical `for...in` reuse, and catch/body lexical slot accounting across repeated compiled evaluation. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, targeted binding/block/for-in/control-flow/embedding tests, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T042711Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark debt remains runtime-model work: 40 latency and 23 memory rows are tracked exceptions, `compiled_script_reuse` is `1.22x`, and `atomized_bindings` is `1.17x`, so hoisted vars, globals, closure cells, and direct slot operands remain future work. |
| [x] | Done | Hoisted var frame slots tranche | Runtime architecture / performance | Extend compiled local-slot frame construction from lexical declarations to function-local hoisted `var` declarations. | `hoist_var` now tries the active compiled `LocalSlot` when hoisting into a local function frame, using the same optional-slot fallback path as lexical declarations. Global `var` declarations deliberately keep ordinary insertion because the current global `BindingScope` also contains built-ins, while `GlobalSlot` metadata only describes script declarations. Direct coverage verifies a function parameter, two hoisted `var` declarations including one from a nested block, and a block lexical declaration sharing the compiled local-slot layout across repeated compiled evaluation. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, targeted binding/block/language/embedding/static-name tests, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T043536Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark debt remains tracked: 39 latency and 24 memory rows are exceptions, `compiled_script_reuse` is `1.19x`, and `atomized_bindings` is `1.17x` latency plus `1.11x` memory, so global frames, closure cells, and direct slot operands remain future work. |
| [x] | Done | Static object literal borrowed keys tranche | Runtime architecture / memory | Remove an owned-string allocation from the static object literal creation path while preserving atom-backed property storage. | `ObjectHeap::create` now accepts `ObjectPropertyInit<'_>` so parser-known object literal property names are passed as borrowed `&str` values while ordinary object storage still uses atom-backed `PropertyKey` and shape metadata. Direct embedding coverage verifies repeated compiled object literal evaluation, reused property atoms, and `__proto__` literal prototype behavior. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, targeted embedding smoke coverage, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T044715Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, full Test262 at 10872/102578, and QuickJS differential at 64/64. Benchmarks measured 63 rows with 42 latency and 21 memory exceptions; `object_literals` is within latency budget at `0.90x` but remains a memory exception at `1.11x`, so remaining string work belongs to dynamic string keys, `StringId`, compact values, and bytecode/property operands. |
| [ ] | Backlog | Compiler-assigned slots and upvalues | Runtime architecture / performance | Replace repeated runtime name lookup for locals, globals, and captured variables with checked slot indices. | The compiled `BindingLayout` now exists, seeds matching runtime binding-cache occurrences after a safe runtime location is known, populates function parameter frames at compiled local slots, best-effort places lexical declaration paths at compiled local slots, and best-effort hoists function-local `var` declarations into compiled local slots. The next step is using that metadata for global frames and closure cells, then making reads, writes, compound assignments, updates, and constructor lookups use checked slot operands instead of runtime name resolution. Keep best-effort fallback for dynamic or mismatched runtime scopes until direct slot operands have their own validation model. |
| [x] | Done | Prototype chain traversal metadata tranche | Runtime architecture / performance | Centralize prototype traversal before shared shapes and inline caches. | Moves prototype-chain lookup, `in`, `__proto__`, and cycle checks into `runtime_object_prototype.rs`, replacing per-lookup visited-vector allocation with a checked traversal budget based on VM object count. Adds a deep prototype-chain smoke test while preserving existing cycle rejection behavior. This is a foundation for later prototype-version guards and `ShapeId` caches, not the full shape implementation; the `Shape-based object and prototype layout` backlog remains open. Validation passed with `cargo fmt --all -- --check`, targeted object/prototype/`in`/`for...in` tests, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T003559Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains tracked runtime-model debt: `object_prototypes` is still over latency and memory budget, and `object_prototype_root`, `object_builtin`, `for_in_statements`, and `in_operator` still need shared shapes, prototype versioning, compiler-assigned property operands, and inline caches. |
| [x] | Done | Shape id layout metadata tranche | Runtime architecture / performance | Give ordinary objects stable VM-local shape identity before property offset caches and prototype guards. | Adds `ShapeId` and a VM-owned `ShapeTable`, records shape transitions on ordinary named-property add/remove paths including descriptors, sparse array fallback, data objects, string wrappers, and built-in prototypes, and exposes `shape_count` in `VmResourceUsage` for embedding-side resource snapshots. Direct embedding coverage verifies that matching object property layouts reuse shapes and that adding a property creates a new layout. Validation passed with `cargo fmt --all -- --check`, targeted embedding/object/array tests, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T005608Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. This is still metadata-only: shared slot offsets, prototype-version guards, descriptor-aware shape compatibility, and inline caches remain follow-up work. |
| [x] | Done | Shape property offset tranche | Runtime architecture / performance | Move ordinary object named-property lookup from per-object indexes to shared shape-owned offsets. | Replaces the remaining per-object `PropertyIndexEntry` with `ShapePropertyOffset` metadata owned by `ShapeTable`, so property read, write, descriptor, sparse-array fallback, prototype, `in`, and enumeration paths resolve named-property slots through the object's `ShapeId`. Direct embedding coverage now verifies shared layout reuse after deleting a middle property, which exercises offset compaction for the remaining properties. Validation passed with `cargo fmt --all`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T010724Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains tracked runtime-model debt: object/prototype paths still need prototype-version guards, descriptor-aware shapes, compiler-assigned property operands, and inline caches. |
| [x] | Done | Prototype lookup version tranche | Runtime architecture / performance | Add an explicit VM-local guard for future prototype-chain inline caches. | Adds a checked `PrototypeLookupVersion` to `ObjectHeap`, exposes it in `VmResourceUsage`, and bumps it when object structure or `__proto__` changes through central object, descriptor, and array-index mutation paths. Direct embedding coverage verifies that prototype assignment, property add, and property delete advance the version while value-only writes keep it stable. Validation passed with `cargo fmt --all`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T011917Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. This is a coarse safe invalidation guard; later inline caches can refine it with descriptor-aware shapes and per-site cache records. |
| [x] | Done | Descriptor-aware shape layout tranche | Runtime architecture / performance | Make ordinary object shapes distinguish property descriptor attributes, not only key order. | Shapes now store per-slot data-property attributes together with insertion-order keys, reuse layouts only when writable/enumerable/configurable metadata matches, and transition existing object shapes when `Object.defineProperty` or enumerable updates change attributes. Direct descriptor coverage verifies same-attribute layout reuse, different-attribute layout separation, and descriptor update back to an existing compatible shape. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T013421Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. This prepares cacheable lookup APIs and inline caches while preserving current descriptor semantics. |
| [x] | Done | Cacheable property lookup API tranche | Runtime architecture / performance | Give future inline caches a checked shape/prototype lookup snapshot for named object and prototype properties. | Adds `runtime_object_lookup.rs` with `PropertyLookupGuard`, cacheable hit/miss/uncacheable results, receiver `ShapeId`, owner `ShapeId`, slot offset, prototype depth, and VM-local prototype lookup version validation. Object/prototype `get` and `in` paths now try this cacheable named-property path first and fall back to the generic path for array `length`, dense array indices, non-atomized property lookups, and stale guards. Direct smoke coverage verifies own and prototype named reads, `in`, structure-version changes after shadow/delete, and array-specific fallback semantics. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T014403Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark debt remains for object/prototype/descriptor paths because this branch adds the safe cache boundary, not bytecode or per-site inline cache storage. |
| [ ] | Backlog | Shape-based object and prototype layout | Runtime architecture / performance | Move ordinary objects and prototypes toward shared shapes plus slot storage instead of per-object string-keyed maps. | `ShapeId`, `ShapeTable`, add/remove transitions, shared property offsets, prototype lookup versioning, descriptor-aware shape compatibility, and cacheable named-property lookup snapshots now exist. Next add compiler-assigned property operands, object/prototype-heavy built-in migration coverage, and eventual per-site inline caches. |
| [x] | Done | Array storage foundation tranche | Runtime architecture / performance | Move array element state into a dedicated storage layer before adding method-specific fast paths. | Adds `ArrayStorage` with packed and holey dense element variants plus sparse index-key tracking, routes ordinary set/delete, descriptor, lookup, and enumeration paths through it, and adds smoke coverage for packed-to-holey transitions. This is a structural prerequisite, not the final fast path: `concat`, `slice`, `includes`, `indexOf`, `lastIndexOf`, `join`, `reverse`, `shift`, and `unshift` still need guarded specialized implementations. |
| [x] | Done | Packed array read/copy fast paths | Runtime architecture / performance | Use the new array storage boundary for guarded packed-array operations before broader array rewrites. | Adds packed full-length guards for `includes`, `indexOf`, `lastIndexOf`, `join`, `slice`, and `concat`, while leaving holey, sparse, descriptor-sensitive, and prototype-index cases on the generic path. Validation passed with `cargo fmt --all -- --check`, targeted array smoke tests, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T001352Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, QuickJS differential at 64/64, and full Test262 at 10872/102578. Benchmark signal remains tracked debt: packed read/copy paths reduce some compiled-eval work, but array methods still need deeper storage and mutation fast paths to reach the 1.10x budget. |
| [x] | Done | Packed array reverse fast path | Runtime architecture / performance | Use `ArrayStorage` for guarded in-place reverse on simple packed arrays. | Adds a full-length packed reverse path for arrays whose dense elements all have default writable, enumerable, and configurable data attributes. Holey arrays, sparse indices, prototype-index behavior, and descriptor-modified dense elements stay on the generic reverse path. Direct smoke coverage verifies the descriptor fallback by reversing an array with a non-writable dense element. Validation passed with `cargo fmt --all -- --check`, `cargo check --all-targets`, targeted reverse smoke coverage, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260706T045624Z.md` keeps engine fixtures at 68/68, active Test262 at 66/66, full Test262 at 10872/102578, and QuickJS differential at 64/64. Benchmarks measured 63 rows with 39 latency and 24 memory exceptions; `array_prototype_reverse` remains a tracked exception at `1.68x` latency and `1.10x` memory, so shift/unshift, deeper array storage, and prototype-index guards remain future work. |
| [ ] | Backlog | Dense array fast paths | Runtime architecture / performance | Add guarded packed, holey, and sparse array fast paths now that storage has a separate owner. | Target the measured array debt: `concat`, `slice`, `includes`, `indexOf`, `lastIndexOf`, `join`, `reverse`, `shift`, and `unshift`. Preserve fallback semantics when holes, sparse indices, descriptors, or prototype index properties can affect reads. |
| [ ] | Backlog | Built-in intrinsic metadata and native-call fast paths | Runtime architecture / performance | Reduce repeated construction, lookup, and call overhead for built-in constructors, prototypes, and native functions. | Share immutable intrinsic metadata, lazily materialize mutable JS objects per VM, pre-resolve atoms and shapes, and add direct call paths for common built-ins without mutable global JavaScript state. |
| [ ] | Backlog | VM-owned heap accounting and GC | Resource control / runtime architecture | Define mark/sweep or reference-counting plus cycle collection over indexed VM heaps. | Must preserve deterministic teardown, hard heap limits, host callback handles, queued jobs, and VM isolation. |
| [ ] | Backlog | Bytecode VM and quickening | Runtime architecture / performance | Replace direct AST evaluation on hot paths with compact bytecode behind the `CompiledScript` API, then specialize hot instructions safely. | Start with generic bytecode. After profiles justify it, quicken operations such as numeric add, property load, property store, and native built-in calls with checked fallback to generic instructions. |
| [ ] | Backlog | Inline caches | Runtime architecture / performance | Cache stable property, call, and `in` access paths after shapes and bytecode sites exist. | Cache shape id, slot offset, and prototype version where needed. Keep fallback paths correct, invalidation explicit, and cache storage checked through newtype indices. |
| [ ] | Backlog | Compact values and heap strings | Runtime architecture / memory | Reduce scattered allocations and cloned strings without introducing unsafe allocator work first. | Add `StringId`, compact handles, Vec-backed heaps, free lists, boxed immutable constants, and memory accounting. Defer NaN boxing unless safer structural changes are exhausted. |
| [ ] | Backlog | Parallel VM execution and compiled-script sharing | Embedding API / runtime architecture | Document and implement the realistic parallelism model for embedders. | Do not try to parallelize one arbitrary JavaScript context. Support independent VM execution, parallel compilation of different scripts, immutable compiled-script sharing, context pooling, async host callbacks through the job queue, and later background cleanup where safe. |
| [ ] | Backlog | Performance and memory checkpoints | Performance and memory | Bring tracked benchmark exceptions back within the `1.10x` latency and memory budget where measurements are stable. | These are recurring checkpoint tasks driven by reports, not the whole project direction. Preserve semantics with engine tests and QuickJS differential cases. |

## Default Project Sequence

The order below is the default project direction. It can change when the latest
report shows a clearer bottleneck, compatibility gap, or embedding API need,
but branches should document why they changed priority.

1. Keep the repository and reports trustworthy.
   Guardrails, CI, test reports, QuickJS setup, Test262 setup, and benchmark
   reporting are part of the product. Do not let them drift.

2. Stabilize the embedding API.
   Keep direct library tests ahead of CLI-only coverage. The API must support
   many VMs per process, isolated state, resource failures, deterministic
   teardown, host output, host functions, and reusable compiled scripts.

3. Expand compatibility in coherent tranches.
   Use Test262 feature maps to pick parser, runtime, built-in, and error-model
   clusters. Each branch should close a visible area and add engine plus QuickJS
   differential coverage where possible.

4. Grow practical built-ins.
   Prioritize `Object`, `Array`, `String`, `Number`, `Math`, `Boolean`,
   `Function`, standard errors, JSON, Date, RegExp, Map, and Set based on
   reports and embedding needs. Add benchmarks for hot paths as they land.

5. Keep host extension support close to real embedding use.
   Improve value conversion, host callback errors, VM-local callback ownership,
   and examples before adding advanced async behavior.

6. Add modules, promises, and async integration.
   Design module loading around embedder-owned I/O and policy. Add the
   JavaScript job queue before promises, async functions, and async Rust host
   callbacks.

7. Expand resource control.
   Move from source and runtime counters toward heap, stack, atom, job, module,
   host callback, and cancellation limits. Every limit must be visible through
   the library API and teardown reports.

8. Add observability as product surface.
   Structured events, profiling hooks, resource snapshots, and execution reports
   should be designed for production embedders, not only for local debugging.

9. Improve runtime data structures when they unblock features, resource
   accounting, observability, or measured debt.
   Atoms, slot-based locals, shape-based objects, dense arrays, and indexed
   heaps are architecture tasks. They should support compatibility and resource
   control while keeping QuickJS-like speed and footprint.

10. Add bytecode after the source language is broad enough.
    Bytecode should preserve the `CompiledScript` API and have interpreter
    fallback tests as an oracle. Inline caches belong after shapes and bytecode.

11. Add explicit VM heap accounting and GC.
    The indexed heap model should grow into deterministic accounting and a safe
    collection strategy compatible with host callbacks, promises, queued jobs,
    and many isolated VMs.

12. Run performance and memory checkpoint tasks continuously.
    Benchmark exceptions should be handled as recurring checkpoint tasks, but
    they do not define the whole roadmap. When a feature makes a hot path
    slower, either fix it in the same branch or record a measured exception
    with a follow-up task.

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

Recent benchmark evidence suggests that parsing and compile are not the main
cost for many heavy implemented cases anymore. Do not treat bytecode as only a
parse-cache feature. The value of bytecode is a more compact dispatch model,
explicit resource accounting, compiler-assigned operands, and a place to attach
inline caches and quickened instructions.

### Slot-Based Locals

String-keyed binding maps are simple but expensive. A compiler pass should
assign local, global, and upvalue slots before execution. The runtime should
then read and write `Vec<Value>` entries through checked newtype indices. This
also gives the engine a natural place to account for stack and closure memory.

The current runtime already moved scope storage in that direction: bindings are
stored in a `Vec` and found through an atom-to-slot map. Compiled scripts also
carry a `BindingLayout` that assigns stable `LocalSlot`, `GlobalSlot`, and
`UpvalueSlot` operands, can seed matching occurrence caches after runtime
locations are known, and can populate function parameter frames at compiled
local-slot offsets. Runtime lexical declarations, lexical `for...in` bindings,
catch parameter scopes, and function-local hoisted `var` declarations now also
try compiled local-slot offsets with a fallback to ordinary insertion when the
active runtime scope does not match the compiled frame shape. That is still not
the final design. The next slot branch should extend layout-owned frame
construction to globals and closure cells, then leave name lookup for dynamic
or fallback paths only. Keep global cache hits guarded until the runtime has an
explicit global-frame model for Annex B shadowing behavior.

### Atom Interner

Identifiers and property keys should use `AtomId` values instead of repeated
owned strings on hot paths. The atom table belongs to the engine or VM boundary
and must not create mutable global state. Atoms should be usable by parser,
compiled scripts, object shapes, function metadata, and diagnostics.

The current first step adds a VM-owned atom table and stores lexical/global
binding keys as `AtomId` values. Public APIs still accept string names, and
missing binding lookups do not create atoms. Property keys, function metadata,
string constants, compile-time atomization, and a hard atom-table budget remain
future work.

Full atomization means runtime object, function, built-in, and prototype paths
should not repeatedly allocate, clone, or compare owned `String` values. String
APIs can still accept `&str`; the hot representation should be `AtomId`.

### Shapes And Inline Caches

Shapes describe object layouts. Object instances store values in slots, and
shape transitions describe property additions or layout changes. Once shapes
exist, property access sites can cache `(ShapeId, offset)` and fall back to the
generic lookup path when the shape does not match.

Shapes must cover descriptor attributes and prototype behavior, not only plain
`obj.foo` reads. Prototype chains need an explicit version or equivalent guard
so cached prototype hits are invalidated when the chain changes. Inline caches
should start as interpreter or bytecode data structures, not as JIT code.

### Dense Arrays

Array storage should distinguish packed arrays, holey arrays, and sparse
objects. Packed storage is the default fast path. Holey storage preserves
JavaScript holes without forcing every array into dictionary mode. Sparse
storage remains the fallback for large or unusual indices.

Array fast paths should be guarded by layout and prototype facts. Packed arrays
can make `includes`, `indexOf`, `lastIndexOf`, `join`, `reverse`, `slice`, and
`concat` mostly linear `Vec` work. Holey arrays need explicit missing-element
semantics. Sparse arrays and arrays affected by prototype index properties must
fall back to the generic object/property path. `shift` and `unshift` should not
force a full element move on every call once a better storage model exists.

### Built-In Intrinsics And Native Calls

Built-in constructors, prototypes, and native functions should not require
repeated string lookup and object construction on hot paths. Shared immutable
metadata can describe intrinsic names, property attributes, native function
kinds, default prototypes, and method tables. Each VM still owns its mutable JS
objects and descriptors. Direct native-call paths are allowed when they preserve
observable semantics and fall back when user code shadows or redefines the
built-in.

### Bytecode Quickening

Quickening is a safe-Rust alternative to JIT specialization. The engine can
start with generic bytecode operations such as `Add`, `LoadProp`, `StoreProp`,
and `Call`. After a successful generic execution, an instruction can be
rewritten or annotated as `AddNumber`, `LoadPropCached`, or `CallNativeBuiltin`
with checked guards. If the guard fails, execution falls back to the generic
operation and updates or clears the cache.

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

The first memory-layout goal is structural: reduce scattered allocations and
large clones through compact handles, heap strings, boxed immutable constants,
shape sharing, and captured-variable cells only where closures need them. A
custom global allocator or NaN boxing should not be the first move; both add
complexity and are less likely to beat the gains from slots, atoms, shapes, and
dense arrays.

### Garbage Collection

The current indexed handle direction is compatible with a safe collector. A GC
design should start from explicit VM roots, stack slots, globals, closures,
objects, arrays, promises, and host callback handles. It must report memory
usage and reclaim all VM-owned state during teardown.

### Parallelism

One arbitrary JavaScript context should not be the main parallelism target.
Side effects, prototypes, getters, exceptions, and host callbacks make that
model fragile. The product should instead support parallel compilation of
different scripts, parallel execution of independent VMs, immutable
compiled-script sharing, context pooling, async host callbacks through the job
queue, and eventually background cleanup or collection when the root model is
explicit.
