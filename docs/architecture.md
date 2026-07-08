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

Runtime and public API terminology should call guard misses `slow paths` or
`generic semantic paths`. A guarded bytecode, inline-cache, direct-native, slot,
shape, or dense-array specialization may take that slow path when its guard
cannot prove the optimization is valid. Runtime code must not call that an AST
fallback, and it must never fall back to a parser-AST interpreter, retain AST
statement bodies in function objects, or reparse from runtime code.

Removing the parser AST itself is a separate front-end redesign, not fallback
cleanup. It requires a direct parser-to-frontend-IR or parser-to-bytecode
pipeline that still preserves binding analysis, diagnostics, resource
accounting, and Test262 compatibility. Until that redesign is scheduled, the
AST remains a compile-time IR only.

## Embedding Model

The library API is the product surface. The CLI exists for smoke tests, differential checks, and benchmark orchestration, but engine architecture must optimize for embedding inside a larger Rust application. A feature is not complete just because it works through the CLI; VM-facing behavior must also make sense through the Rust API.

The public model should evolve around these roles:

- `Engine`: shared immutable configuration, feature flags, parser caches, atom tables, and other data that can be reused safely across isolated virtual machines.
- `Vm`: one isolated JavaScript virtual machine with its own heap, globals, job queue, resource counters, and teardown report.
- `Context`: an execution view into a `Vm`, used to evaluate scripts, inspect values, and register host bindings.
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

Embedding API invariants:

- Creating, running, interrupting, and dropping one `Vm` must not require stopping or mutating another `Vm`.
- Public handles must not permit values, objects, promises, or callbacks to cross VM boundaries accidentally.
- APIs should make resource ownership explicit. Engine-wide caches, VM heaps, queued jobs, host callbacks, and output buffers must have clear owners.
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

The current synchronous host-function skeleton is registered through
`Context::register_host_function`. Callbacks receive a `HostCall` view with
checked argument accessors such as `number`, `string`, and `boolean`, and they
return `Result<Value>`. Callback storage is VM-local. The skeleton rejects
VM-owned handle return values (`Object`, `Function`, `NativeFunction`, and
`HostFunction`) until the embedding API has VM-bound handles or explicit
serialization.

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
