# Project Roadmap And Execution Plan

This document is the canonical product roadmap and execution plan for growing
`rs-quickjs` into a safe-Rust, embeddable JavaScript engine. It describes what
we are building, the rough order of work, and the protocol every branch should
follow.

The plan is intentionally operational. Each repository, embedding API,
compatibility, built-in, async, testing, resource-control, observability,
runtime-architecture, performance, or memory task should update this document
in the same branch that implements the task. Future work should resume from
repository state instead of relying on conversation history.

This is a whole-project roadmap, not an optimization backlog. Read it as a
delivery order for the engine: keep the validation base reliable, keep the Rust
library API useful for embedders, expand language compatibility, add practical
built-ins, design modules and async jobs, expand resource control, add
production observability, and evolve runtime internals behind stable public
interfaces. Performance and memory budgets are acceptance criteria for all of
that work, with dedicated checkpoint tasks only when measurements show debt.

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

## Near-Term Product Order

When there is no stronger signal from the latest report, pick the next task in
this order:

1. Preserve repository trust.
   CI, one-command validation, Test262 setup, QuickJS setup, compact reports,
   and benchmark reporting must keep working before feature work expands.

2. Keep the library surface usable.
   Direct Rust API coverage should stay ahead of CLI-only behavior for isolated
   VMs, resource limits, typed host functions, output separation, teardown, and
   reusable compiled scripts.

3. Expand compatibility by visible Test262 clusters.
   Parser, runtime semantics, object semantics, functions, errors, and built-ins
   should land in narrow branches that improve measured corpus progress.

4. Add practical built-ins by usage and report evidence.
   `Object`, `Array`, `String`, `Number`, `Math`, `Boolean`, `Function`, errors,
   JSON, Date, RegExp, Map, and Set should be prioritized by missing-binding
   counts, feature-area failures, and embedding needs.

5. Improve diagnostics before errors become API debt.
   Syntax, runtime, host callback, and resource-limit errors should become
   stable enough for embedders to log, classify, and act on.

6. Add modules, jobs, promises, and async host callbacks.
   These features should build on the synchronous embedding surface and keep the
   embedder in control of I/O policy and the outer executor.

7. Expand resource control and observability.
   Heap, stack, atom, job, module, host callback, cancellation, event, profiling,
   and teardown data should become first-class library behavior.

8. Change runtime data structures when they unblock product work or measured
   debt.
   Atoms, slots, shapes, dense arrays, indexed heaps, bytecode, inline caches,
   and GC are architecture work in service of compatibility, embedding,
   observability, resource control, performance, and memory footprint.

Performance and memory checkpoint tasks can preempt this order only when the
latest measurements show a regression or budget exception that blocks the next
feature tranche, embedding API promise, or device-footprint target.

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

The major implementation directions are:

- `CompiledScript` before bytecode
- atom ids for identifiers and property keys
- slot-based locals and upvalues
- shape-based object layouts
- inline property caches after shapes exist
- dense array fast paths
- VM-owned indexed heaps instead of scattered small allocations
- explicit heap accounting and a safe collection strategy

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
| [x] | Done | Engine case registry split | Testing / maintenance | Keep the engine fixture registry below the project file-size limit before adding more compatibility tranches. | Moves runtime/error/built-in engine fixture registration out of the central `cases.rs` file into `cases_engine_runtime.rs`, reducing `cases.rs` from 789 to 745 lines so future built-in cases can be added without pushing the main registry over 800 lines. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T184709Z.md` keeps engine fixtures at 57/57, active Test262 at 57/57, full Test262 at 9782/102578, and QuickJS differential at 55/55. Benchmark counts are behavior-equivalent but show normal measurement noise with latency exceptions at 26 and memory exceptions at 2. |
| [ ] | Backlog | Report triage cadence | Testing / planning | Keep the next work item grounded in the latest Test262, QuickJS differential, benchmark, and memory evidence. | Before selecting each compatibility or architecture tranche, summarize the newest report signals and record why the chosen task is next. |
| [ ] | Backlog | Library API documentation pass | Embedding API / documentation | Keep crate docs, README examples, and direct library tests aligned with the current public API. | Do this whenever API shape changes enough that embedders could be confused by stale examples. |
| [x] | Done | Parser and syntax compatibility tranches | Compatibility | Continue reducing Test262 parser and lexer failure clusters. | Adds the `no-substitution-template-literal` cluster: backtick string literals without `${...}` interpolation, including common escapes and line terminator normalization. Template substitutions now fail with an explicit unsupported-feature lexer error. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, upstream Test262 manifest row, and QuickJS differential coverage. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T172449Z.md` raises full Test262 passes from 9609 to 9616, raises `language/expressions` passes from 4153 to 4157, raises active Test262 to 55/55, raises QuickJS differential to 53/53, and lowers the top `lexer: unexpected character` hint from 16369 to 6531. Some cases now advance from lexer failures into parser/runtime failure classes, which is expected until interpolation, tagged templates, BigInt, and related syntax are implemented. No new benchmark case was added because this is parser-only literal support; existing benchmark exceptions remain tracked in the report. |
| [ ] | Backlog | Runtime semantics tranches | Compatibility | Expand statements, functions, lexical environments, `this`, exceptions, equality, iteration, and prototype behavior. | Each tranche should improve a visible Test262 feature area without mixing unrelated semantics. |
| [x] | Done | Practical built-ins tranches | Compatibility | Expand `Object`, `Array`, `String`, `Number`, `Math`, `Boolean`, `Function`, errors, JSON, Date, RegExp, Map, Set, and other high-value built-ins. | Adds the basic `String` built-in: callable `String(value)` conversion, constructable `new String(value)` wrapper objects with `String.prototype`, non-enumerable `length`, enumerable character indices, primitive string `length`/index lookup, `in`, and `for...in` support. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T173929Z.md` raises full Test262 passes from 9616 to 9722, raises `built-ins/String` passes from 4 to 62, raises active Test262 to 56/56, raises QuickJS differential to 54/54, and removes `String` from the top missing bindings table. The new `string_builtin` benchmark is tracked as a latency exception at `1.50x` while staying within memory budget at `1.07x`; full String prototype methods, wrapper internal primitive semantics, abstract equality/object-to-primitive coercion, and string Unicode code-unit accuracy remain follow-up work. |
| [x] | Done | Boolean built-in tranche | Compatibility | Promote `Boolean` from a special call-path into a normal built-in binding and constructor. | Adds `Boolean` as a normal native built-in binding with call and constructor paths, `Boolean.prototype.constructor`, function `name`/`length`/`prototype` metadata, and shadowing-correct call behavior by removing the old direct-call special case. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T183517Z.md` raises full Test262 passes from 9722 to 9782, raises active Test262 to 57/57, raises QuickJS differential to 55/55, and removes `Boolean` from the top missing bindings table. The new `boolean_builtin` benchmark is tracked as a latency exception at `1.47x` while staying within memory budget at `0.94x`; Boolean wrapper internal primitive semantics and `Boolean.prototype.valueOf`/`toString` remain follow-up work. |
| [x] | Done | Global numeric constants tranche | Compatibility | Add standard global numeric value properties that remove the remaining `NaN` missing-binding cluster. | Adds immutable lazy global bindings for `NaN` and `Infinity`, materializes lazy built-ins before assignment, compound assignment, and delete operations, and adds Rust smoke coverage plus engine, active Test262, upstream Test262 manifest, QuickJS differential, and benchmark fixtures. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T190135Z.md` raises full Test262 passes from 9782 to 10003, raises engine fixtures to 60/60, raises active Test262 to 58/58, raises QuickJS differential to 56/56, raises `built-ins/Number` passes from 32 to 116, raises `built-ins/String` passes from 62 to 76, and removes `NaN` from the top missing bindings table. The new `global_numeric_constants` benchmark is tracked as a latency exception at `1.22x` while staying within memory budget at `0.91x`; full global object property descriptors remain follow-up work. |
| [x] | Done | Math built-in tranche | Compatibility | Add the first standard `Math` object surface and reduce the `missing binding: Math` cluster. | Adds the `Math` object, standard numeric constants, and `abs`, `ceil`, `floor`, `max`, `min`, `pow`, `round`, `sqrt`, and `trunc`, with signed-zero handling for `max`, `min`, and `round`. Adds direct Rust smoke coverage, engine fixture, active Test262 fixture, six upstream Test262 manifest rows, QuickJS differential coverage, and a benchmark. Validation passed with `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features`, `cargo test`, and `scripts/test-all.sh`; report `rsqjs-test-report-20260705T192241Z.md` raises engine fixtures to 61/61, active Test262 to 59/59, QuickJS differential to 57/57, full Test262 passes from 10003 to 10193, and `built-ins/Math` to 154/654 passed with `Math` removed from top missing bindings. The new `math_builtin` benchmark is tracked as a latency and memory exception at `1.59x` and `1.24x`; full property descriptor semantics, remaining Math methods, and faster native-call/property paths remain follow-up work. |
| [ ] | Backlog | Diagnostics and error model | Compatibility / embedding API | Make syntax, runtime, host callback, and resource-limit errors precise and stable enough for embedders. | Preserve error chains and report enough location/context data for production logging. |
| [ ] | Backlog | Module loading design | Compatibility / embedding API | Design how modules are resolved, loaded, cached, limited, and observed by embedding applications. | Keep the embedder in control of I/O and policy; document what is intentionally unsupported in the first implementation. |
| [ ] | Backlog | Promise job queue | Compatibility / embedding API | Add the JavaScript job model required by promises and async functions. | The VM owns JavaScript jobs; the embedding application controls when jobs are drained. |
| [ ] | Backlog | Async host callbacks | Embedding API | Allow Rust host callbacks to complete asynchronously through embedder-owned executors. | Should build on the promise job queue and preserve VM isolation, cancellation, and resource accounting. |
| [ ] | Backlog | Resource limit expansion | Resource control | Extend limits from source and runtime counters toward heap, stack, atom table, jobs, host callbacks, modules, and cancellation. | Every new limit needs library tests, error reporting, and teardown accounting. |
| [ ] | Backlog | Observability hooks | Observability | Add structured execution events, profiling hooks, resource usage snapshots, and teardown reports. | Useful for production embedding, debugging, and future performance work. |
| [ ] | Backlog | Slot-based local bindings | Runtime architecture / performance | Replace repeated name lookups for local variables with compiler-assigned local, global, and upvalue slots. | Requires scope analysis, closure/upvalue model, and migration tests for lexical bindings. |
| [ ] | Backlog | Shape-based object layout | Runtime architecture / performance | Move ordinary objects toward shape plus slot storage instead of per-object key maps for stable layouts. | This supports compatibility and performance for object/prototype-heavy built-ins. |
| [ ] | Backlog | Dense array fast paths | Runtime architecture / performance | Split array storage into packed, holey, and sparse representations. | Most array-heavy JavaScript needs packed or holey arrays to stay close to QuickJS. |
| [ ] | Backlog | VM-owned heap accounting and GC | Resource control / runtime architecture | Define mark/sweep or reference-counting plus cycle collection over indexed VM heaps. | Must preserve deterministic teardown, hard heap limits, host callback handles, queued jobs, and VM isolation. |
| [ ] | Backlog | Bytecode VM | Runtime architecture / performance | Replace direct AST evaluation on hot paths with compact bytecode behind the `CompiledScript` API. | Start only after enough language coverage exists to benchmark honestly. |
| [ ] | Backlog | Inline caches | Runtime architecture / performance | Cache stable property and call access paths after shapes and bytecode exist. | Keep fallback paths correct and keep all cache invalidation explicit. |
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

The current first step adds a VM-owned atom table and stores lexical/global
binding keys as `AtomId` values. Public APIs still accept string names, and
missing binding lookups do not create atoms. Property keys, function metadata,
string constants, compile-time atomization, and a hard atom-table budget remain
future work.

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
