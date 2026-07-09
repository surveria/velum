# Architecture Stabilization And Development Strategy

This document is the focused execution plan for correcting the engine
architecture and the development methodology used to grow it. It turns the
current architecture review into repository-owned work that can be completed
through small pull requests with explicit evidence.

The plan does not replace [Project Development Plan](project-plan.md). The
project plan remains the whole-product roadmap and task history. This document
defines architecture prerequisites, development gates, and the order in which
cross-cutting runtime foundations should be stabilized. When a product task is
affected by one of these gates, this plan determines the required architecture
work before the product task may claim completion.

`AGENTS.md` remains the mandatory workflow and engineering policy. Every task
under this plan uses a separate worktree and branch, opens a draft pull request
early, keeps progress visible in reviewable commits, follows the release-only
version policy, and uses the validation lane appropriate to the change.

## Plan Status

- Plan version: 1
- Initial review date: 2026-07-10
- Adoption task: PR #396
- Review baseline: `origin/main` at `f0e4666`
- Test baseline: 34,002 of 102,578 full Test262 variants passed in
  `reports/test-runs/rsqjs-test-report-20260709T213555Z.md`
- Current program state: AS-01 in progress; AS-01a is complete and AS-01b is
  implemented in PR #399 pending required CI and merge

The baseline is historical evidence, not a value to keep editing after every
merge. Current task selection must always use the newest trusted report.

## Executive Decision

The project will not follow either extreme development strategy:

1. It will not continuously add workload-specific optimizations while core
   semantics are still changing.
2. It will not ignore architecture and performance until an arbitrary full
   Test262 percentage is reached.

Instead, the project will use a staged strategy:

1. stabilize the semantic object model, abstract operations, completion model,
   VM ownership, execution frames, and root/accounting contracts;
2. continue compatibility work where it does not deepen a known architecture
   problem;
3. expand compatibility aggressively after the relevant architecture gate is
   complete;
4. treat performance as a continuous regression budget and use dedicated
   optimization work only for measured cross-cutting bottlenecks;
5. postpone representation-level experiments until semantic and ownership
   boundaries are stable.

The current bytecode-first outer architecture is worth preserving. This is a
stabilization and migration program, not a rewrite.

## Current Strengths To Preserve

- The parser AST is compile-time frontend data and is not a runtime execution
  fallback.
- `CompiledScript` hides bytecode internals behind the embedding API.
- Bytecode and binding metadata are AST-free runtime-owned representations.
- Engine state is intended to be VM-local and safe Rust remains the default.
- Typed ids, binding slots, atoms, shapes, array storage, and inline-cache
  guards provide useful implementation foundations.
- Test262, QuickJS differential tests, focused engine tests, and benchmark
  reports provide a strong evidence loop.
- Correctness and performance validation have separate lanes, so feature work
  does not need to run unstable timing workloads before every merge.

These foundations should be migrated forward rather than discarded.

## Problems This Plan Corrects

### Split JavaScript Object Semantics

The current `Value` representation gives ordinary objects, JavaScript
functions, native functions, host functions, and error values different value
variants. Property access, property definition, callability, construction, and
prototype behavior therefore require parallel dispatch paths. New exotic
objects also add optional fields or side tables in several runtime owners.

This increases the cost of every new built-in and makes it easy for separate
agents to implement subtly different object semantics.

### Scattered ECMAScript Abstract Operations

Operations such as `ToPropertyKey`, `ToLength`, `SameValueZero`, `Get`, `Set`,
`Call`, `Construct`, iterator access, and exception creation are distributed
between bytecode execution, object helpers, and individual built-ins. A new
built-in can therefore reproduce an existing operation instead of reusing one
semantic implementation.

### Non-Resumable Execution State

The bytecode VM owns a program counter and operand stack, but structured
instructions contain nested bytecode blocks and block evaluation creates local
execution state. This is sufficient for synchronous execution, but it is not a
durable continuation model for generators, pending `await`, top-level await,
debugging, or cooperative interruption.

### Incomplete Heap Ownership And Weak Semantics

Objects, functions, strings, promises, collections, iterators, and callbacks
use several indexed stores and shared allocations. The engine reports selected
counts, but it does not yet have one root enumeration contract, complete heap
byte accounting, generational stale-handle protection, or a collector. Weak
collections currently share strong entry storage with ordinary collections.

### Mixed JavaScript And Engine Error Channels

JavaScript throws, parser failures, resource failures, host errors, and engine
invariant errors share overlapping error paths. Some JavaScript error
classification is recovered from message text. Source offsets are available
at tokenization and parse-failure sites, but stable source spans are not carried
through frontend and bytecode metadata.

### Optimization Leakage Into Semantics

The runtime contains specializations for narrow loop shapes, and the compiler
contains source-name recognition for support functions such as `print` and
`assert.throws`. These paths can be useful prototypes, but workload or harness
knowledge must not define the language semantics or become the only execution
path for a feature.

### Misleading Single-Number Progress

A full Test262 percentage combines core language semantics, large optional
libraries, proposals, modules, internationalization, binary data, and async
protocols. It is useful as a global signal but is not a sufficient milestone
for architecture or product readiness.

## Target Architecture

### One Semantic Object Model

All JavaScript objects should eventually use one semantic object reference.
The physical payload may remain split across typed VM-owned arenas, but every
object must participate in the same internal-method boundary.

A target model may use the following conceptual shape:

```text
Value::Object(ObjectRef)

ObjectRecord {
    ordinary: OrdinaryObjectData,
    internal: ObjectKind,
}

ObjectKind =
    Ordinary
    Array(ArraySlots)
    Function(FunctionSlots)
    NativeFunction(NativeFunctionSlots)
    HostFunction(HostFunctionSlots)
    Proxy(ProxySlots)
    Promise(PromiseSlots)
    Collection(CollectionSlots)
    ArrayBuffer(ArrayBufferSlots)
    TypedArray(TypedArraySlots)
    Date(DateSlots)
    RegExp(RegExpSlots)
    ...
```

This sketch is not a requirement to place every payload in one large enum or
one allocation. The required invariant is semantic: one object identity and
one internal-method API must own the behavior.

The common boundary must cover at least:

- `[[GetOwnProperty]]`
- `[[DefineOwnProperty]]`
- `[[HasProperty]]`
- `[[Get]]`
- `[[Set]]`
- `[[Delete]]`
- `[[OwnPropertyKeys]]`
- `[[GetPrototypeOf]]`
- `[[SetPrototypeOf]]`
- `[[IsExtensible]]`
- `[[PreventExtensions]]`
- optional `[[Call]]`
- optional `[[Construct]]`
- heap tracing and resource accounting

No new object-like `Value` variant may be added after plan adoption without a
documented migration reason. New exotic behavior should normally be an
`ObjectKind`/internal-slots payload behind the common boundary.

### Shared ECMAScript Abstract Operations

Bytecode instructions and native built-ins must reuse a shared semantic layer.
The final module layout may evolve, but the ownership should be recognizable,
for example as `runtime::abstract_ops` with focused submodules.

The initial shared set should include:

- primitive conversion: `ToPrimitive`, `ToBoolean`, `ToNumber`, `ToString`;
- object/key conversion: `ToObject`, `ToPropertyKey`;
- numeric indexing: `ToIntegerOrInfinity`, `ToLength`, `ToIndex`;
- equality: strict equality, `SameValue`, `SameValueZero`;
- property operations: `Get`, `Set`, `HasProperty`, `CreateDataProperty`,
  `CreateDataPropertyOrThrow`, `DeletePropertyOrThrow`;
- invocation: `IsCallable`, `IsConstructor`, `Call`, `Construct`, `GetMethod`;
- iteration: `GetIterator`, `IteratorStep`, `IteratorValue`, `IteratorClose`;
- promise resolution and thenable assimilation when the async gate is opened.

An optimized instruction may bypass an abstract operation only when explicit
guards prove an equivalent result. A guard miss must return to the shared
generic operation.

### Typed Completion And Failure Channels

JavaScript execution and engine control must use distinct result channels:

```text
JavaScript completion:
    Normal(Value)
    Return(Value)
    Throw(Value)
    Break(...)
    Continue(...)

Engine or embedding failure:
    ResourceLimit(...)
    Cancelled(...)
    HostFailure(...)
    InvariantViolation(...)
```

Ordinary JavaScript `TypeError`, `ReferenceError`, `RangeError`, and other
exceptions must become real JavaScript objects and travel through `Throw`.
Message-prefix parsing must not be required to recover an exception class.

Source identity and source spans must be available to diagnostics without
forcing runtime execution to retain the parser AST. A side table keyed by
frontend or bytecode ids is acceptable and may be preferable to placing a span
inside every enum variant.

### Explicit Resumable Execution Frames

The VM must make JavaScript activation state explicit. A target frame should
own or reference:

- the current code block/function and program counter;
- operand stack range and local binding frame;
- lexical environment and captured-cell references;
- `this`, `new.target`, and super state;
- exception/finally handler state;
- call/construct metadata;
- suspend/resume metadata;
- resource counters needed for stack and job limits.

The interpreter should return a result such as completed, thrown, suspended,
or yielded without losing the activation state. Fully flattening every
structured instruction is optional. Keeping bytecode blocks in an immutable
arena and saving an explicit block/frame stack is acceptable if it provides a
complete continuation.

Generators, pending `await`, top-level await, and async host callbacks may not
claim complete support until they use this continuation model. Synchronously
draining jobs inside `await` is not the target semantics.

### Rootable And Accounted VM-Owned Heaps

Before a collector is implemented, every heap-owned type must participate in a
rooting and accounting contract.

The contract must define:

- typed or generational handles and stale-handle behavior;
- roots in globals, execution frames, closures, jobs, modules, host handles,
  and temporary native-call state;
- strong and weak edges;
- byte accounting for objects, properties, strings, arrays, buffers, functions,
  jobs, and collection backing storage;
- hard limit checks at allocation/growth boundaries;
- deterministic teardown reporting;
- how a host callback retains or releases a VM-bound handle.

The first collector should be safe Rust and non-moving unless measurements
show a concrete reason for a more complex design. Mark/sweep over indexed
arenas is a valid initial direction. WeakMap, WeakSet, WeakRef, and
FinalizationRegistry semantics depend on the collector and must use explicit
weak edges rather than ordinary strong `Value` storage.

### VM-Bound Embedding Handles

`Vm` and `Context` must not expose cloning semantics that can accidentally
share mutable JavaScript state. Public values should distinguish:

- owned primitive or serialized values that can cross a VM boundary;
- VM-bound local handles validated by VM identity and generation;
- explicit serialization or transfer operations.

The exact Rust API may use lifetimes, checked runtime ids, or both. The required
property is that a value from one VM cannot be accepted by another VM merely
because its numeric slot id exists there.

### Frontend And Bytecode Boundary

The current parser AST remains a valid compile-time IR. Removing it is not part
of the stabilization critical path. The frontend should first gain stable ids,
source spans, literal pools where useful, and clean inputs to binding analysis
and compilation.

A later direct parser-to-frontend-IR redesign requires its own evidence. It
must not be justified only as cleanup, because runtime AST execution is already
forbidden.

### Optimization Boundary

The generic semantic path is the source of truth. Optimization is a separate
layer over stable semantics.

Every new optimization must satisfy all of these conditions:

1. a trusted profile identifies the bottleneck;
2. the bottleneck affects multiple unrelated workloads or protects a declared
   product budget;
3. explicit guards describe when the specialization is valid;
4. a guard miss reaches the generic semantic path;
5. correctness coverage runs with the optimization enabled and disabled;
6. the change reports latency and memory effects on a stable benchmark cohort;
7. the optimization does not encode Test262 harness names or one benchmark's
   source shape as language semantics.

`print`, assertions, and Test262 harness functions should be ordinary host or
harness bindings. They should not require language-level bytecode operations
selected only by source name.

Safe Rust remains the default. NaN boxing, a custom allocator, JIT code
generation, or narrowly scoped `unsafe` are not stabilization tasks.

## Development Methodology

### Work Allocation

During the stabilization program, the default effort split is:

- 40-50% compatibility and product work that does not deepen a blocked area;
- 35-45% architecture and semantic-kernel work;
- 10-15% measurement, profiling, and regression work.

After the core stabilization gates are complete, the default split becomes:

- 60-70% compatibility and product features;
- 20-25% architecture, resource control, and correctness foundations;
- 10-15% performance and memory evidence.

For a group of five or six active agents, the steady-state model is:

- three or four compatibility agents on disjoint feature clusters;
- one platform agent owning shared semantics and architecture invariants;
- at most one performance/evidence agent.

The performance agent should work on profiling, benchmark quality, and memory
accounting when no cross-cutting bottleneck has enough evidence for an
optimization branch.

### Allowed Parallel Work During Stabilization

The following work may proceed while object/heap/execution gates are open:

- repository, CI, report, and corpus reliability;
- focused lexer/parser work that also preserves the planned source metadata;
- direct embedding tests that expose an ownership requirement without freezing
  a new public handle design;
- compatibility tests and failure classification;
- generic semantic implementations that land behind the target boundary;
- benchmark instrumentation and stable-cohort maintenance.

The following work should wait for the named gate:

- new exotic object families: wait for the semantic object boundary;
- additional async/generator/module execution: wait for resumable frames;
- WeakRef, finalization, or production weak collections: wait for roots and GC;
- public object/promise/function host handles: wait for VM-bound handles;
- additional workload-shaped fast paths: wait for optimization isolation.

### Feature Selection

The newest trusted report is always an input, but raw failure count does not
select a task by itself. Choose a branch using this order:

1. check whether the feature is blocked by an architecture gate;
2. identify the product profile and embedding value;
3. find a coherent Test262 feature cluster rather than isolated cases;
4. identify the shared abstract operations the feature should reuse;
5. define ownership, limits, teardown, errors, and observability;
6. add a benchmark only when the feature creates or exercises a meaningful hot
   path;
7. implement the generic semantic path first.

### Product Profiles

Track progress in several profiles instead of using one percentage as the only
goal:

1. Core language: syntax, static semantics, bindings, functions, objects,
   descriptors, prototypes, exceptions, iteration, and coercion.
2. Embedded standard library: Object, Function, Array, String, Number, Math,
   JSON, Date, RegExp, Map, Set, and standard errors.
3. Async and modules: jobs, promises, generators, async functions, modules,
   dynamic import, and top-level await.
4. Binary data: ArrayBuffer, DataView, TypedArray families, and their iterator
   and species behavior.
5. Extended libraries: Intl, Temporal, Atomics, SharedArrayBuffer, and proposal
   surfaces selected by product need.
6. Full Test262: the global compatibility signal across all profiles.

Each report improvement should name the affected profile. A target such as
"80% Test262" is not an architecture gate unless the profile composition is
also stated.

### Definition Of Ready

A task under this plan is ready when it has:

- one plan id from the program table or a newly added child id;
- a fresh `origin/main` base and visible draft pull request;
- a bounded semantic or architecture outcome;
- stated dependencies and gates;
- identified generic semantic operations and fallback behavior;
- expected ownership, rooting, resource-limit, and teardown effects;
- a validation plan appropriate to the task;
- current report or profile evidence when performance or compatibility is the
  reason for the work.

### Definition Of Done

A plan task is complete only when:

- the generic semantics are covered by focused engine tests;
- relevant QuickJS differential and Test262 coverage is updated;
- embedding-facing behavior has direct library tests when applicable;
- resource counters and limits cover newly owned state;
- error and completion behavior is explicit;
- optimization guard misses preserve identical semantics;
- performance and memory evidence exists when a hot path changed;
- the required CI run is green on the final tested tree;
- this plan records status, PR/merge evidence, and remaining work;
- `docs/project-plan.md` is updated when the whole-project task board or
  delivery order changed.

Intermediate checkpoint commits may precede validation, as required by the
repository workflow. Ordinary plan tasks do not bump Cargo package versions.

### Stop-The-Line Conditions

Pause feature expansion in the affected area and create or select an
architecture task when any of these occur:

- a feature requires another object-like `Value` variant;
- property, call, construct, equality, coercion, or iteration semantics would
  be copied into another built-in;
- a new object kind requires more unrelated optional fields in the ordinary
  object record or another untracked side table;
- async behavior depends on synchronously draining all jobs;
- weak behavior is implemented with strong `Value` edges;
- a public handle can cross VM boundaries without identity validation;
- a JavaScript exception class depends on parsing an error message;
- a new fast path has no explicit generic fallback;
- an optimization recognizes one benchmark or harness source shape;
- a new allocation owner has no byte accounting or hard-limit path;
- a compatibility PR cannot explain which semantic layer owns the behavior.

## Stabilization Program

Status values are `Backlog`, `In progress`, `Blocked`, `Complete`, and
`Deferred`. At most one top-level item should normally be marked `In progress`;
independent child items may proceed in parallel when their file ownership and
dependencies do not overlap.

| ID | Status | Program item | Depends on | Completion evidence |
| --- | --- | --- | --- | --- |
| AS-00 | Complete | Adopt this plan and route project documentation to it. | None | PR #396 merged as `f79056b`; required CI, post-merge performance, publisher, and canonical report publication passed. |
| AS-01 | In progress | Inventory semantic entrypoints and add architecture guards. | AS-00 | AS-01a merged in PR #398; AS-01b guards are implemented in PR #399 pending required CI and merge. |
| AS-02 | Backlog | Introduce the unified semantic object and internal-method boundary. | AS-01 | Ordinary objects, functions, native/host functions, errors, proxies, promises, and collections can migrate through one semantic facade. |
| AS-03 | Backlog | Centralize ECMAScript abstract operations. | AS-01, AS-02 foundation | Shared coercion, equality, property, invocation, and iterator operations used by bytecode and built-ins. |
| AS-04 | Backlog | Separate JavaScript completions from engine failures and add source metadata. | AS-01; coordinate with AS-02 | Real JavaScript error objects, typed throw path, no message-prefix classification, spans available to diagnostics. |
| AS-05 | Backlog | Define VM-bound handles, roots, and complete resource accounting. | AS-02 foundation, AS-04 | Non-cloneable VM state, checked cross-VM boundaries, trace/root contract, heap/stack/job/buffer counters and limits. |
| AS-06 | Backlog | Introduce explicit resumable execution frames. | AS-03, AS-04, AS-05 root contract | Synchronous execution migrated without regressions; suspended/yielded outcomes preserve complete activation state. |
| AS-07 | Backlog | Add safe collection and correct weak-edge semantics. | AS-05, AS-06 | Collector with explicit roots, deterministic teardown, hard heap limits, correct WeakMap/WeakSet behavior. |
| AS-08 | Backlog | Isolate quickening, inline caches, and loop specialization from semantics. | AS-02, AS-03, AS-06 | Optimizer on/off equivalence, harness opcodes removed, workload-shaped paths replaced or justified by broad evidence. |
| AS-09 | Backlog | Scale compatibility work across product profiles. | Relevant AS-02 through AS-07 gates | Multiple feature clusters land through shared semantics without new architecture exceptions. |
| AS-10 | Backlog | Run recurring performance and memory checkpoints. | Stable benchmark cohort; relevant subsystem maturity | Profile, stable latency/memory comparison, named cross-cutting debt, regression gate updates. |

## Program Item Details

### AS-00: Plan Adoption

Deliverables:

- publish this document;
- link it from README, roadmap, and project plan;
- state how it interacts with the whole-product roadmap;
- keep the initial review baseline for provenance;
- validate the documentation-only change through the fast gate.

Evidence:

- PR: #396
- Merge: `f79056b`
- Tests: the fast gate passed with `RSQJS_BASE_REF=origin/main` and
  `RSQJS_FAST_RUNNER=1`; required CI run `29052367465` passed in 46 seconds
- Test262/QuickJS: compatibility stayed at 34,002 of 102,578 full Test262
  variants and the existing differential baseline remained green
- Performance/memory: post-merge performance and publisher run `29052445325`
  passed; canonical report
  `reports/test-runs/rsqjs-test-report-20260709T214557Z.md` was published by
  report-only commit `5f23559`
- Remaining: start AS-01a, the semantic entrypoint and ownership inventory

### AS-01: Semantic Inventory And Guards

Create an inspectable map of current entrypoints for:

- property read/write/define/delete/enumeration;
- callable and constructable values;
- prototype and descriptor behavior;
- equality and conversion;
- iterator protocol;
- JavaScript throw creation and propagation;
- VM-owned stores, side tables, roots, and public handles;
- bytecode and native built-in generic/optimized paths.

Convert the most important boundaries into architecture tests or focused lint
scripts. Guards should reject new parser-AST runtime imports, new object-like
value variants, source-name harness opcodes, and other mechanically detectable
regressions.

AS-01a evidence:

- Inventory: [Semantic Architecture Inventory](semantic-architecture-inventory.md)
- PR: #398
- Merge: `56e6400`
- Covered: object-like values, physical stores, property/call/construct paths,
  abstract-operation duplicates, completion/error paths, iteration, roots,
  handles, accounting, and optimization owners
- Tests: the fast gate passed with engine and runner validation; required CI
  run `29053863483` passed in 47 seconds
- Test262/QuickJS: no runtime behavior or corpus baseline changed
- Performance/memory: post-merge performance and publisher run `29053927852`
  passed; canonical report
  `reports/test-runs/rsqjs-test-report-20260709T221425Z.md` was published by
  report-only commit `7fac57e`
- Remaining for AS-01: AS-01b in draft PR #399 must merge the deterministic
  no-growth guards

AS-01b evidence:

- PR: #399
- Scope: deterministic structural allowlists for split object/value state,
  frontend/runtime separation, source-name harness paths, duplicated semantic
  operations, optimization owners, and VM-state cloning debt
- Tests: 16 negative mutation probes passed; the fast gate passed with engine
  and runner formatting, strict clippy, tests, and documentation
- Test262/QuickJS: no runtime behavior or corpus baseline changed
- Performance/memory: no hot path or owned runtime state changed; no local
  benchmark run was warranted
- Remaining for AS-01: required CI and merge of PR #399; AS-02a should record
  the final merge and canonical report evidence when it starts

### AS-02: Unified Semantic Object Boundary

Migrate incrementally rather than replacing every store in one pull request:

1. introduce a checked semantic object reference/facade over existing stores;
2. route ordinary property internal methods through that facade;
3. route functions and native/host functions through common object properties;
4. unify call and construct dispatch behind optional internal methods;
5. turn Error instances into ordinary JavaScript objects;
6. migrate Proxy, Promise, collections, ArrayBuffer, and typed arrays to typed
   internal slots;
7. remove obsolete parallel property and descriptor implementations.

Physical arena consolidation is optional. Semantic duplication is not.

### AS-03: Abstract Operations

Land operations in coherent groups:

1. equality and primitive conversion;
2. property-key and numeric-index conversion;
3. generic object property operations;
4. call/construct and method lookup;
5. iterator operations and iterator closing;
6. promise resolution after resumable execution is available.

Each migrated built-in must delete or delegate its local duplicate. Avoid a
permanent period in which two implementations are both treated as canonical.

### AS-04: Completion, Errors, And Source Metadata

Required migration steps:

1. define the typed boundary between JavaScript abrupt completion and engine
   failure;
2. route native built-ins through JavaScript `Throw(Value)` where the
   specification requires an exception;
3. create real Error objects through the unified object model;
4. remove exception classification by string prefix;
5. add `SourceId` and stable spans to frontend/bytecode metadata;
6. expose structured diagnostic fields without making formatted text an API.

### AS-05: Ownership, Handles, Roots, And Accounting

Required outcomes:

- `Vm`/`Context` cloning cannot share mutable JavaScript state accidentally;
- VM-bound values are validated by identity and generation or lifetime;
- host callbacks use explicit local/owned value boundaries;
- all VM stores participate in root enumeration;
- allocation and growth paths report bytes and enforce hard limits;
- stack, atom, string, object, property, buffer, job, module, callback, and
  output budgets are visible in the embedding API;
- teardown reports account for every owner.

### AS-06: Resumable Execution

Migrate the existing synchronous engine before exposing new async behavior:

1. represent call activations as explicit VM-owned frames;
2. represent try/finally and loop control without relying on the Rust call
   stack as the durable state;
3. preserve current bytecode and Test262 behavior;
4. add stack-depth and frame-memory accounting;
5. add suspended/yielded run outcomes;
6. resume pending promises and generators through the VM job/frame APIs;
7. add explicit job-draining and cancellation surfaces for embedders.

### AS-07: Collection And Weak Semantics

Start with a safe non-moving collector over indexed arenas. The collector must
enumerate all roots, preserve host handles, integrate with jobs and suspended
frames, enforce hard heap limits, and produce deterministic teardown data.

WeakMap and WeakSet must stop retaining keys strongly. WeakRef and
FinalizationRegistry remain gated until collection order and callback/job
semantics are specified and tested.

### AS-08: Optimization Isolation

Create an explicit optimization owner for:

- bytecode verification and quickening;
- direct binding/property operands;
- inline-cache state and invalidation;
- generic superinstructions with broad evidence;
- dense-array and native-call specializations;
- stable profiling counters.

Audit existing narrow loop paths. Keep a path only when it expresses a reusable
operation, has complete guards, and demonstrates value across unrelated
workloads. Remove or replace harness-specific bytecode and source-name
recognition.

### AS-09: Profile-Based Compatibility Expansion

After the relevant gates close, compatibility agents should work in coherent
clusters and reuse the semantic kernel. Each cluster records profile progress,
not just the global Test262 delta.

Suggested order after the semantic object gate:

1. core object/function/prototype/descriptor correctness;
2. coercion, equality, and iterator correctness;
3. embedded standard-library clusters;
4. resumable generators, jobs, promises, async, and modules;
5. complete ArrayBuffer/DataView/TypedArray families;
6. product-selected extended libraries.

### AS-10: Performance And Memory Checkpoints

Maintain a stable core benchmark cohort whose history remains comparable even
when new feature benchmarks are added. Report at least:

- parse and compile cost;
- cold evaluation;
- repeated compiled evaluation;
- property access and mutation;
- call and construct paths;
- arrays and typed arrays;
- host callbacks and job draining;
- VM creation/teardown;
- process RSS where meaningful;
- engine-owned heap bytes by category.

An optimization task starts only after a profile identifies a cross-cutting
cause. Geometric means over different benchmark subsets must not be compared as
if they were the same metric.

## First Execution Queue

The first branch-sized tasks after AS-00 should be selected in this order. A
task may be split further if it would otherwise mix owners or exceed a
reviewable scope.

1. AS-01a: inventory all object-like `Value` variants, property/call/construct
   entrypoints, VM stores, and semantic duplicates in
   [Semantic Architecture Inventory](semantic-architecture-inventory.md)
   (complete in PR #398).
2. AS-01b: add architecture guards for new split object variants and
   source-name harness bytecode.
3. AS-02a: introduce the checked semantic object reference/facade while keeping
   current physical stores.
4. AS-02b: route get/has/set/define/delete/own-keys through common internal
   methods.
5. AS-02c: route JavaScript, native, host, bound, and callable Proxy values
   through common call/construct internal methods.
6. AS-03a: centralize equality and primitive conversion operations.
7. AS-03b: centralize property-key, property, call, and iterator operations.
8. AS-04a: separate JavaScript throw completion from engine/host/resource
   failures.
9. AS-04b: migrate Error values into real objects and add source-span metadata.
10. AS-05a: remove ambiguous VM cloning and define VM-bound handle identity.
11. AS-05b: add root enumeration plus complete allocation accounting and
    limits.
12. AS-06a: migrate synchronous calls and structured control flow to explicit
    activation frames.
13. AS-06b: add suspend/resume outcomes and correct pending `await` behavior.
14. AS-07a: add safe collection over explicit roots and correct weak edges.
15. AS-08a: move reusable optimization state behind one optimizer/quickening
    boundary and remove harness-specific opcodes.

## Updating This Plan

Every plan implementation PR must update the program table or add a child item
with:

- current status;
- PR and final merge commit;
- focused tests and compatibility delta;
- CI run and report artifact path;
- latency and memory evidence when applicable;
- architecture decisions or deviations;
- explicitly remaining follow-up work.

Use the following compact evidence format in the relevant item:

```text
Evidence:
- PR: #...
- Merge: ...
- Tests: ...
- Test262/QuickJS: ...
- Performance/memory: ...
- Remaining: ...
```

Do not mark an item complete because a prototype exists. Completion means the
target boundary owns the production path, compatibility evidence is green, and
obsolete competing paths have been removed or explicitly scheduled.

## Explicitly Deferred Work

The following work is not part of the stabilization critical path:

- removing the compile-time parser AST;
- JIT compilation;
- NaN boxing;
- a custom allocator;
- unsafe execution paths;
- polymorphic inline-cache sophistication beyond measured needs;
- aggressive shape-table or atom-table tuning without profiles;
- full Intl or Temporal implementation selected only to raise the global
  Test262 percentage;
- parallel execution inside one arbitrary JavaScript context.

These items may be reconsidered only after the relevant semantic, ownership,
and measurement foundations are complete.
