# Architecture

The engine is a small safe-Rust bytecode VM with a parser AST used as a
front-end intermediate representation. Runtime execution must stay behind
bytecode-owned structures so language growth does not reintroduce AST
interpreter paths.

## Layers

1. `lexer`: converts source text into tokens.
2. `syntax`: owns shared operator, declaration, and script-local static metadata
   used by parser, bytecode, and runtime.
3. `parser`: builds a small AST for the currently supported language subset.
4. `binding_layout`: analyzes parser AST scopes and assigns checked binding
   metadata.
5. `binding_metadata`: owns the AST-free scope, slot, and binding operand data
   consumed by bytecode and runtime layers.
6. `compiler`: consumes parser AST plus binding metadata and emits bytecode-owned
   executable metadata.
7. `bytecode`: owns AST-free VM executable data structures and bytecode metrics.
8. `runtime`: owns globals, host functions, output, and resource counters.
9. `value`: defines the current JavaScript value model.

### In-tree RegExp engine

`src/regress` contains the complete source of the ECMAScript-oriented RegExp
compiler and executor used by the root engine. It is a non-publishable local
crate and the root `Cargo.toml` references it only through a relative `path`, so
RegExp compilation never depends on downloading a `regress` package from a
registry. Keeping the upstream crate boundary preserves its compiler,
optimizer, bytecode, Unicode tables, and executor namespace while making every
source file versioned and editable with the JavaScript engine.

The vendored component retains its upstream dual MIT/Apache-2.0 licenses and a
source checksum manifest. Its large generated Unicode tables and upstream
source layout use a dedicated provenance gate instead of the 800-line gate for
project-authored Rust files. Changes under `src/regress/src` must update that
manifest and carry focused engine behavior tests so an intentional fork stays
distinguishable from accidental vendor drift.

### ECMAScript string representation

Heap strings own their authoritative ECMAScript value as a sequence of UTF-16
code units. This permits lone surrogates, makes indexed properties and
`length` code-unit based, and lets string iteration combine only valid
surrogate pairs. UTF-8 rendering is materialized lazily: it is exact for
well-formed strings and replacement-character based only for diagnostics that
cannot carry arbitrary UTF-16. The heap byte budget reserves both the exact
UTF-16 payload and the exact size of that possible rendering before admission,
so later cache materialization cannot escape the configured limit.

`JsString` is also the portable admission payload. A detached value keeps the
same exact UTF-16/Rc representation without a VM owner; the central runtime
value boundary interns it into `StringHeap` before it can be retained. VM
admission adds owner and slot metadata without changing string semantics.

Source literals, templates, `String.fromCharCode`, `String.fromCodePoint`,
concatenation, slicing, and RegExp patterns/results preserve the code-unit
sequence when they enter the runtime string heap. The in-tree RegExp executor
receives UTF-16 directly, so matching and capture indices use the same units as
ECMAScript `lastIndex`. Rust `str`, `String`, and `OwnedValue::String`
conversions accept only well-formed values; embedders can inspect
`JsString::as_utf16` when exact ill-formed strings must cross the API boundary.

Dynamic compilation preserves the same invariant. Well-formed source keeps the
direct UTF-8 frontend path. An ill-formed ECMAScript string is compiled from its
authoritative UTF-16 units through a diagnostic UTF-8 rendering plus a sparse
lone-surrogate offset map. String, template, and RegExp tokens recover the exact
units before static-value interning; diagnostics retain checked UTF-8 byte
spans, and neither source representation survives `CompiledScript` creation.

The parser AST is not a runtime fallback. It is consumed by binding analysis and
the compiler, then `CompiledScript` stores a `BytecodeProgram` and
bytecode-owned function metadata for execution. The `bytecode` module must not
import parser AST types; AST traversal belongs in `parser`, `binding_layout`,
and `compiler` only. Runtime and bytecode layers may consume
`binding_metadata`, but must not import `binding_layout`; that module remains a
front-end analyzer over parser AST.

`CompiledScript::compile` is the only bridge from source text into the
front-end pipeline. Runtime, bytecode, API, value, storage, object, native, and
function layers must not import lexer, parser, compiler, `binding_layout`, or
parser AST modules directly. If execution needs a new construct, the construct
must first be represented as bytecode-owned metadata and then executed through
the VM.

Contextual `await` grammar is owned by the parser boundary that introduces the
context. Host script compilation may retain the explicit top-level-await
extension for a future asynchronous evaluation API, while direct and indirect
eval parse ECMAScript Script code and therefore reject a top-level await
expression. Module code keeps `await` reserved across every nested ordinary
function. Function parameters and bodies select their own async context, and
class-field initializers neither inherit an enclosing async function's await
expressions nor lose the Module reservation. These contexts are restored at
every nested parse boundary instead of being inferred from emitted bytecode.

Runtime and public API terminology should call guard misses `slow paths` or
`generic semantic paths`. A guarded bytecode, inline-cache, direct-native, slot,
shape, or dense-array specialization may take that slow path when its guard
cannot prove the optimization is valid. Runtime code must not call that an AST
fallback, and it must never fall back to a parser-AST interpreter, retain AST
statement bodies in function objects, or reparse from runtime code.
Standardized ECMAScript and ECMA-402 API property names keep their specified
spelling; for example, the `Intl.DisplayNames` `fallback` option is API data,
not execution-path terminology.

`runtime/optimizer.rs` is the single owner of the VM-local optional-optimization
policy and stable profiling counters. `OptimizationMode::Enabled` is the
default. `OptimizationMode::Disabled` executes the same compiled bytecode but
routes direct binding operands, numeric and string quickening, dense-array
shortcuts, inline/native call caches, linear plans, function fast paths, and
the guarded packed numeric array reduction through their generic semantic
fallbacks. Structured `for`, `while`, `do-while`, `for-in`, `switch`, and
`try` execution is always continuation-owned; there are no workload-shaped
whole-loop executors. The disabled mode is an equivalence and diagnosis tool,
not a separate interpreter or a security boundary.

Optimization state may be read or mutated only through the optimizer boundary.
Guard misses must preserve completion, output, and error behavior. The public
`Vm::optimization_snapshot` exposes the selected mode and stable counters
without exposing cache records or runtime ids.

Reusable control machinery is limited to `for_in`, `structured_do_while`,
`structured_switch`, and `try_catch`. Counted loops execute their original
condition, body, and update bytecode through linear plans. Each immutable,
`Rc`-shared `BytecodeBlock` owns the structural linear and reduction templates
recognized when the block is compiled. Loop entry resolves only the current
activation's binding cells and does not rescan the block for instruction
shapes. One broad linear reduction plan may sum a packed default numeric array
when binding identity, numeric values, array shape, unit update, and step
accounting are all proven; any hole, indexed prototype behavior, non-number,
or guard miss runs the same structured bytecode path.

Test support is not part of bytecode semantics. `print` is an ordinary lazy
native global binding and therefore follows normal lookup, call, shadowing,
metadata, and error rules. `assert.throws` is installed by the JavaScript test
harness. The compiler and bytecode model contain no source-name recognizers or
harness-only instructions for either function.

Removing the parser AST itself is a separate front-end redesign, not fallback
cleanup. It requires a direct parser-to-frontend-IR or parser-to-bytecode
pipeline that still preserves binding analysis, diagnostics, resource
accounting, and Test262 compatibility. Until that redesign is scheduled, the
AST remains a compile-time IR only.

## Execution Model

`Context::activation_frames` is the single owner for synchronous JavaScript
call state. One call activation holds a stable function id independent of its
temporarily parked continuation, its local-scope base, captured upvalues,
dynamic object or eval binding environments, `this`, `new.target`, and an
optional super binding. Eligible sloppy ordinary calls also retain an
accounted original-argument snapshot on that activation. The legacy
`Function#arguments` extension lazily materializes one mapped or unmapped
arguments object from the snapshot, caches it on the matching recursive call
frame, and roots both forms through the same active-execution owner. Strict,
arrow, method, accessor, class, async, and generator functions instead inherit
the restricted `caller` and `arguments` accessors. Temporary class-field
`this` and generated-function
evaluation boundaries use explicit variants on the same stack, so root
enumeration, binding visibility, and resource accounting observe one coherent
owner instead of parallel vectors.

AS-06a2a attaches a `BytecodeContinuationFrame` to the current activation.
Function calls use their stable `FunctionId` as the program key, while general
and top-level frames own an immutable `BytecodeBlock`. Running program-counter,
operand-stack, and last-value state stays with the synchronous driver and its
transient root registry. A suspended outcome moves that state into the
frame's `parked_state`; the `BytecodeFrame` direct-root category traces both
active function ids and parked operands.

AS-06a2b adds one typed structured-control stack to each continuation. Loop,
switch, iterator, and try/catch/finally records own their phase, reusable
segment states, cursors or iterator source, accumulated value, and pending
abrupt completion. The synchronous driver checks a record out once for the
whole construct and mutates it in place; traceable running values use transient
roots, while parked records participate in the `BytecodeFrame` direct-root
category. Each record is charged as an `ExecutionFrame` and must be empty at
activation unwind.

AS-06b uses that same owner for asynchronous execution. `BytecodeOutcome`
distinguishes completed and suspended runs. A pending `await` advances its
instruction state, parks the activation suffix and lexical scopes inside a
typed Promise reaction, and later reattaches those exact owners when the
reaction job runs. Nested bytecode blocks return their completion to the
parked parent continuation, while a control cursor re-enters the existing
loop, iterator, switch, or try record at its recorded phase. No second async
interpreter or reconstructed source execution path exists.

Parent bytecode state distinguishes a direct `await` from suspension in a
nested child. This preserves the parent's program counter, operand stack, and
lexical-scope ownership without routing the settled value to the wrong frame.
Destructuring keeps a typed task stack in that same state. Object-property and
array-element phases, consumed keys, and live iterator records therefore
survive suspension without repeating computed keys, property reads, iterator
steps, or already-created bindings.

Suspend-only state stays behind lazy boxed owners. Ordinary functions use a
const-specialized synchronous call path and the original compact operand-root
iterator; control and async execution consult cold resume roots only when a
state is actually parked. This keeps pending execution explicit without
making every synchronous instruction traverse the async ownership graph.

Awaited non-Promise values and already settled Promises still resume through a
later Promise job. Rejection is injected into the awaiting bytecode state as a
throw completion, so surrounding catch/finally records observe the same path
as synchronous throws. `Context` and `Vm` expose `run_jobs`,
`pending_job_count`, and the shutdown-oriented `cancel_jobs`; cancellation
discards ready and pending reactions, releases parked frames and bindings,
and leaves affected Promise objects pending. Legacy `eval` continues draining
ready jobs after a normal script for compatibility. Top-level await remains
gated until an asynchronous evaluation result API exists and is rejected
without retaining execution frames.

## Embedding Model

The library API is the product surface. The CLI exists for smoke tests, differential checks, and benchmark orchestration, but engine architecture must optimize for embedding inside a larger Rust application. A feature is not complete just because it works through the CLI; VM-facing behavior must also make sense through the Rust API.

The public model should evolve around these roles:

- `Engine`: shared immutable configuration, feature flags, parser caches, atom tables, and other data that can be reused safely across isolated virtual machines.
- `Vm`: one isolated JavaScript virtual machine with its own heap, globals, job queue, resource counters, and teardown report.
- `Context`: an execution view into a `Vm`, used to evaluate scripts, inspect values, and register host bindings.
- `OwnedValue`: a VM-independent primitive copy for serialization and transfer.
- `RetainedValue`: a non-cloneable, VM-bound durable root for values that must
  survive across embedding calls.
- `VmStorageSnapshot`: an explicit on-demand count and logical payload-byte
  map for every current variable-size VM storage owner.
- `CompiledScript`: a reusable bytecode-owned representation hidden behind the
  embedding API.
- `HostFunctionRegistry`: synchronous and asynchronous Rust callbacks exposed to JavaScript as functions.

The current public skeleton exposes `Engine`, `EngineConfig`, `Vm`, `VmConfig`,
`Context`, `CompiledScript`, `CompiledScriptUsage`, `VmResourceUsage`, and
`VmTeardownReport`. `Runtime` remains as a compatibility surface for existing
smoke tests and runner code, while new embedding-facing work should prefer the
`Engine -> Vm -> Context` path.

`CompiledScript` records compile-time usage for source length, top-level
statement count, maximum expression depth, and bytecode instruction counts. A
target `Context` checks those metrics before execution, so a script compiled
with wider limits cannot bypass a stricter VM's compile-time resource limits.
The representation is intentionally hidden behind the public API so bytecode
operands and quickening can evolve without exposing internal VM details.

Multiple `Vm` instances must be able to run in the same Rust process without sharing mutable JavaScript state. A failure, resource-limit hit, pending job, or global mutation in one VM must not affect another VM. Shared data is allowed only when it is immutable or protected by explicit synchronization and resource accounting.

Each `Vm` also owns the origin and last observed value for its monotonic
`performance.now()` clock. The default source is `std::time::Instant`, while
the embedding API accepts a duration reader for deterministic execution. A
shared reader does not create shared JavaScript clock state: each VM captures
its own origin and clamps regressions against its own last observation.

Embedding API invariants:

- Creating, running, interrupting, and dropping one `Vm` must not require stopping or mutating another `Vm`.
- Public handles must not permit values, objects, promises, or callbacks to cross VM boundaries accidentally.
- Raw `Value` results are call-local compatibility values. Durable values must
  use `OwnedValue` or an identity- and generation-checked `RetainedValue`.
- APIs should make resource ownership explicit. Engine-wide caches, VM heaps, queued jobs, host callbacks, and output buffers must have clear owners.
- Storage snapshots must remain explicit diagnostic work; ordinary evaluation
  and host callback dispatch must not scan every VM owner implicitly.
- The engine must not assume a process-wide async runtime. Async integration belongs at the embedding boundary.
- The API should keep bytecode internals hidden unless exposing a VM control is
  clearly useful for embedders.

Host extensions are a first-class design concern:

- Rust code must be able to register typed host functions under explicit names.
- Host functions must receive arguments through checked conversions and return `Result` values with contextual errors.
- Asynchronous host functions must integrate with a VM-owned job queue and return JavaScript promises once promises exist.
- The embedding application, not the engine, should own the outer async executor. The engine should expose explicit polling or job-draining APIs instead of depending on a specific runtime.
- Host callbacks must have per-callback quotas for runtime steps, allocations, output, and wall-clock cancellation hooks.
- Host callbacks must never bypass VM isolation or leak values across VM boundaries without an explicit serialization or transfer step.

The current synchronous host-function API is registered through
`Context::register_host_function`. Callbacks receive a `HostCall` view with
checked argument accessors such as `number`, `string`, and `boolean`, and they
return `Result<Value>`. Callback storage is VM-local. A callback-local value
can be copied with `LocalValue::to_owned_value` or rooted beyond the callback
with `LocalValue::retain`. VM-owned object/function callback returns remain
conservative until host returns accept `RetainedValue` directly.

## Safety Policy

The crate uses `#![deny(unsafe_code)]`, and the lint is also declared in `Cargo.toml`.

If future performance work appears to require unsafe code, it must go through a separate design document with:

- the exact measured bottleneck
- the safe alternative that was rejected
- memory and aliasing invariants
- fuzzing and sanitizer coverage
- a review path that keeps unsafe code isolated from the public embedding API

The default answer should remain safe Rust.

## Resource Model

`RuntimeLimits` is part of the public API and is checked by the parser,
compiler, and bytecode runtime. The goal is to make resource use explicit at
the embedding boundary instead of relying on global process limits.

`RuntimeLimits::storage` adds an unlimited-by-default `VmStorageLimits` policy
keyed by `VmStorageKind`. AS-05b2c1 enforces atoms, heap strings, Symbols,
objects, byte buffers, host callbacks, output, and retained source records;
AS-05b2c2 enforces bindings, JavaScript/native/bound functions, object
properties, and cache entries through an independently reconciled VM-local
ledger. AS-05b2c3 extends that ledger to collections, Promise reactions/jobs,
retained and transient roots, execution frames, and associations. Module
storage is explicitly zero until a module owner exists. Every snapshot checks
all twenty-six categories against policy and reconciles all ledger-backed
owners. Custom policies are immutable and shared across cloned configuration,
while VMs keep independent usage and teardown state.

Current limits cover:

- source length
- statement count
- expression nesting depth
- runtime evaluation steps
- string length
- number of global bindings
- number of interned atoms as a reported usage metric

Future limits should cover heap budgets, atom table budgets, stack budgets, module loading, and host callback quotas.

Every new VM-facing feature should define how it participates in limits before it is considered complete. This includes parser work, runtime steps, heap growth, host callback calls, queued jobs, module loads, and output buffering.

## Collection Model

Each VM owns a safe, non-moving collection domain. Objects, JavaScript and
native callables, host and bound functions, Promises, collections, and
collection iterators use sparse indexed arenas. Heap strings and Symbols use
equivalent sparse tables. Live records keep stable ids; a collection cycle may
vacate unreachable slots and reuse them only after identity-bearing caches are
invalidated.

`Vm::collect_garbage` exposes an explicit stop-the-world safepoint. The
bytecode dispatcher also invokes the same collector at allocation-pressure
safepoints after registering the active operand state as transient roots. The
first pressure threshold reserves the final one-sixteenth of the configured
object-record limit as allocation headroom. If a cycle cannot reclaim enough
records, the next threshold advances toward the hard limit instead of running
a complete mark/sweep on every instruction. The marker starts from the shared
direct-root registry and follows the typed callable, object, Promise,
collection, iterator, and suspended-frame edge visitors. Map, Set, and iterator
entries are strong. WeakSet keys are weak, and WeakMap entries are resolved as
ephemerons to a fixed point: a value is marked only when its key is already
reachable. Sweep removes dead weak entries before reclaiming arenas, then
reconciles every ledger-backed owner with the independent storage snapshot.

Queued jobs, suspended async activations, runtime anchors, registered Symbols,
scoped transient values, and retained embedder handles are roots. Raw
VM-local `Value` ids are compatibility values, not durable handles across a
collection call; embedders must retain values that need to survive the
safepoint. Collection is VM-local and does not mutate another VM created by the
same `Engine`.

Hard storage limits remain checked at each allocation. Explicit or
pressure-triggered collection releases logical owner counts and permits arena
slots, heap-string bytes, and Symbol slots to be reused under those limits. A
reachable graph still fails at the same configured limit. Atoms, shapes, and
their canonical metadata remain deliberate cache roots until AS-08 centralizes
optimizer ownership and invalidation. WeakRef and FinalizationRegistry remain
unsupported until callback ordering and job semantics are specified.

## Compatibility Strategy

QuickJS is the reference engine, not code to transliterate line by line. The compatibility path should be:

1. add focused engine-specific tests for a feature
2. compare behavior against QuickJS
3. add or identify the relevant Test262 coverage
4. implement the safe Rust model
5. add performance and memory baselines
6. only then widen the supported Test262 subset or mark the feature as covered

Feature work should not rely on Test262 alone. Project-specific tests must cover embedding behavior, resource limits, host extension behavior, and regressions that are important for surveillance-device workloads even when those cases are not represented in upstream ECMAScript tests.

For embedding-sensitive features, the compatibility path also needs a direct library API check. CLI-only coverage is acceptable for smoke tests, but it must not be the final proof for VM isolation, host callback behavior, async job scheduling, resource accounting, or teardown reporting.
