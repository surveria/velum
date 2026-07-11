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
- Current program state: AS-01 through AS-05 and AS-06a are complete through
  PR #442. PR #445 adds suspended outcomes, same-owner async resume, and
  embedder-controlled job lifecycle APIs

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
counts. AS-05b1a now provides one direct-root enumeration contract, but the
engine does not yet have complete heap-edge traversal, heap byte accounting,
generational stale-handle protection, or a collector. Weak collections
currently share strong entry storage with ordinary collections.

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
| AS-01 | Complete | Inventory semantic entrypoints and add architecture guards. | AS-00 | AS-01a merged in PR #398; AS-01b guards merged in PR #399 with required CI and canonical report publication. |
| AS-02 | Complete | Introduce the unified semantic object and internal-method boundary. | AS-01 | AS-02a merged in PR #400; AS-02b1 merged in PR #401; AS-02b2 merged in PR #403; AS-02c merged in PR #408 with required CI and canonical report publication. |
| AS-03 | Complete | Centralize ECMAScript abstract operations. | AS-01, AS-02 foundation | AS-03a1 equality merged in PR #409; AS-03a2 conversions completed through PRs #410 and #411; AS-03b1a `ToPropertyKey` merged in PR #412; AS-03b1b integer/length/index conversion merged in PR #413; AS-03b2 property/method/call operations merged in PR #414; AS-03b3 iterator operations merged in PR #415. |
| AS-04 | Complete | Separate JavaScript completions from engine failures and add source metadata. | AS-01; coordinate with AS-02 | AS-04a typed throw boundary merged in PR #416; AS-04b1 ordinary Error object identity merged in PR #418; AS-04b2a source identity/frontend diagnostics merged in PR #419; AS-04b2b1 token ranges/span-bearing AST merged in PR #420; AS-04b2b2 bytecode/runtime spans merged in PR #421 with exact-tree correctness and canonical report publication. |
| AS-05 | Complete | Define VM-bound handles, roots, and complete resource accounting. | AS-02 foundation, AS-04 | AS-05a1 through AS-05b2c3 are merged through PR #436 with exact-tree correctness, complete owner-limit reconciliation, and canonical report publication. |
| AS-06 | Complete | Introduce explicit resumable execution frames. | AS-03, AS-04, AS-05 root contract | AS-06a1 through AS-06a2b merged in PRs #438 through #442. AS-06b merged in PR #445 as `9e25e77` with exact-tree correctness and canonical report publication. |
| AS-07 | Complete | Add safe collection and correct weak-edge semantics. | AS-05, AS-06 | PR #446 merged as `62e2725`; exact-tree correctness, paired sentinels, post-merge performance, and canonical report publication passed. |
| AS-08 | Complete | Isolate quickening, inline caches, and loop specialization from semantics. | AS-02, AS-03, AS-06 | AS-08a and AS-08b merged through PR #449; exact-tree correctness, disabled-mode equivalence, specialization audit, paired sentinels, and canonical publication passed. |
| AS-09 | In progress | Scale compatibility work across product profiles. | Relevant AS-02 through AS-07 gates | AS-09a starts the profile-driven expansion with unary bitwise NOT through the shared numeric semantics and no architecture exception. |
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
- Merge: `703a3fe`
- Scope: deterministic structural allowlists for split object/value state,
  frontend/runtime separation, source-name harness paths, duplicated semantic
  operations, optimization owners, and VM-state cloning debt
- Tests: 16 negative mutation probes passed; the fast gate passed with engine
  and runner formatting, strict clippy, tests, and documentation; required CI
  run `29054677736` passed in 47 seconds
- Test262/QuickJS: no runtime behavior or corpus baseline changed
- Performance/memory: post-merge performance and publisher run `29054773356`
  passed; canonical report
  `reports/test-runs/rsqjs-test-report-20260709T223143Z.md` was published by
  report-only commit `5e81d2e`
- Remaining for AS-01: none; AS-02 now owns the next architecture boundary

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

AS-02a evidence:

- PR: #400
- Merge: `b760177`
- Scope: one checked `Context::semantic_object_ref` entrypoint over Object,
  JavaScript function, native function, host function, and inline Error
  storage, without changing the public `Value` representation
- Initial migrations: Proxy object validation, Proxy construct-result
  validation, JavaScript constructor return selection, and typed-array debug
  inspection
- Tests: focused public smoke coverage validates all five current object-like
  owners, undefined store slots, primitive exclusion, and retained
  constructor/Proxy behavior; required correctness CI run `29055467174`
  passed in 54 seconds with all 34,002 expected Test262 variants and all 95
  QuickJS differential cases green
- Ownership limit: AS-02a validates local slot existence but cannot identify a
  foreign VM value whose numeric slot happens to alias a live local slot;
  VM-bound identity and generations remain AS-05a
- Performance/memory: post-merge performance and publisher run `29055601549`
  passed in 49 and 17 seconds; canonical report
  `reports/test-runs/rsqjs-test-report-20260709T224900Z.md` was published by
  report-only commit `a5b3909`
- Remaining for AS-02a: none

AS-02b1 evidence:

- PR: #401, squash-merged as `92eac23`
- Scope: shared semantic-object `[[Get]]` and `[[HasProperty]]` pre-dispatch,
  explicit `Reflect.get` receiver propagation, ordinary-object cache tails,
  and generic fallbacks
- Consolidation: dynamic/static reads and presence checks, computed-symbol
  destructuring, iterator method lookup, descriptor field reads, generic array
  presence, `Object.prototype.toString`, and `Reflect.get` now enter through
  the shared boundary
- Proxy correction: get/has traps now receive the original Symbol key rather
  than its display string, and no-trap fallback preserves the same lookup
- Tests: focused coverage exercises ordinary, JavaScript-function,
  native-function, Error, boxed-string, and HostFunction behavior plus Proxy
  Symbols, accessors, explicit receivers, descriptors, and iteration; the
  exact-head fast gate passed, and the complete local correctness refresh
  passed with all 34,006 expected Test262 variants and all 95 QuickJS
  differential cases green
- Test262 change: the first required CI run preserved all 34,002 existing
  passes and detected four intentional new variants: default and strict forms
  of `proxy-function-async.js` and `proxy-revoked.js`; the official full-corpus
  refresh accepted exactly those four variants
- Validation/publication: required CI run `29057007323` passed in 59 seconds on
  tree `58678b69`; post-merge performance and publisher run `29057098105`
  passed, and report-only commit `1471b29` published matching Markdown and YAML
  reports at `reports/test-runs/rsqjs-test-report-20260709T232030Z.*`
- Remaining for AS-02b1: none

AS-02b2 evidence:

- PR: #403, squash-merged as `9697734`
- Scope: shared object-like `[[Set]]`, `[[Delete]]`,
  `[[DefineOwnProperty]]`, `[[GetOwnProperty]]`, `[[OwnPropertyKeys]]`,
  prototype, extensibility, and integrity dispatch
- Cache boundary: static/dynamic write and delete caches receive only explicit
  ordinary-object tails after Proxy, function, native-function, Error, and
  HostFunction pre-dispatch
- Proxy correction: set/delete/define/descriptor/ownKeys preserve Symbol keys;
  `Reflect.set` preserves its explicit receiver across Proxy and prototype
  recursion; Proxy seal/freeze uses observable internal methods rather than
  mutating the wrapper's physical record; `ownKeys` validates duplicate,
  non-configurable, and non-extensible target invariants across string and
  Symbol keys
- Consolidation: Object, Reflect, JSON, regexp state writes, destructuring,
  object spread, and `Object.prototype.isPrototypeOf` delegate to the shared
  boundary while physical stores remain backend-only
- Tests: four new public regression cases cover Symbol mutation and metadata,
  receiver-aware Reflect writes, function mutation, Proxy integrity, and
  falsy prototype traps; the first required CI exposed 160 new passes plus a
  two-variant `ownKeys` invariant regression, which was fixed rather than
  removed from the baseline
- Test262/QuickJS: the reviewed full-corpus refresh adds 180 variants across 91
  files with zero lost passes; complete local correctness is green at
  34,186/34,186 expected Test262 variants and 95/95 QuickJS differential cases
- Validation/publication: required correctness run `29071727421` passed on
  tested tree `73df8768`; trusted historical correctness recovery run
  `29072795054` reproduced that exact tree after `main` moved; rerun attempt 2
  of post-merge workflow `29071824973` then published the canonical report
  `reports/test-runs/rsqjs-test-report-20260710T060416Z.*` in report-only
  commit `66dad44`
- Workflow hardening: PRs #405 through #407 made performance artifacts trust
  the current post-merge run and added trusted exact-tree correctness recovery
  without accepting caller-supplied artifacts or tree claims
- Remaining for AS-02b2: none

AS-02c evidence:

- PR: #408, squash-merged as `1b51bed`
- Scope: one checked `semantic_is_callable`/`semantic_call` and
  `semantic_is_constructor`/`semantic_construct` owner for JavaScript, native,
  host, bound-function, and callable/constructable Proxy values
- Construction semantics: explicit `newTarget` now reaches JavaScript
  constructors, Proxy traps and fallbacks, and bound-target replacement;
  callable and constructable Proxy capabilities are captured independently at
  creation and survive revocation
- Consolidation: generic bytecode calls/construction, Function helpers,
  accessors, array/collection callbacks, JSON, Promise, Reflect, Proxy,
  `typeof`, and coercion hooks share the predicates and dispatch; guarded
  direct-native and call-cache hits remain backend optimizations
- Tests: focused regression coverage exercises callable Proxy consumers,
  JSON callbacks, Proxy/Reflect `newTarget`, nested Proxy construction, bound
  constructors, non-constructable bound arrows, and host-function Proxy
  capability; the complete engine/runner fast gate passes
- Test262/QuickJS: the reviewed pass-set refresh adds exactly 87 variants with
  zero removals, moving the full corpus from 34,186 to 34,273 of 102,578;
  the refreshed 34,273/34,273 expected-pass baseline and all 95 QuickJS
  differential cases pass the complete local correctness gate
- Recorded residuals: alternate `newTarget.prototype` is not yet applied by
  native constructor payloads, and derived-class `super()` still initializes a
  pre-created receiver in place; AS-03b and AS-06 own those migrations
- Validation/publication: required correctness run `29074703586` certified
  tree `eed7d1ab` at 34,273/34,273 expected Test262 variants and 95/95 QuickJS;
  post-merge run `29074810069` measured the five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T064817Z.*` in
  report-only commit `ed25948`
- Remaining for AS-02c: none

AS-02 completion means observable property and invocation dispatch now has one
semantic boundary. Representation migrations that require real Error identity,
VM-bound handles, complete typed internal slots, roots, or collection are
explicitly owned by AS-04, AS-05, and AS-07 rather than keeping AS-03 blocked
behind physical arena consolidation.

### AS-03: Abstract Operations

Land operations in coherent groups:

1. equality relations;
2. primitive conversion;
3. property-key and numeric-index conversion;
4. generic object property operations;
5. call/construct and method lookup;
6. iterator operations and iterator closing;
7. promise resolution after resumable execution is available.

Each migrated built-in must delete or delegate its local duplicate. Avoid a
permanent period in which two implementations are both treated as canonical.

AS-03a1 completion evidence:

- PR: #409, squash-merged as `d16197d`
- Scope: one pure runtime owner for Abstract Equality, Strict Equality,
  `SameValue`, and `SameValueZero`, including their numeric specializations
- Consolidation: bytecode, numeric quickening/control paths, `Object.is`,
  Map/Set, generic arrays, and packed/holey array paths delegate equality truth
  to `runtime/abstract_operations/equality.rs`
- Guard: the equality duplicate allowlist permits definitions only in that
  owner; optimized paths may select operands and negate a result but may not
  redefine NaN or signed-zero semantics
- Tests: focused public coverage exercises primitive, boxed-string, Symbol,
  object identity, NaN, signed zero, arrays, collections, switch, callback, and
  loop paths; the complete engine/runner fast gate passes
- Test262/QuickJS: required CI run `29075779553` certified tree `8eda1227`
  with no pass-set change at 34,273/34,273 expected Test262 variants, 34,273
  of 102,578 full variants, and 95/95 QuickJS differential cases
- Publication: post-merge run `29075883784` measured all five project sentinels
  and published `reports/test-runs/rsqjs-test-report-20260710T071037Z.*` in
  report-only commit `b5e6147`
- Remaining for AS-03a1: none

The primitive-conversion group is split by dependency and review boundary:

- AS-03a2a (merged PR #410) owns `ToPrimitive`, `OrdinaryToPrimitive`, and
  `ToNumber`. It replaces the Date, Math, JSON, boxed-string equality, numeric
  operator, numeric built-in, and numeric argument conversion paths with one
  abrupt-completion-aware owner under `runtime/abstract_operations`. It also
  installs the missing `Function.prototype.toString` intrinsic so function
  source coercion reaches the ordinary property/call path instead of a concat
  formatter exception.
- AS-03a2b (merged PR #411) owns `ToString` and `ToBoolean`. It removes direct
  truthiness from `Value`, routes observable string consumers through the
  shared conversion boundary, and reserves Rust `Display` for diagnostics.

AS-03a2a keeps numeric fast paths only when operands are already numbers. Every
generic fallback must call the shared conversion owner; no built-in may probe
`valueOf`/`toString` itself. The architecture guard records the complete owner
function set and rejects a second conversion definition.

AS-03a2a local validation evidence:

- the engine and runner fast gate passes, including strict Clippy, unit and
  integration tests, documentation, architecture guards, and 112 runner tests;
- focused public tests cover conversion hints, ordinary method order, abrupt
  conversion, numeric consumers, and array-search conversion ordering;
- the complete Test262 review preserves every one of the prior 34,273 expected
  variants and adds 1,330 newly passing variants, bringing the reviewed
  expected-pass baseline and full-corpus pass set to 35,603 of 102,578;
- the largest gains are addition (702 variants), Array (288), Date (94),
  Number (62), String (52), and Object (48), while QuickJS differential remains
  95 of 95;
- array length writes were added through the shared number conversion boundary
  because conversion callbacks can mutate `length` before Array search methods
  continue; empty search receivers now return before converting `fromIndex`.

AS-03a2a completion evidence:

- PR #410 was squash-merged as `4ec0e115`; required CI run `29078751092`
  certified exact tree `3c8ea146` at 35,603/35,603 expected Test262 variants
  and 95/95 QuickJS differential cases;
- post-merge run `29078940370` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T081035Z.*` in
  report-only commit `28e293d`.

AS-03a2b local validation evidence:

- one `Context::to_string` owner performs hint-aware object conversion and one
  `to_boolean` owner covers the complete value domain; primitive-only number
  formatting remains reusable without invoking JavaScript;
- bytecode branches, logical operators, callbacks, Proxy traps, Set iterators,
  concatenation, templates, and String, Array, RegExp, JSON, Date, Function,
  Error, Symbol, and global string consumers delegate to these owners;
- focused public tests cover hints, ordinary fallback order, symbol handling,
  left-to-right Function argument conversion, truthiness without user code,
  Error property conversion, and Error `newTarget.prototype` ordering;
- the complete engine/runner fast gate passes, including strict Clippy,
  documentation, architecture self-tests, and 112 runner tests;
- the complete Test262 review preserves all 35,603 prior expected variants and
  adds 384 reviewed passes, bringing the expected-pass baseline and full pass
  set to 35,987 of 102,578 with QuickJS differential unchanged at 95 of 95;
- the largest gains are String (262 variants), RegExp (38), Function (26),
  addition (18), Array (10), and Error-family behavior (14).

AS-03a2b completion evidence:

- PR #411 was squash-merged as `49b1faaf`; required CI run `29081075120`
  certified exact tree `4d3e64bb` at 35,987/35,987 expected Test262 variants
  and 95/95 QuickJS differential cases;
- post-merge run `29081261765` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T085419Z.*` in
  report-only commit `04cef12`.

The remaining AS-03b work is split into narrow semantic ownership boundaries:

- AS-03b1a owns `ToPropertyKey` and all dynamic key consumers;
- AS-03b1b owns `ToIntegerOrInfinity`, `ToLength`, and `ToIndex`;
- AS-03b2 owns `GetMethod` plus specification-level property and call
  operations over the AS-02 internal methods;
- AS-03b3 owns iterator acquisition, stepping, values, and closing.

AS-03b1a local validation evidence:

- one `Context::to_property_key` owner applies string-hint `ToPrimitive`,
  preserves Symbol identity, delegates non-symbol primitive formatting to the
  shared `ToString` operation, and reuses interned property keys;
- dynamic bytecode access, Object, Reflect, and Proxy paths delegate to this
  owner, while the former Rust-`Display` property-key conversion is deleted;
- focused public tests cover every dynamic consumer, ordinary method order,
  Symbol identity, and abrupt or invalid primitive conversion;
- the complete engine/runner fast gate passes, including strict Clippy,
  documentation, architecture self-tests, and 112 runner tests;
- the complete Test262 review preserves all 35,987 prior expected variants and
  adds 96 reviewed passes, bringing the expected-pass baseline and full pass
  set to 36,083 of 102,578 with QuickJS differential unchanged at 95 of 95;
- the gains cover Object (56 variants), expression and statement property
  names (20), SpiderMonkey staging cases (8), Array (6), computed property
  names (4), and Symbol behavior (2).

AS-03b1a completion evidence:

- PR #412 was squash-merged as `63315e3`; required CI run `29082427967`
  certified exact tree `e96d55d21d45f40638faa5ea486a5550c01ef31d` at
  36,083/36,083 expected Test262 variants and 95/95 QuickJS differential
  cases;
- post-merge run `29082667976` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T091940Z.*` in
  report-only commit `75499a1`.

AS-03b1b local validation evidence:

- `Context::to_integer_or_infinity`, `Context::to_length`, and
  `Context::to_index` are the only specification-named owners; the guard
  rejects another definition;
- Array and String indices, array-like lengths, Function.apply, Reflect and
  Proxy argument lists, RegExp `lastIndex`, Number formatting, Set records,
  Date clipping, ArrayBuffer, and Uint8Array delegate to the shared operations;
- `ToLength` retains the full `Number.MAX_SAFE_INTEGER` range, while conversion
  to `usize`, array storage bounds, byte-buffer limits, and execution budgets
  remain explicit engine-resource checks rather than JavaScript semantics;
- direct tests cover observable number hints, truncation, negative zero,
  case-sensitive Infinity parsing, maximum safe array-like indices, missing
  indices, buffer coercion, and `ToIndex` range/type errors;
- an exhaustive regression audit found and repaired case-sensitive Infinity
  parsing and negative-zero representation before the baseline was refreshed;
- the complete engine/runner fast gate passes, including strict Clippy,
  documentation, architecture self-tests, and 112 runner tests;
- the final complete Test262 review preserves all 36,083 prior expected
  variants and adds 102 reviewed passes, bringing the expected-pass baseline
  and full pass set to 36,185 of 102,578 with QuickJS differential unchanged at
  95 of 95;
- the gains cover Array (58 variants), ArrayBuffer (12), RegExp (10),
  SpiderMonkey staging cases (8), Number (6), Object (4), String (2), and
  Uint8Array constructor behavior (2).

AS-03b1b completion evidence:

- PR #413 was squash-merged as `435b5f8`; required CI run `29085127624`
  certified exact tree `c9f9e7f61810dec52925f1d40ec2dad962198120` at
  36,185/36,185 expected Test262 variants and 95/95 QuickJS differential
  cases;
- post-merge run `29085347387` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T100811Z.*` in
  report-only commit `86f8f4b`.

AS-03b2 local validation evidence:

- `Context::get`, `Context::set`, `Context::call`, and
  `Context::get_method` in `runtime/abstract_operations/property_call.rs`
  compose the AS-02 internal methods; named-key and native-value helpers only
  adapt keys or completion results and do not repeat semantic dispatch;
- `SetFailureBehavior` makes the specification's false-versus-throw choice
  explicit. Reflect and Proxy fallback preserve an explicit receiver, while
  RegExp `lastIndex` writes use the strict throw behavior;
- generic runtime reads and calls, Proxy traps, `@@toPrimitive`,
  `@@hasInstance`, Object invocation, and Set-record methods delegate to the
  shared operations. The old `get_property_value`,
  `get_property_value_with_lookup`, `eval_call_value`,
  `eval_call_completion`, and local Proxy `GetMethod` facade are deleted;
- focused public tests cover getter and call receivers, nullish methods,
  non-callable rejection, abrupt getter completion, Symbol hooks, explicit Set
  receivers, false writes, and strict non-writable RegExp writes;
- the architecture guard fixes the abstract-operation owner set, rejects the
  removed legacy facades, and exercises both failures in self-tests;
- the complete engine/runner fast gate passes, including strict Clippy,
  documentation, architecture self-tests, and 112 runner tests;
- the complete Test262 review preserves all 36,185 prior expected variants and
  adds 24 reviewed RegExp/staging passes for strict non-writable `lastIndex`
  behavior, bringing the expected-pass baseline and full pass set to 36,209 of
  102,578 with QuickJS differential unchanged at 95 of 95.

AS-03b2 completion evidence:

- PR #414 was squash-merged as `be331b2`; required CI run `29086423614`
  certified exact tree `146537c758563c74ce099479ac7253c473e89c60` at
  36,209/36,209 expected Test262 variants and 95/95 QuickJS differential
  cases;
- post-merge run `29086606187` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T103147Z.*` in
  report-only commit `bf4a298`.

AS-03b3 local validation evidence:

- `Context::get_iterator`, `get_iterator_from_method`, `iterator_step`,
  `iterator_close`, and `iterator_close_on_error` in
  `runtime/abstract_operations/iterator.rs` own iterator acquisition,
  stepping, value extraction, and closing; `runtime/bytecode/for_of.rs` now
  owns only loop control;
- destructuring, spread, for-of, Map, Set, WeakMap, WeakSet,
  `Math.sumPrecise`, `Object.fromEntries`, and Set algebra delegate to the
  shared operations; the independent Set-algebra protocol loop is deleted;
- `IteratorClose` validates the return result, preserves the specification's
  original-throw precedence, propagates close failures for non-throw early
  completion, and never suppresses engine resource-limit failures;
- array and string direct implementations remain explicit guarded backends
  for built-in protocol methods that are not installed yet. Observable
  callable `String.prototype[Symbol.iterator]` overrides use the generic
  protocol path;
- the architecture guard fixes the iterator owner set, rejects the removed
  bytecode facades and Set loop, and exercises both failures in self-tests;
- six focused public tests cover close precedence and result validation,
  receiver identity, consumer-side closing, non-object `next` results, and a
  primitive-string iterator override;
- the complete engine/runner fast gate passes, including strict Clippy,
  documentation, architecture self-tests, and 112 runner tests;
- the complete Test262 review preserves all 36,209 prior expected variants and
  adds 12 reviewed Object.fromEntries/for-of/staging passes, bringing the
  expected-pass baseline and full pass set to 36,221 of 102,578 with QuickJS
  differential unchanged at 95 of 95. The local evidence is
  `target/rsqjs-reports/test-runs/rsqjs-test-report-20260710T105146Z.*`.

AS-03b3 completion evidence:

- PR #415 was squash-merged as `fb9917e`; required CI run `29087917661`
  certified exact tree `5b27931a9e3bacd03ef880e2942fcd7c6810e86c` at
  36,221/36,221 expected Test262 variants and 95/95 QuickJS differential
  cases;
- post-merge run `29088089959` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T110040Z.*` in
  report-only commit `d58e48f`;
- Remaining for AS-03: none.

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

AS-04a owns steps 1 and 2 plus removal of message-based exception
classification. AS-04b1 owns ordinary Error object identity and structured
built-in error metadata. AS-04b2a owns canonical source identity and structured
frontend errors. AS-04b2b1 gives tokens and every recursive frontend AST node a
canonical range. AS-04b2b2 lowers those ranges into bytecode/runtime metadata.

AS-04a local implementation evidence:

- `Error::JavaScript { value }` is the single reversible `Result` carrier for
  an arbitrary JavaScript thrown value. `Completion::into_result`, function
  result conversion, and native value conversion preserve that value instead
  of formatting it as `Error::Runtime`;
- `runtime_exception_value` unwraps only the typed JavaScript variant.
  `ReferenceError` helpers create typed JavaScript errors directly; the
  `ReferenceError:` prefix parser and every `uncaught throw:` conversion are
  deleted;
- accessor, JSON callback, eval, iterator, collection, Proxy, and other native
  boundaries share the Completion conversion contract. Host callbacks can use
  public `Error::javascript(value)` to throw intentionally, while Runtime and
  ResourceLimit errors still bypass JavaScript catch;
- `Error::with_context` never rewrites a thrown JavaScript value. CLI and
  runner diagnostics format the VM-local error at their outer boundary rather
  than requiring it to be `Send + Sync` through `anyhow`;
- the architecture guard rejects string-formatted throws, message-prefix
  classification, including raw `ReferenceError:` construction, a moved
  ReferenceError owner, or another exception bridge;
- six focused public tests cover the embedding result, object/Symbol identity
  through native frames, ReferenceError metadata, an explicit host throw,
  forged error text, and resource-limit non-catchability;
- the Test262 runner matches negative runtime cases by the typed JavaScript
  error name instead of accepting formatted `Runtime` text. The migration also
  found and removed the three remaining assignment/update paths that created
  textual ReferenceErrors;
- the complete local correctness gate preserves every prior expected variant
  and adds 332 reviewed negative/error variants. The expected-pass baseline
  and full pass set are now 36,553 of 102,578, with QuickJS differential
  unchanged at 95 of 95. The local evidence is
  `target/rsqjs-reports/test-runs/rsqjs-test-report-20260710T113234Z.*`.

AS-04a completion evidence:

- PR #416 was squash-merged as `d9ae782`; required CI run `29089996777`
  certified exact tree `f2513f994b95f221abad7cbb4b4e0beb884d95c2` at
  36,553/36,553 expected Test262 variants and 95/95 QuickJS cases;
- post-merge run `29090177028` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T114057Z.*` in
  report-only commit `9cf86ef`.

AS-04b1 local implementation evidence:

- `Value::Error(ErrorObject)` and every synthetic Error dispatch branch are
  deleted. Built-in Error instances use `Value::Object(ObjectId)` and ordinary
  property, descriptor, key, prototype, integrity, equality, JSON, and host
  paths;
- `Object.error_metadata` is the single internal slot for stable built-in class
  and creation-message diagnostics. The JavaScript-visible `name` and `message`
  remain ordinary mutable properties, while `Error::JavaScript` preserves the
  thrown VM-local object and exposes structured metadata;
- typed exception requests allocate their Error object in the active VM before
  entering `Completion::Throw`. Runtime, parser, host, and resource-limit
  failures remain outside JavaScript catch;
- Error construction honors both intrinsic and explicit `newTarget`
  prototypes. Five focused tests cover distinct identity, catch round trips,
  ordinary descriptors/mutation, prototype/integrity operations, new-target
  construction, and public structured metadata;
- the architecture guard fixes the twelve-variant `Value` representation,
  owns the new Object slot, and rejects any return of `Value::Error` or
  `ErrorObject`;
- the complete engine and 112-test runner suites, strict Clippy, documentation,
  and guard self-tests pass. The complete corpus preserves all 36,553 prior
  variants and adds 106 reviewed passes, bringing the baseline to 36,659 of
  102,578 with QuickJS unchanged at 95 of 95. Local evidence is
  `target/rsqjs-reports/test-runs/rsqjs-test-report-20260710T120828Z.*`.

AS-04b1 completion evidence:

- PR #418 was squash-merged as `e00884d`; required CI run `29091906917`
  certified exact tree `83908367ca8b17732931ff97d11a5e45257f2ec0` at
  36,659/36,659 expected Test262 variants and 95/95 QuickJS cases;
- post-merge run `29092104706` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T121755Z.*` in
  report-only commit `8f20df9`.

AS-04b2a local implementation evidence:

- public `SourceId` deterministically identifies the framed optional source
  name and source bytes. It is stable across repeated compilation and VMs but
  explicitly does not claim VM ownership or collision-free security identity;
- public `SourceSpan` is one validated half-open byte range. `Error::Lex` and
  `Error::Parse` carry it directly, and `Error::source_span` exposes it without
  requiring formatted-message parsing;
- `CompiledScript` retains only `SourceId` plus the optional bounded source
  name, not source text or parser AST. `Runtime`, `Context`, and `Vm` expose
  `compile_named`, while the existing anonymous API remains deterministic;
- frontend diagnostics bind their offset to the compiling source, expand an
  offending UTF-8 scalar to its byte range where available, and preserve the
  same range through `Error::with_context`;
- focused public tests cover deterministic/different identities, named and
  anonymous compilation, cross-VM reuse, UTF-8 lexer ranges, EOF parser points,
  contextual errors, source-name limits, and public span validation;
- the architecture guard fixes the source metadata owners, prevents source or
  AST retention in `CompiledScript`, and rejects a return to offset-only
  lexer/parser diagnostics. AS-04b2b2 will extend this same boundary with a
  parallel bytecode span table rather than retaining the AST at runtime;
- the complete local correctness gate preserves all 36,659 expected Test262
  variants and the exact 36,659 of 102,578 full pass set, with QuickJS
  differential unchanged at 95 of 95. Local evidence is
  `target/rsqjs-reports/test-runs/rsqjs-test-report-20260710T123222Z.*`.

AS-04b2a completion evidence:

- PR #419 was squash-merged as `a3d9af6`; required CI run `29093164392`
  certified exact tree `39fbddffa9891f567a1ea0f97d74fbd048816a55` at
  36,659/36,659 expected Test262 variants, the exact 36,659 of 102,578 full
  pass set, and 95/95 QuickJS cases;
- post-merge run `29093382601` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T124126Z.*` in
  report-only commit `9ba7953`.

AS-04b2b1 local implementation evidence:

- every lexer token carries one canonical `SourceSpan`, including the EOF
  point, and parser errors preserve the complete offending-token range rather
  than reconstructing a one-byte location;
- `Expression` and `Statement` are span-bearing `AstNode<Expr>` and
  `AstNode<Stmt>` values. Recursive expression and statement fields retain
  those nodes, so compound expressions, member/call chains, functions,
  classes, object/array literals, and structured control flow cover their
  complete parsed source ranges;
- binding analysis and bytecode compilation consume the node kind while
  retaining its range at the frontend boundary. `CompiledScript` still retains
  neither source text nor AST, and no duplicate source map is introduced;
- the architecture guard fixes the token-span owner, the two AST aliases,
  parser root return types, and compiler inputs. Its mutation self-test proves
  that removing the span-bearing AST boundary is rejected;
- touching the legacy lexer also completed its mechanical split: operator
  scanning now lives in `src/lexer/scanner/operators.rs`, and every touched
  Rust source remains below the 800-line limit;
- the complete engine test suite, 118-test runner suite, strict Clippy,
  documentation, architecture self-tests, and touched-file size gate pass;
- the complete local correctness gate preserves all 36,659 expected Test262
  variants and the exact 36,659 of 102,578 full pass set, with QuickJS
  differential unchanged at 95 of 95. Local evidence is
  `target/rsqjs-reports/test-runs/rsqjs-test-report-20260710T131658Z.*` for
  tested tree `564c29b62f27f1feefd07b2474c74cd709ba9f28`.

AS-04b2b1 completion evidence:

- PR #420 was squash-merged as `6f887c2`; required CI run `29095778077`
  certified exact tree `72e3f0562974e828593aa52db93b42761fc78a2a` at
  36,659/36,659 expected Test262 variants, the exact 36,659 of 102,578 full
  pass set, and 95/95 QuickJS cases;
- post-merge run `29096015664` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T132547Z.*` in
  report-only commit `e380c19`.

AS-04b2b2 local implementation evidence:

- `BytecodeBlock` owns one `Rc` instruction array and one equally sized `Rc`
  `SourceSpan` array. Construction rejects count mismatches, unknown source
  identities, and ranges from multiple sources;
- `BytecodeCompiler` carries the current AST node range and `emit` appends an
  instruction and range together. Nested control-flow blocks, functions,
  default parameters, class fields, patterns, and expression blocks use the
  same construction path without retaining source text, tokens, or AST;
- normal execution consumes a paired instruction/span step. Linear plans carry
  one range per lowered operation, including fused peepholes and direct paths,
  so optimization does not create a second diagnostic map;
- Runtime, JavaScript, and resource-limit channels preserve their existing
  catchability and value identity while exposing an optional structured span.
  Built-in Error metadata keeps the first origin across function frames, and
  formatted error text remains unchanged;
- cold JavaScript diagnostic payloads and their optional ranges are boxed, so
  adding source locations does not enlarge every engine `Result` beyond the
  strict `result_large_err` limit;
- five focused embedding tests cover an executing ReferenceError identifier,
  host Runtime and resource-limit call sites, a primitive throw statement, and
  a nested Error origin. Bytecode, quickening, completion, and source-focused
  tests pass together with strict Clippy;
- the architecture guard fixes the two `BytecodeBlock` fields, the compiler
  span owner, atomic emit, the two runtime execution owners, and structured
  Error span fields. Its mutation self-test rejects removal of the bytecode
  side table;
- the complete engine suite, 118-test runner suite, strict Clippy,
  documentation, architecture mutation self-tests, and touched-file size gate
  pass;
- the complete local correctness gate preserves all 36,659 expected Test262
  variants and the exact 36,659 of 102,578 full pass set, with QuickJS
  differential unchanged at 95 of 95. Local evidence is
  `target/rsqjs-reports/test-runs/rsqjs-test-report-20260710T134839Z.*` for
  tested tree `cbf27db19721ea28dfaf073c11819792ab389647`.

AS-04b2b2 completion evidence:

- PR #421 merged as `13b0bbe` after required CI run `29097737538`
  certified exact tree `461b2229cf8379255261841630f065707725cd81`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29097960654` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T135712Z.*` in
  report-only commit `88a2f52`.

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

AS-05 is split at ownership boundaries:

1. AS-05a1 removes ambiguous `Vm`/`Context` cloning and establishes one opaque
   capability identity plus an explicit storage generation for every Context.
   It does not claim that raw public `Value` is safe to transfer yet;
2. AS-05a2a stamps the VM-derived primitive values already accepted by host
   returns (`HeapString` and `Symbol`) and rejects a foreign owner before a
   colliding slot can be used;
3. AS-05a2b introduces callback-borrowed `LocalValue`, binds public JavaScript
   errors to its VM owner, and rejects a foreign throw before it becomes
   catchable;
4. AS-05a2c defines the VM-independent `OwnedValue` primitive subset and
   explicit copying from callback-local/evaluation values;
5. AS-05b1a establishes the executable direct-root categories and enumerates
   stored engine, binding, active-call, and queued-job roots;
6. AS-05b1b1 defines the shared typed strong-edge visitor and enumerates
   JavaScript, native, and bound function stores;
7. AS-05b1b2 enumerates object properties, prototypes, accessors, Proxy state,
   typed views, and other object internal slots;
8. AS-05b1b3 enumerates Promise, collection, and iterator associations and
   classifies weak collection keys without adding a collector;
9. AS-05b1c closes transient allocation-point and embedder-root gaps, with
   AS-06 remaining responsible for replacing Rust-stack execution state with
   durable activation frames;
10. AS-05a2d adds retained object/function handles and explicit release against
   the root registry;
11. AS-05b2a adds complete logical per-owner counts and a teardown report that
   reconciles every current VM store;
12. AS-05b2b adds logical retained payload bytes to the same owner map;
13. AS-05b2c1 defines the public limit policy and enforces payload-bearing and
    top-level atom/string/Symbol/object/callback/output/source owners;
14. AS-05b2c2 enforces callable, binding, property, and cache owners;
15. AS-05b2c3 enforces asynchronous, root, frame, and association owners and
    closes the full growth-point reconciliation gate.

AS-05a1 local implementation evidence:

- `Vm` and `Context` no longer implement `Clone`; independent VM construction
  cannot copy indexed stores while sharing binding cells, buffers, callbacks,
  or metadata;
- `VmIdentity` uses a private `Rc` owner capability rather than mutable global
  numbering. Independently created identities cannot alias, while a clone
  keeps the owner token alive and prevents accidental reuse;
- `VmGeneration` is an explicit part of the identity contract. The current
  append-only stores create one non-reused generation per VM; future reset or
  slot reuse must advance or replace it before a stale handle can validate;
- `Vm::identity` and `Context::identity` expose the same opaque identity for
  embedding diagnostics without exposing a forgeable numeric id;
- focused tests cover independent VM and Runtime context identities, shared
  Vm/Context ownership, identity cloning, and initial generation behavior;
- the architecture guard fixes the new Context owner field and capability /
  generation representation, and rejects reintroducing `Clone` on either
  public VM owner;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b2a completion evidence:

- PR #432 merged as `afcbe6b` after required CI run `29110640594`
  certified exact tree `45a63d62baafe68c5746633f5dffa0078f5cc504`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29110845591` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T172423Z.*` in
  report-only commit `0132afa`.

AS-05b2b local implementation evidence:

- `VmStorageSnapshot` keeps independent fixed per-kind arrays and checked
  totals for logical records and variable-size payload bytes. The original
  count semantics remain unchanged;
- payload bytes have a stable cross-platform meaning: owned UTF-8 length and
  raw buffer length, excluding allocator headers, pointer width, fixed record
  layout, spare capacity, and duplicate references to shared payload;
- the current non-zero sources are canonical atom names, interned heap-string
  text, host callback names, RegExp pattern/flag text, `ArrayBuffer` backing
  bytes, captured output text, and retained Function-constructor source;
- fixed-size bindings, functions, properties, collections, Promise records,
  roots, frames, caches, associations, and future modules remain represented
  by their complete counts and therefore report zero payload bytes unless
  they later acquire directly owned variable data;
- immutable `CompiledScript`/bytecode data remains governed by
  `CompiledScriptUsage` rather than being multiplied for every VM reference.
  Opaque Rust callback captures remain embedder-owned and are bounded by the
  host-callback count, not guessed by the engine;
- atom text bytes are maintained with checked arithmetic at the unique intern
  point. Other byte sources are scanned only by the explicit diagnostic
  snapshot, so evaluation and callback paths gain no hidden full-heap scan;
- focused tests prove an all-zero fresh map, exact category and byte sums,
  exact host/RegExp/buffer/output payloads, retained-handle release, and exact
  preview/finish reconciliation;
- the architecture guard fixes both compact arrays, both checked totals, and
  every current payload source. Its mutation suite rejects removal of buffer
  byte accounting or the checked atom-byte accumulator;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b2b completion evidence:

- PR #433 merged as `1c3dee8` after required CI run `29111664726`
  certified exact tree `73a737a6189ba5cdb4f80931c4b6ea40724208f4`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29116932748` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T190739Z.*` in
  report-only commit `7ee0441`.

AS-05b2c1 local implementation evidence:

- `VmStorageLimits` maps independent hard record and payload-byte limits onto
  the same twenty-six stable `VmStorageKind` categories. Its unlimited default
  preserves existing behavior, while immutable custom tables are shared by
  cloned engine/VM configuration through `Arc`;
- `RuntimeLimits`, `EngineConfig`, and `VmConfig` are cloneable rather than
  `Copy`. This prevents a 416-byte owner table from being copied through every
  parser/runtime call while keeping unlimited policies allocation-free;
- atom, heap-string, Symbol, host-callback, object, byte-buffer, output, and
  Function-constructor source growth checks both projected record counts and
  logical payload bytes before committing the owning store mutation;
- `ObjectHeap::push_object` is now the single append boundary for every object
  constructor. It maintains exact RegExp/buffer totals, charges a buffer only
  at its owner object rather than through typed views, and combines legacy
  object limits with the new per-owner policy;
- output payload bytes are maintained incrementally and reset by
  `take_output`; atom/string/object payload accounting likewise uses existing
  or new O(1) counters, so no hard-limit path introduces an O(heap) scan;
- rejected growth leaves the limited owner category unchanged. Other earlier
  side effects in the same JavaScript evaluation remain governed by their own
  categories rather than pretending the entire evaluation is transactional;
- four direct embedding tests cover count and byte rejection, exact owner
  stability, output release/reuse, independent VM policies, and all eight
  enforced owner categories;
- the architecture boundary guard fixes the public policy seam, the single
  object insertion boundary, and every AS-05b2c1 owner check. Three new
  mutations prove that atom payload, byte-buffer insertion, and output-release
  accounting cannot disappear unnoticed;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b2c1 completion evidence:

- PR #434 merged as `db704bd` after required CI run `29118568725`
  certified exact tree `61535178e7f5411842938e5cbcb6b1b3579a40bf`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29118808176` measured five of five valid sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T194046Z.*` in
  report-only commit `213f350`.

AS-05b2c2 local implementation evidence:

- one VM-local `VmStorageLedger` provides O(1) projected count checks across
  stores that share a `VmStorageKind`; it uses `Rc` only within one VM and
  never introduces process-global mutable state or cross-VM sharing;
- bindings activate and release their ledger charges with lexical/function
  scopes, while closure upvalues remain charged with their owning function;
- JavaScript, native, and bound function creation reserves callable records
  before arena growth, including native-registry rollback and bound-function
  rollback when the companion native callable cannot be created;
- ordinary objects, arrays, JavaScript functions, and native functions share
  the global `ObjectProperty` budget. Named, dense, packed, descriptor, delete,
  and pop paths reserve or release exact deltas without scanning the heap;
- atom/string indices, binding indices, shape layouts, function metadata,
  property-order indices, well-known/descriptor keys, static evaluation
  caches, class fields, and the native registry share the global `CacheEntry`
  budget;
- every public storage snapshot independently recounts the six AS-05b2c2
  categories and rejects any ledger drift, so the enforcement mechanism cannot
  silently diverge from the AS-05b2a owner contract;
- four additional direct tests cover rejection for all six categories,
  lexical binding release, object/function property delete-and-reuse, native
  registry rollback, and zero-budget cache materialization. The complete
  engine test suite and strict Clippy pass locally;
- the architecture guard fixes the ledger owner, six-category reconciliation,
  binding/callable activation, property release, and shape/static cache seams.
  Four new mutations prove that reconciliation, scope release, property
  release, and shape-cache charging cannot disappear unnoticed;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b2c2 completion evidence:

- PR #435 merged as `5729920` after required CI run `29120888639`
  certified exact tree `f4388da04b798621e346ae300c0efe02661dcf80`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29121083477` measured five of five valid sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T202058Z.*` in
  report-only commit `e3f47ef`. The function-call median moved from 145.41 ms
  to 155.18 ms and remains an explicit trend check for the next canonical run.

AS-05b2c3 local implementation evidence:

- collection stores charge `Collection` and `CollectionEntry` at creation and
  insertion, release deleted or cleared entries, and charge iterator records
  plus their materialized item snapshots before publication;
- Promise stores charge records, pending reactions, and queued jobs at their
  exact ownership transitions. Settling transfers the reaction charge to the
  queue, while draining releases each queued owner before execution;
- retained and transient roots share the same VM-local ledger as their RAII
  registries. Explicit release remains checked, and destructor cleanup uses a
  non-panicking decrement whose invariant is independently verified by every
  storage snapshot;
- one execution-storage boundary charges lexical scopes, function frame bases,
  upvalue frames, `this`, `new.target`, super state, and temporary class-field
  receivers, with rollback and release on every existing exit path;
- object/array prototype anchors, global and Promise anchors, Symbol registry
  entries, collection/Promise object slots, and the iterator Symbol now share
  the global `Association` budget. `Module` remains an explicit zero-growth
  category until a module store exists;
- every storage snapshot reconciles all eighteen ledger-backed categories and
  independently verifies all twenty-six count and payload categories against
  the configured policy;
- five additional integration tests cover every remaining owner category,
  collection delete-and-reuse, Promise reaction-to-job transfer and drain,
  retained-handle Drop/reuse, transient cleanup, execution-frame rollback,
  association rejection, and zero module storage;
- the complete engine integration suite passes. The architecture guard and
  all mutation self-tests cover full-policy reconciliation, collection
  release, Promise job growth, transient Drop release, execution frames, and
  association anchors;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b2c3 completion evidence:

- PR #436 merged as `4a3afaf` after required CI run `29122001416`
  certified exact tree `e6ef603b330346dbbe27923570801d3b3bfa2e2b`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29122207039` measured five of five valid sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T204151Z.*` in
  report-only commit `e8f11fe`. Function-call latency improved from the prior
  155.18 ms to 151.76 ms; all other sentinel changes stayed within 1.7%.

AS-06a1 local implementation evidence:

- one `ActivationFrame::Call` owns the local-scope base, captured upvalue
  frame, `this`, `new.target`, and optional super binding for a synchronous
  JavaScript invocation. `Context` no longer maintains five parallel vectors
  whose lengths and unwind order could diverge;
- temporary class-field `this` state uses an explicit activation variant, and
  generated Function-constructor evaluation uses a boundary variant that
  hides caller lexical/new-target/super state without temporarily removing its
  direct roots from the VM;
- binding lookup, lexical function creation, current `this`/`new.target`/super
  access, root enumeration, and storage accounting read through the activation
  owner rather than reconstructing a call from parallel stacks;
- `ExecutionFrame` accounting now charges one logical call activation plus
  each active lexical scope. Nested-call limit coverage proves a two-call
  stack fits exactly four records, rejects a three-record budget, and releases
  every record on unwind;
- nested root snapshot coverage observes both active call records, and the
  complete engine integration suite preserves Function-constructor isolation,
  binding layouts, class fields/inheritance, transient roots, and storage
  reconciliation;
- AS-06a2 still owns bytecode program counters, operand stacks, and structured
  loop/try/finally continuation records. AS-06a1 deliberately does not claim
  that the current recursive bytecode executor can suspend yet;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-06a1 completion evidence:

- the initial correctness run `29123071604` was superseded after `main` moved;
  the branch was rebased before merge rather than relying on a stale green
  merge tree;
- PR #438 merged as `63bfb88` after refreshed required CI run `29123294076`
  certified exact tree `c9b3275e9d46fa72af2ebe2ae3b5ba03e89ba5d2`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29123493389` measured five of five valid sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T210337Z.*` in
  report-only commit `b51cb6d`. Relative to the immediately preceding report,
  changes ranged from -2.8% to +2.7%; the 155.01 ms function-call median
  remains a trend check rather than a one-run rollback trigger.

AS-06a2a local implementation evidence:

- `BytecodeContinuationFrame` owns a stable function id or immutable
  `BytecodeBlock` program key plus optional parked `BytecodeState` containing
  the program counter, operand stack, and last value. Synchronous execution
  keeps running state in the driver and unwinds the frame on every outcome; a
  future suspended outcome can instead park that state without changing the
  owner model;
- a call, temporary-this, or evaluation-boundary activation owns its current
  continuation directly. Top-level or genuinely nested blocks use an explicit
  bytecode activation, so the function-call hot path does not add a second
  vector owner or a second logical ExecutionFrame record;
- repeated loop/quickening segments that already receive a reusable
  `BytecodeState` continue to run on that control-owned state without cloning
  and pushing a continuation per iteration. AS-06a2b must move those loop and
  try/finally owners into explicit durable control records;
- active function program ids and parked continuation operands have a
  dedicated `VmRootKind::BytecodeFrame`; running operands remain covered by
  the existing transient operand scope.
  `ExecutionFrame` accounting charges standalone nested bytecode activations
  and reconciles them through the AS-05 ledger;
- nested calls now prove an exact five-record budget: one top-level bytecode
  activation, two call activations, and two function scopes. A four-record
  budget rejects and fully unwinds;
- focused generic, quickened, structured-control, root, transient-root, and
  storage tests pass. Guard mutations protect continuation state,
  unwind-on-outcome, program ownership, and parked operand roots;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 119 runner tests.

AS-06a2a completion evidence:

- PR #439 merged as verified `35d7c5c` after required CI run `29124448272`
  certified exact tree `7d2e69abc702f99ae93ee19e49ac55855d946c4f`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29124671670` measured five of five valid sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T212554Z.*` in
  verified report-only commit `90b06a4`;
- arithmetic, array-index, and property-read improved by 2.5% to 5.2%, while
  string-scan changed by +0.6%. Function-call moved from 155.01 ms to
  162.01 ms (+4.5%), so draft PR #440 owns an immediate measured follow-up
  rather than carrying possible continuation overhead into AS-06a2b.

AS-06a2a1 local implementation evidence:

- a clone-only experiment measured 163.21 ms against a paired 163.05 ms base
  and was rejected as ineffective. A detached pre-AS-06a2a control measured
  152.77 ms, confirming that the broader synchronous frame lifecycle was the
  material difference;
- call activations now use their stable `FunctionId` as the continuation
  program key. The function body executes against that already-owned record,
  so it neither clones a `BytecodeBlock` nor attaches, checks out, restores,
  and immediately removes another frame. General and top-level continuations
  still own an immutable block handle;
- running interpreter state remains with the synchronous driver under the
  transient-root contract. `parked_state` is reserved for a future suspended
  outcome, avoiding work that cannot currently survive the call. Active
  function ids are visited through the `BytecodeFrame` direct-root category;
- the final focused measurement is 155.29 ms with 0.5% CV: 4.8% faster than
  the paired 163.05 ms base, 4.1% faster than the canonical 162.01 ms report,
  and within 1.7% of the 152.77 ms pre-AS-06a2a control;
- focused function/default-parameter, bytecode, quickening, root, and
  execution-limit tests plus the full architecture mutation suite pass;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 119 runner tests.

AS-06a2a1 completion evidence:

- PR #440 merged as verified `544a7d6` after required CI run `29126038777`
  certified exact tree `a9d832d98d48b166e28d00dbca135b0f68a40aaa`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29126229568` published
  `reports/test-runs/rsqjs-test-report-20260710T215609Z.*` in verified
  report-only commit `8934889`;
- canonical function-call latency returned from 162.77 ms to 155.36 ms
  (-4.6%). The other sentinel deltas were mixed across independent paths and
  remain trend evidence rather than a reason to reverse the targeted change.

AS-06a2b local implementation evidence (draft PR #442):

- one continuation-owned stack holds typed loop, `for-in`, `for-of`, switch,
  and try/catch/finally records. Each record carries its current phase,
  reusable segment states, accumulated completion value, cursor or iterator
  source, and pending abrupt completion where applicable;
- the synchronous driver checks a record out once for the complete construct
  and mutates it in place. Running carried values use scoped transient roots;
  parked records are traced through `VmRootKind::BytecodeFrame`. No record is
  allocated, moved, or pushed per iteration;
- structured records are charged as `ExecutionFrame` owners, checked during
  activation unwind, and included in independent storage reconciliation.
  Focused tests prove pending-value roots, exact control-frame limits, nested
  rejection, and complete release after errors;
- generic loop, switch, iterator-close, destructuring, catch/finally, labels,
  and fast-path semantics pass. Architecture mutations protect the control
  owner, in-place lifecycle, roots, accounting, and unwind contract;
- an initial per-segment checkout design regressed the arithmetic sentinel to
  128.76 ms and was rejected. The in-place design measures 82.91 ms at 1.6%
  CV against a paired 83.10 ms `origin/main` control. The final five-sentinel
  run is valid throughout: arithmetic 79.32 ms, array-index 2.33 ms,
  property-read 224.59 ms, function-call 150.26 ms, and string-scan 70.75 ms;
  relative to the latest canonical report, changes range from -7.2% to +2.1%.

AS-05a1 completion evidence:

- PR #422 merged as `4143ec4` after required CI run `29098691127`
  certified exact tree `a64ce722ad7d50c2f9d2cea1dcc05419dec4ba77`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29098932099` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T141238Z.*` in
  report-only commit `064c12b`.

AS-05a2a local implementation evidence:

- the ownership types move to the crate ownership layer so VM stores can use
  them without depending back on `runtime`;
- `StringHeap` and `SymbolTable` own the Context identity. Every emitted
  `JsString` and `JsSymbol` retains that capability, preventing owner-token
  reuse while the local value is alive;
- `VmGeneration` is stored once inside the shared owner token, keeping each
  local primitive stamp to one `Rc` word rather than enlarging hot `Value` and
  bytecode representations with duplicated generation data;
- string text/owner and Symbol description/owner live behind that existing
  handle word. A layout regression test keeps `JsString`, `JsSymbol`, and
  `Value` no larger than the owned-String baseline after an initial CI build
  exposed release Test262 stack exhaustion from a wider inline stamp;
- `Context::checked_value` verifies owner identity before looking up a string
  or Symbol slot. Colliding numeric ids from another VM therefore cannot alias
  a valid local entry;
- host return validation preserves same-VM strings/Symbols and reports foreign
  ownership with host-function context;
- three focused host tests cover same-VM round trips and foreign string/Symbol
  returns with deliberately colliding slots. The compact-layout test, existing
  string and Symbol suites, and strict Clippy pass;
- the architecture guard fixes all four primitive owner fields, both central
  checks, and a mutation test that removes a primitive identity;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05a2a completion evidence:

- initial required run `29099643637` exposed release Test262 worker stack
  exhaustion from the first wider inline owner representation; the corrected
  compact representation is guarded by a layout test;
- PR #423 merged as `923988b` after corrected required CI run `29099970843`
  certified exact tree `cd4dad8763be5082d433d8b3a130757b1df0723f`;
- the corrected corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29100213566` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T143244Z.*` in
  report-only commit `49f8405`.

AS-05a2b local implementation evidence:

- `HostCall::value` and `required_value` return a borrowed `LocalValue` that
  carries the active `VmIdentity` beside the borrowed raw `Value`;
- the public way to request an arbitrary JavaScript throw from a host callback
  is `LocalValue::javascript_error`, so the owned `Error::JavaScript` retains
  the source VM capability. Evaluation-produced JavaScript errors are stamped
  by the same Context boundary;
- cold thrown identity/value payloads share one box, avoiding growth of the
  common engine `Result` representation;
- internal completion conversions may create a crate-private unbound throw
  while it remains inside one runtime call. Every public evaluation result and
  host-requested throw is bound before crossing the embedding boundary;
- `runtime_exception_value` rejects a bound foreign owner before returning its
  raw Value to JavaScript, so a colliding `ObjectId` cannot alias a target-VM
  object or become catchable;
- focused tests preserve same-VM arbitrary object identity, verify public eval
  owner metadata, and reject a foreign host error with deliberately colliding
  object slots. Host, completion, and source-diagnostic suites plus strict
  Clippy pass;
- the architecture guard fixes the LocalValue and HostCall identity fields,
  the boxed error payload, the conversion/validation calls, and a mutation
  test that removes the local owner or makes the exception identity forgeable;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05a2b completion evidence:

- PR #424 merged as `da7c7c4` after required CI run `29101033535`
  certified exact tree `ab3d44ac572d86314645b03b79ad34911ee5c69f`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29101277015` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T144925Z.*` in
  report-only commit `eabb1b5`.

AS-05a2c local implementation evidence:

- `OwnedValue` contains only undefined, null, Boolean, Number, and owned Rust
  String variants. It contains no VM identity, id, Symbol, object, or function
  representation;
- `TryFrom<Value>` moves portable data, while `TryFrom<&Value>` and
  `LocalValue::to_owned_value` copy heap-string text out of its VM. Every
  non-portable variant returns one typed Runtime error;
- `Context` and `Vm` expose `eval_owned` and `eval_compiled_owned`, letting
  embedders request a result that is statically independent of VM lifetime;
- `OwnedValue` implements the typed host return conversion. A focused test
  destroys the source VM, moves its owned string into a second VM callback,
  and observes the same string without an owner token;
- six focused tests cover all portable variants, compiled evaluation,
  callback-local copying, cross-VM movement, reverse primitive conversion, and
  rejection of Symbol/object/function values. Host and embedding suites plus
  strict Clippy pass;
- the architecture guard fixes the five-variant portable enum and requires
  local-copy, evaluation, and typed-host conversion entrypoints. Its mutation
  test rejects adding a VM-local Symbol variant;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05a2c completion evidence:

- PR #425 merged as `99abdb2` after required CI run `29101824784`
  certified exact tree `a17282945949754347e37edad8d522706dbcd45a`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29102069741` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T150132Z.*` in
  report-only commit `dd71e28`.

AS-05b1a local implementation evidence:

- `VmRootKind` initially defined nine durable direct-root categories: global, builtin,
  local, and captured bindings; active `this`, `new.target`, and `super`;
  queued jobs; and runtime anchors;
- one internal `DirectRootVisitor` walks initialized `BindingCell` values,
  active call vectors, the global object and Promise prototype anchors, and
  every result Promise id, handler, and settled value retained by queued
  Promise jobs;
- `VmRootSnapshot` reports checked per-category reference counts and a checked
  total. Counts intentionally include duplicate references; a future marker,
  not the root registry, owns heap-identity deduplication;
- `Context::root_snapshot`, `Vm::root_snapshot`, and the snapshot captured on
  `HostCall` make the contract executable and testable without exposing raw
  trace internals or adding a collector;
- four focused tests prove an empty fresh VM, stable category totals, globals
  and builtins, local and captured cells, active `this`/`new.target`/`super`,
  runtime anchors, queued Promise values, and complete removal of transient
  call/job categories after evaluation;
- the architecture guard fixes the original nine categories and every current Context
  source, requires Promise-job and public snapshot entrypoints, and includes a
  mutation test that removes an active root source;
- AS-05b1a deliberately does not classify values stored behind object,
  function, Promise, collection, or iterator arenas as direct roots. AS-05b1b
  owns those strong trace edges; AS-05b1c and AS-06 own transient operand,
  native-call-argument, retained-handle, and durable activation roots;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b1a completion evidence:

- PR #426 merged as `ce44f0b` after required CI run `29103081648`
  certified exact tree `c4722165242e43342e8bc5c45ae5de14588c1f37`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29103302187` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T152044Z.*` in
  report-only commit `07f4cd1`.

AS-05b1b1 local implementation evidence:

- one generic `StrongEdgeVisitor<Kind>` receives typed `Value`, function,
  object, Promise, bound-function, collection-iterator, and property-key
  targets without exposing their forgeable arena ids publicly;
- `VmCallableEdgeKind` separates JavaScript upvalues, properties, and internal
  slots from native properties/internal ids and bound-function payloads;
  `VmCallableEdgeSnapshot` reports checked physical-slot counts and explicitly
  does not claim reachability or identity deduplication;
- JavaScript functions enumerate initialized captured cells, intrinsic and
  custom property keys/values, `super` state, static parents, class-field
  keys, and lexical `new.target`;
- native functions enumerate the same property storage plus bound-function,
  collection-iterator, Promise-resolver, and Proxy-revoke ids. A mechanical
  payload allowlist catches any future id-bearing native kind that lacks a
  reviewed edge rule;
- bound functions enumerate their target, bound `this`, and every bound
  argument;
- `native_function_registry` ids are added to direct `RuntimeAnchor` roots.
  They are Context-owned reuse anchors, not edges owned by a native-function
  payload;
- four focused tests cover empty/stable totals, closures, custom properties,
  class/super/new-target state, all four current native id payload families,
  bound values, registry roots, and preserved evaluation behavior;
- architecture mutation tests reject removal of a bound argument edge or the
  native-registry direct root. AS-05b1b2 and AS-05b1b3 subsequently completed
  object and asynchronous arena traversal;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b1b1 completion evidence:

- PR #427 merged as `5c42a71` after required CI run `29104239314`
  certified exact tree `e2f15ef12d4ece946289da4a26a5ff0127f8ebc0`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29104449632` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T153859Z.*` in
  report-only commit `b88e9d9`.

AS-05b1b2 local implementation evidence:

- `VmObjectEdgeKind` separates property, prototype, and typed internal-slot
  edges; the checked `VmObjectEdgeSnapshot` is explicit that Context Promise
  and collection associations remain outside this slice;
- named object properties enumerate their keys plus data values or both
  accessor halves. Dense packed/holey array descriptors and sparse-key maps
  use the same property visitor;
- every object prototype, boxed heap string/Symbol, Proxy target/handler, and
  typed-array buffer-object link is a typed strong edge. Rust-owned Error,
  Date, RegExp, and byte-buffer payload data contain no additional JS arena
  ids;
- the shared edge target gains borrowed `JsString` and `JsSymbol` variants,
  keeping string/Symbol storage typed rather than encoding owner-local ids;
- ObjectHeap's cached Object/Array prototypes and every physical shape key are
  direct `RuntimeAnchor` roots. Context iterator-symbol, well-known-property,
  and descriptor-property caches use the same property-key root path;
- four focused tests cover stable totals, named data/accessor and dense/sparse
  array properties, prototypes, boxed primitives, Proxy state, typed-array
  buffer links, shape/cache roots, and preserved behavior;
- architecture mutation tests reject removal of a typed-array internal edge
  or shape-key root. Promise, collection, iterator, and weak-edge associations
  were deliberately deferred to AS-05b1b3;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b1b2 completion evidence:

- PR #428 merged as `778fe2a` after required CI run `29105311375`
  certified exact tree `960dce6eae960abf587d029ce4efef9be50b8377`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29105524565` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T155616Z.*` in
  report-only commit `cae131d`.

AS-05b1b3 local implementation evidence:

- `VmAsyncEdgeKind` separates Promise state/reactions, Promise and collection
  object associations, ordinary collection entries, iterator items, weak
  collection keys, and ephemeron pairs. Every category has an explicit
  `Strong`, `Weak`, or `Ephemeron` classification;
- Promise associations carry typed object and Promise ids. Settled values,
  pending reaction result ids, and both optional handlers are enumerated as
  strong edges, while queued jobs remain direct roots owned by AS-05b1a;
- collection associations carry typed object and collection ids. Each backing
  store now retains and validates its `CollectionKind`, so traversal does not
  infer weak semantics from an optional object-side association;
- Map/Set physical key/value slots and collection iterator snapshots are
  strong edges. WeakSet emits one weak key per entry, and WeakMap emits one
  key/value ephemeron pair; neither enters the ordinary strong-entry path;
- `VmAsyncEdgeSnapshot` exposes bounded category and strength counts without
  leaking arena ids or claiming that garbage collection is already implemented;
- four focused tests cover empty classification, Promise states/reactions,
  Map/Set entries and iterator items, WeakSet weak keys, WeakMap ephemerons,
  snapshot sums, and preserved runtime behavior;
- architecture mutation tests reject removal of a Promise reaction result or
  a WeakMap ephemeron source. AS-05b1c remains responsible for transient
  allocation-point and embedder roots;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b1b3 completion evidence:

- PR #429 merged as `74ef846` after required CI run `29106389366`
  certified exact tree `4a44f9220f4319503b8b21df0a3a0904a9a53114`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29106620598` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T161351Z.*` in
  report-only commit `73334c7`.

AS-05b1c local implementation evidence:

- `VmRootKind` adds `TransientOperand`, `TransientCall`, and
  `TransientTemporary`, extending the executable direct-root registry from
  nine durable categories to twelve total categories;
- one VM-owned `TransientRootRegistry` stores only arena-bearing Values behind
  `Rc<parking_lot::Mutex<_>>`. Scopes own checked ids, can accumulate values,
  and remove exactly their entries through `Drop`, including error or unwind
  exits. Primitive-only scopes remain inactive and take no registry lock;
- every bytecode instruction registers the traceable operand stack and last
  value before its allocation/call safepoint. Shared call and construct entry
  points register callee/constructor, receiver/new-target, and arguments;
  direct host dispatch registers its owned argument snapshot before producing
  `HostCall::root_snapshot`;
- protocol iterator records and returned iterator-result objects stay rooted
  across `next`, `done`, `value`, and `return` callbacks. Descriptor parsing
  accumulates getter, setter, and data Values while later descriptor getters
  run. Proxy own-key/descriptor results and key lists stay rooted during
  validation and nested traps;
- computed class keys and heritage remain covered after they are removed from
  `BytecodeState`, because the instruction scope snapshots operands before
  dispatch;
- six focused re-entrant host tests observe operand, call, iterator,
  descriptor, Proxy-result, and class-key roots, then prove exact cleanup on
  both success and host error;
- the future collector may start only after the relevant instruction/call
  scope is installed. Raw allocation helpers are not collector safepoints.
  AS-06 replaces this bridge with durable activation frames;
- at the AS-05b1c boundary, opaque Rust callback captures and raw Values
  retained after an embedding call could not be inspected safely. AS-05a2d
  follows with identity-stamped retained handles and explicit release;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05b1c completion evidence:

- PR #430 merged as `ffb2102` after required CI run `29107804152`
  certified exact tree `d3494ad5433f46abc39bb66d826ecb009cc71a1a`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29108015179` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T163712Z.*` in
  report-only commit `602e65e`.

AS-05a2d local implementation evidence:

- `RetainedValue` is a non-cloneable VM-bound capability. It contains the
  opaque `VmIdentity`, a private weak registry capability, a private slot and
  checked slot generation, and release state; no arena or registry index is
  exposed to embedders;
- the VM registry reuses a vacant slot only after a checked generation
  increment. Handle resolution validates VM identity, the exact registry
  capability, slot generation, and active value before returning metadata or
  copying a portable primitive;
- `eval_retained`, `eval_compiled_retained`, `get_global_retained`, and
  `LocalValue::retain` create handles only at boundaries where the source VM
  is already known. There is deliberately no public `retain(Value)` operation
  that could relabel a foreign raw object/function id after the fact;
- `RetainedValue::release` consumes the handle and reports teardown or stale
  state. `Drop` performs a non-failing safety-net release, while the weak
  registry link avoids a VM/host-callback ownership cycle and makes Context
  teardown authoritative;
- `VmRootKind::RetainedHandle` extends the direct-root registry from twelve to
  thirteen categories. Every active retained value is visited by the same
  root snapshot used for bindings, jobs, runtime anchors, and transient
  execution values;
- raw `Value` results remain compatibility-only, non-durable values. Public
  documentation routes portable primitives through `OwnedValue` and every
  value retained across later VM calls through `RetainedValue`. A collector
  lane must keep legacy raw-result calls collector-disabled or remove them
  before reclaiming arena slots;
- six focused tests cover object/function roots, compiled/global/portable
  values, foreign VMs with colliding slots, callback-local retention,
  automatic release plus slot reuse, and release after VM teardown;
- the architecture guard fixes the handle fields, generation increment,
  source-proven constructors, owner validation, retained root source, and
  thirteen-category root map. Mutation tests reject removal of the slot
  generation or retained-root visit;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

AS-05a2d completion evidence:

- PR #431 merged as `30dfc0a` after required CI run `29109122937`
  certified exact tree `0f94732ed87357042eb028fd47c6f9e39daa35cf`;
- the required corpus preserved all 36,659 expected Test262 variants, the
  exact 36,659 of 102,578 full pass set, and 95 of 95 QuickJS differential
  cases;
- post-merge run `29109349272` measured all five project sentinels and
  published `reports/test-runs/rsqjs-test-report-20260710T165910Z.*` in
  report-only commit `c7f3ba8`.

AS-05b2a local implementation evidence:

- `VmStorageKind` defines twenty-six stable logical owner categories spanning
  atoms, heap strings, Symbols, bindings, callable stores, objects and
  properties, buffers, collections and iterator items, Promises and jobs,
  retained/transient roots, execution frames, output, caches, associations,
  modules, and retained function-constructor source records;
- `VmStorageSnapshot` uses a fixed private count array plus a checked total.
  Counts are logical records, not allocator blocks, unique reachable values,
  or bytes; AS-05b2b adds a distinct retained-byte dimension without changing
  these semantics;
- one Context-owned traversal records top-level stores and nested variable
  records: binding cells and indexes, function/native properties and metadata,
  object properties/buffers/shapes, collection entries/iterator items,
  Promise reactions/jobs, active retained/transient roots, caches, side-table
  associations, execution frames, output, and source records. Module storage
  remains an explicit zero category until modules are introduced;
- `Context::storage_snapshot` and `Vm::storage_snapshot` are explicit
  diagnostic operations. Ordinary evaluation and host-call paths do not scan
  storage, so accounting observability does not become hidden hot-path work;
- `Vm::teardown_report` previews the complete owner snapshot and
  `Vm::finish` consumes the VM, returning that same checked snapshot before
  deterministic Rust ownership releases it. Both now return `Result` rather
  than hiding a possible counter overflow;
- focused tests prove a fresh empty map, materialize every currently
  externally reachable owner category, verify exact category summation,
  release retained handles, and prove preview/finish snapshot reconciliation
  plus handle invalidation after teardown;
- the architecture guard fixes all twenty-six categories, the compact
  snapshot representation, every Context owner source, and the consuming
  teardown boundary. Mutation tests reject removal of a nested iterator owner
  or teardown snapshot;
- `RSQJS_BASE_REF=origin/main RSQJS_FAST_RUNNER=1 ./scripts/check-fast.sh`
  passes the complete engine suite, strict Clippy, documentation, architecture
  mutation self-tests, touched-file size checks, and all 118 runner tests.

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

AS-06b local implementation evidence (PR #445):

- `BytecodeOutcome` now distinguishes completed and suspended execution, and
  pending `await` never drains the job queue or consumes an unresolved value;
- one `SuspendedAsyncFunction` moves the existing lexical scopes and activation
  suffix into a typed await reaction. Settlement reattaches the same owners and
  resumes the same bytecode/control driver;
- continuation-owned cursors and recorded phases resume nested blocks, while,
  do/while, for, for-in, for-of, switch, catch, and finally without replaying
  completed iterations or introducing a parallel interpreter;
- parent bytecode states distinguish direct awaits from suspended children,
  and typed destructuring tasks retain property/default phases, consumed keys,
  and iterator sources without replaying observable side effects;
- suspend-only metadata and parked states use lazy boxed owners, while ordinary
  functions retain a const-specialized synchronous driver and compact operand
  root view;
- non-Promise, fulfilled, rejected, pending, repeated, and nested awaits use
  Promise jobs. Rejections re-enter as throw completions;
- `Context` and `Vm` expose `run_jobs`, `pending_job_count`, and `cancel_jobs`.
  Cancellation releases detached bindings, activations, control records,
  reactions, jobs, roots, and cache accounting; top-level await remains gated
  until an asynchronous evaluation API exists and fails without frame leakage;
- focused tests cover ordering, later settlement, repeated suspension,
  rejection, nested logical/pattern evaluation, structured control,
  cancellation, accounting reconciliation, and the top-level-await gate;
- architecture guards and their mutation self-tests cover the explicit
  outcome, cancellation release, rooted destructuring owner, and cold suspend
  boundary. The complete engine/runner fast gate passes with 119 runner tests;
- the reviewed full-corpus gate passes with 117/117 active fixtures,
  36,514/36,514 expected Test262 variants, 36,514 of 102,578 full variants,
  and 95/95 QuickJS differential cases. The pass-set refresh removes exactly
  151 module variants whose top-level `await` had previously completed through
  the invalid synchronous path, and adds six async-function variants for
  interleaved, monkey-patched-Promise, and non-Promise awaits;
- the four active async fixtures now execute their awaits inside async
  functions and verify the later Promise-job completion through deterministic
  host output. This preserves their coverage without pretending that the
  synchronous script API supports top-level await;
- exact-tree CI artifact evidence remains to be attached before merge.

AS-06b local performance checkpoint:

- a first paired run exposed an 8.8% function-call regression (166.42 ms
  against 152.90 ms), so the draft was not advanced to CI;
- moving suspend metadata behind lazy owners, keeping synchronous calls
  const-specialized, and separating hot operand roots from cold continuation
  roots reduced the final function-call result to 160.35 ms;
- the final adjacent branch/base medians are arithmetic 84.32/79.01 ms,
  array-index 2.57/2.37 ms, property-read 229.96/225.02 ms, function-call
  160.35/151.74 ms, and string-scan 70.34/68.97 ms. Every row is valid with at
  most 0.6% sample variation; all deltas remain below 10%, while property-read,
  function-call, and string-scan are within 5.7% of the paired base after the
  cold-path split.

### AS-07: Collection And Weak Semantics

Start with a safe non-moving collector over indexed arenas. The collector must
enumerate all roots, preserve host handles, integrate with jobs and suspended
frames, enforce hard heap limits, and produce deterministic teardown data.

WeakMap and WeakSet must stop retaining keys strongly. WeakRef and
FinalizationRegistry remain gated until collection order and callback/job
semantics are specified and tested.

AS-07a completion evidence (PR #446, merged as `62e2725`):

- `SlotArena<T>` provides safe non-moving storage with vacant slots for
  objects, callable payloads, Promises, collections, and iterators; heap
  strings and Symbols use equivalent sparse tables;
- `Vm::heap_reachability_snapshot` and `Vm::collect_garbage` expose an explicit
  embedder safepoint over the existing direct-root and typed-edge contracts;
- registered Symbols, native registry entries, runtime anchors, retained
  handles, queued jobs, transient values, and active or suspended execution
  state participate in marking;
- Map, Set, and iterator edges remain strong, WeakSet keys remain weak, and
  WeakMap values are admitted by an ephemeron fixed point before dead weak
  entries are physically removed;
- identity-bearing property, binding, and call caches are invalidated before
  an arena id can be reused, while object prototype versions and payload
  counters are reconciled after sweep;
- independent owner snapshots release exact ledger deltas, allowing repeated
  allocation, explicit collection, and slot reuse under configured hard
  storage limits;
- raw VM-local `Value` ids are explicitly non-durable across collection;
  retained handles are the supported embedder root boundary;
- focused library tests cover retained roots, ephemerons, registered Symbols,
  hard-limit reuse, suspended async survival and cancellation, heap strings,
  cache-id reuse, and isolation between VMs;
- atoms and shape metadata remain explicit cache roots for AS-08; WeakRef and
  FinalizationRegistry remain gated;
- the engine-wide all-target/all-feature suite, strict Clippy, architecture
  guard mutation tests, and the runner-enabled fast gate pass locally;
- adjacent branch/base sentinel medians are arithmetic 84.41/84.75 ms
  (-0.4%), array-index 2.53/2.44 ms (+3.7%), property-read 229.57/228.34 ms
  (+0.5%), function-call 158.98/158.41 ms (+0.4%), and string-scan
  71.64/70.34 ms (+1.8%). All rows are valid, checksums match, and branch
  variation is at most 1.1%; all rows are valid and checksums match;
- required CI run `29134794187` certified tree `697112ae` with 117/117 active
  fixtures, 36,514/36,514 expected Test262 variants, 36,514/102,578 full
  variants, and 95/95 QuickJS differential cases;
- post-merge run `29134943008` attempt 3 published the canonical report
  `reports/test-runs/rsqjs-test-report-20260711T013950Z.*` in report-only
  commit `8925145`.

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

AS-08a completion evidence from PR #447:

- `runtime/optimizer.rs` owns the VM-local mode and stable linear/call-cache
  counters; the previous scattered `Context` fields are removed;
- `VmConfig` selects enabled or disabled optional paths and
  `Vm::optimization_snapshot` exposes stable diagnostics;
- disabled mode bypasses direct binding operands, numeric/string quickening,
  dense-array bytecode shortcuts, static/call caches, direct native calls,
  linear plans, function/callback fast paths, and loop recognizers;
- optimizer-on/off tests compare values, output, and uncaught errors across
  numeric/array/string, binding/closure, call/property, and Proxy/completion
  clusters; disabled counters remain zero;
- compiler source-name handling and the `Print`/`AssertThrows` bytecode/runtime
  paths are deleted; `print` is an ordinary native binding and
  `assert.throws` is supplied by JavaScript test support;
- the architecture guard fixes the single optimizer-state owner and requires
  zero harness opcodes/source-name comparisons, with mutation self-tests;
- focused engine fixtures pass at 68/68, the active Test262 subset passes at
  117/117, and the runner-enabled fast gate is green;
- the reviewed full-corpus refresh raises the pass set from 36,514 to 37,721
  variants: 1,211 additions and four removals, for a net gain of 1,207. The
  four removed Map/WeakMap `getOrInsertComputed` callback cases were false
  positives: the removed opcode accepted a missing-method `TypeError` when the
  test required an exact `Error` constructor. Both local harnesses now require
  exact constructor identity, matching upstream Test262;
- refreshed local correctness passes at 37,721/37,721 expected variants,
  19,414/53,404 files, 37,721/102,578 full variants, and 95/95 QuickJS;
- adjacent branch/base sentinel medians are arithmetic 84.54/85.27 ms
  (-0.9%), array-index 2.57/2.50 ms (+2.8%), property-read 227.07/232.10 ms
  (-2.2%), function-call 158.83/159.06 ms (-0.1%), and string-scan
  72.67/71.04 ms (+2.3%). All rows are valid, checksums match, and branch
  variation is at most 0.9%;
- PR #447 squash-merged as `bc52a723`; recovery correctness run `29137393392`
  certified exact tree `7ee066e2`, post-merge run `29137348540` attempt 3
  passed performance and publication, and report-only commit `5e68fa5`
  published `reports/test-runs/rsqjs-test-report-20260711T030752Z.*`.

AS-08b completion evidence from PR #449:

- fifteen named control recognizer/executor modules are deleted together with
  compiler-generated catch and try/finally source-shape plans. More than 5,400
  lines of duplicate whole-loop semantics are removed;
- structured `for` and `while` always use continuation-owned condition/body/
  update execution and reusable linear plans. `for-in`, `do-while`, `switch`,
  and `try` retain one focused reusable owner each;
- the guard fixes the control directory to four files and rejects
  `LoopFastPath`, loop fast-path functions, and catch/try source-shape plans in
  compiler/runtime control code; its mutation suite proves both owner and
  recognizer failures;
- benchmark-sized unit tests are replaced by small semantic cases. The complete
  engine all-target/all-feature test suite, strict Clippy, focused reduction
  tests, and every architecture mutation pass locally;
- the audit deliberately exposed false performance: old recognizers collapsed
  exact `function_apply_has_instance`, constructor/prototype, object-literal,
  update/compound, string, switch, try, and while benchmark bodies. With those
  paths removed, three allocation-heavy cases correctly hit the one-million
  object limit and the 99.5-million-iteration while case exceeds the operation
  duration cap instead of skipping observable allocation/iteration semantics;
- one reusable operation earned replacement: a packed default numeric-array
  reduction. The linear plan accepts arbitrary binding names and both unit
  increment spellings, validates numeric/default packed storage, charges steps,
  preserves partial limit state, and declines for holes, indexed prototypes,
  strings, and every other guard miss;
- adjacent canonical-base/branch sentinel medians are arithmetic 85.09/81.25
  ms (-4.5%), array-index 2.49/2.28 ms (-8.4%), property-read 230.11/223.55 ms
  (-2.9%), function-call 158.65/153.66 ms (-3.1%), and string-scan
  72.29/69.91 ms (-3.3%). All rows are valid, checksums match, and branch
  variation is at most 0.5%;
- dense array/native built-ins remain because their guards are based on
  operation semantics: storage kind, descriptors, prototype sensitivity,
  callable registry identity, or general pure callback expression plans. Guard
  misses execute ordinary property/call/callback algorithms; no benchmark or
  harness source name is involved;
- the runner-enabled fast gate passes with strict Clippy, all engine targets,
  documentation, 119/119 runner tests, and every architecture mutation. Full
  local correctness preserves 68/68 engine fixtures, 117/117 active Test262,
  19,414/53,404 files, 37,721/37,721 expected variants,
  37,721/102,578 full variants, and 95/95 QuickJS differential cases.
- PR #449 squash-merged as `7802932e`; required run `29138371964` certified
  exact tree `96eff3d1`, post-merge run `29138475834` passed performance and
  publication, and report-only commit `c0bdf07` published
  `reports/test-runs/rsqjs-test-report-20260711T034409Z.*` with 37,721 expected
  Test262 variants and all five valid sentinels.

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

AS-09a profile evidence in draft PR #450:

- the canonical failure profile selected unary bitwise NOT as a foundational
  syntax gap in the largest `language/expressions` lexer-failure cluster;
- one normal path adds `~` across lexer tokenization, recursive unary parsing,
  `UnaryOp`, numeric bytecode quickening, and the existing Number `ToInt32`
  conversion; it does not add a recognizer or duplicate coercion semantics;
- the focused upstream `language/expressions/bitwise-not` profile passes 28 of
  32 variants. The four remaining variants are the two BigInt cases in default
  and strict modes, which belong to the separate unsupported BigInt model;
- the pre-change `unary_operators` benchmark is valid at 66.86 ms for rs-quickjs
  versus 7.78 ms for QuickJS, an 8.59x tracked ratio with 0.2% local variation;
- focused Number boundary, coercion, Symbol-error, bytecode fallback, active
  Test262, and QuickJS differential coverage is part of the tranche;
- the reviewed full baseline gains 32 variants and 16 files: 28 variants come
  from `language/expressions/bitwise-not`, two from a `delete` expression that
  contains `~`, and two from punctuator coverage. Local correctness passes at
  37,753/37,753 expected variants, 19,430/53,404 files,
  37,753/102,578 full variants, 68/68 engine fixtures, 117/117 active Test262,
  and 95/95 QuickJS differential cases;
- the adjacent main/branch `unary_operators` pair is valid at 69.13/68.64 ms
  for rs-quickjs (-0.7%) and 8.01/7.98 ms for QuickJS. The normalized ratio is
  8.62x/8.59x, branch variation is 0.1%, and no performance regression is
  indicated. Exact-tree CI and canonical publication remain required before
  AS-09a can close.

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
   source-name harness bytecode (complete in PR #399).
3. AS-02a: introduce the checked semantic object reference/facade while keeping
   current physical stores (complete in PR #400).
4. AS-02b1: route get/has through common internal methods (complete in PR
   #401).
5. AS-02b2: route set/define/delete/own-keys/descriptor/prototype through common
   internal methods (complete in PR #403).
6. AS-02c: route JavaScript, native, host, bound, and callable Proxy values
   through common call/construct internal methods (complete in PR #408).
7. AS-03a1: centralize Abstract/Strict Equality, `SameValue`, and
   `SameValueZero` (complete in PR #409).
8. AS-03a2a: centralize `ToPrimitive`, `OrdinaryToPrimitive`, and `ToNumber`
   (complete in PR #410).
9. AS-03a2b: centralize `ToString` and `ToBoolean` (complete in PR #411).
10. AS-03b1a: centralize `ToPropertyKey` (complete in PR #412).
11. AS-03b1b: centralize `ToIntegerOrInfinity`, `ToLength`, and `ToIndex`
    (complete in PR #413).
12. AS-03b2: centralize `GetMethod` plus specification-level property and call
    operations (complete in PR #414).
13. AS-03b3: centralize iterator operations and iterator closing (complete in
    PR #415).
14. AS-04a: separate JavaScript throw completion from engine/host/resource
    failures (complete in PR #416).
15. AS-04b1: migrate Error values into ordinary objects (complete in PR #418).
16. AS-04b2a: add stable source identity and structured frontend diagnostics
    (complete in PR #419).
17. AS-04b2b1: carry canonical token ranges through a span-bearing frontend AST
    (complete in PR #420).
18. AS-04b2b2: lower AST ranges into parallel bytecode metadata and expose the
    executing range on structured runtime diagnostics (complete in PR #421).
19. AS-05a1: remove ambiguous VM cloning and establish VM identity/generation
    (complete in PR #422).
20. AS-05a2a: bind heap strings and Symbols to their VM owner and validate host
    returns (complete in PR #423).
21. AS-05a2b: bind host-local JavaScript errors to their VM owner (complete in
    PR #424).
22. AS-05a2c: define portable owned primitives and explicit local copying
    (complete in PR #425).
23. AS-05b1a: establish executable direct-root categories and enumerate
    existing stored binding, active-call, anchor, and queued-job roots
    (complete in PR #426).
24. AS-05b1b1: define the typed strong-edge visitor and enumerate callable
    stores (complete in PR #427).
25. AS-05b1b2: enumerate object-arena strong edges (complete in PR #428).
26. AS-05b1b3: enumerate Promise/collection/iterator edges and classify weak
    collection keys (complete in PR #429).
27. AS-05b1c: close transient allocation-point roots and define the remaining
    embedder-handle boundary (complete in PR #430).
28. AS-05a2d: define retained object/function handles and explicit release
    (complete in PR #431).
29. AS-05b2a: add complete logical owner counts and teardown reconciliation
    (complete in PR #432).
30. AS-05b2b: add logical retained payload-byte accounting (complete in PR
    #433).
31. AS-05b2c1: define public storage-limit policy and enforce payload/top-level
    owners (complete in PR #434).
32. AS-05b2c2: enforce binding, callable, property, and cache owners
    (complete in PR #435).
33. AS-05b2c3: enforce async, root, frame, and association owners and prove
    complete growth-point reconciliation (complete in PR #436).
34. AS-06a1: replace parallel synchronous call-state vectors with one explicit
    activation owner (merged in PR #438).
35. AS-06a2a: move outer bytecode block/program-counter/operand state onto the
    activation owner (merged in PR #439; function-entry follow-up in draft PR
    #440).
36. AS-06a2b: replace recursive loop/try/finally durability with explicit
    control continuation records.
37. AS-06b: add suspend/resume outcomes and correct pending `await` behavior
    (complete in PR #445 with exact-tree CI and canonical publication).
38. AS-07a: add safe collection over explicit roots and correct weak edges
    (complete in PR #446, merged as `62e2725`).
39. AS-08a: move reusable optimization state behind one optimizer/quickening
    boundary and remove harness-specific opcodes (in progress in draft PR
    #447).
40. AS-08b: audit named recognizers and specialized built-in algorithms against
    broad equivalence and performance evidence.

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
