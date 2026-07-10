# Semantic Architecture Inventory

This document is the AS-01a inventory required by
[Architecture Stabilization And Development Strategy](architecture-stabilization-plan.md).
It records where JavaScript semantics are owned today, where equivalent behavior
is split, and which later stabilization task must absorb each path.

This is a map of the current implementation, not an endorsement of every
boundary. It intentionally avoids micro-optimization advice. Its purpose is to
make compatibility work safe to parallelize while AS-02 through AS-08 migrate
the engine toward one semantic kernel.

## Snapshot And Scope

- Inventory date: 2026-07-10
- Source snapshot: `origin/main` at `bbdf546`
- Plan item: AS-01a
- Follow-up: AS-01b converts the mechanical parts of this inventory into
  repository guards
- Runtime scope: `src/value`, `src/runtime`, `src/bytecode`, `src/compiler`,
  `src/storage`, and the public embedding surface under `src/api`

The inventory covers:

- object-like values and their physical stores;
- property, prototype, descriptor, call, and construct entrypoints;
- equality, conversion, iteration, and completion paths;
- VM-owned stores, implicit roots, public handles, and accounting gaps;
- generic execution, caches, direct paths, and workload-shaped specialization;
- the migration owner and guard candidate for each split boundary.

## Reading The Maps

The tables use three ownership labels:

- **widest facade**: the current entrypoint with the broadest value coverage;
- **physical owner**: the store or representation that owns the data;
- **parallel path**: code that reproduces dispatch or semantics outside the
  widest facade.

The widest facade is the safest existing entrypoint for new compatibility work,
but it is not automatically specification-complete. AS-02 and AS-03 will replace
these provisional facades with explicit internal methods and abstract
operations.

## Value And Object Identity Map

`Value` currently has thirteen variants. Five variants are object-like in
JavaScript terms and carry four unrelated numeric ids plus one inline error
record.

| Value variant | Identity and physical owner | Property owner today | Call/construct owner today | Target migration |
| --- | --- | --- | --- | --- |
| `Object(ObjectId)` | `ObjectId(usize)` into `Context.objects: ObjectHeap` | `ObjectHeap`, with Proxy pre-dispatch in `Context` and built-ins | callable/constructable Proxy objects enter `semantic_call`/`semantic_construct` using immutable capability flags | AS-02a facade and AS-02b/AS-02c internal methods |
| `Function(FunctionId)` | `FunctionId(usize)` into `Context.functions` | `FunctionProperties` in `runtime/function/properties.rs` | `semantic_call`/`semantic_construct`, with bytecode functions as the payload backend | AS-02a/AS-02c; retain the function arena as a payload store if useful |
| `NativeFunction(NativeFunctionId)` | `NativeFunctionId(usize)` into `Context.native_functions`, plus `NativeFunctionRegistry` | a second `FunctionProperties` implementation path | common semantic dispatch with guarded direct-native call/construct backends | AS-02a/AS-02c; native code remains a payload behind common methods |
| `HostFunction(HostFunctionId)` | `HostFunctionId(usize)` into `Context.host_functions` | no ordinary property, descriptor, prototype, or own-key path | `semantic_call`; never constructable | AS-02a/AS-02c and AS-05a host-handle boundary |
| `Error(ErrorObject)` | inline `{ ErrorName, String }`, with no object id | synthetic `name`/`message` handling and separately installed error prototypes | not callable or constructable | AS-02a then AS-04b as an ordinary error object |

Primitive variants are `Undefined`, `Null`, `Bool`, `Number`, `String`,
`HeapString`, and `Symbol`. `HeapString` and `Symbol` contain VM-owned ids or
VM-owned shared data even though public `Value` does not encode a VM identity.

All current ids are slot indexes without a VM id or generation. A stale id or a
value moved between VMs cannot be rejected from the id alone. This is the main
input to AS-05a.

### Checked Semantic Object Facade

AS-02a introduces `Context::semantic_object_ref` in
`runtime/semantic_object.rs` as the only checked construction path for the
incremental `SemanticObjectRef`. It preserves the current physical stores while
giving semantic code one entrypoint with this contract:

- `ObjectId` is checked by `ObjectHeap`;
- `FunctionId` and `NativeFunctionId` are checked by their existing owning
  `Context` accessors;
- `HostFunctionId` is checked by the host-function owner;
- inline `ErrorObject` values are admitted as the current object-like error
  representation;
- primitive values return `None` rather than producing a semantic object.

The checked reference retains the source `Value` and currently exposes its
`ObjectId` only when the payload lives in `ObjectHeap`. AS-02b and AS-02c should
add operations to this boundary instead of exposing physical payload records.
The first bounded migrations are Proxy target/handler and construct-result
validation, JavaScript constructor return selection, and typed-array debug
inspection. The architecture guard now rejects another named semantic-object
facade or restoration of the removed local object-like classifier.

AS-02b1 adds `semantic_property_read`,
`semantic_property_read_with_receiver`, and `semantic_property_presence` plus
their ordinary-object finish paths. These methods resolve Proxy, JavaScript
function, native function, HostFunction, Error, boxed-string, and global-object
pre-dispatch before returning an explicit cacheable `ObjectHeap` tail. Static
inline caches may optimize only that tail; their miss and uncacheable paths
remain equivalent to the generic semantic finish path. Proxy get/has receives
the original Symbol value when the lookup key is a Symbol, and `Reflect.get`
propagates its explicit receiver through Proxy and accessor paths.

AS-02b2 extends the facade through focused modules under
`runtime/semantic_object/`. Mutation pre-dispatch returns ordinary-object
tails for cacheable set/delete operations, while descriptor, own-key,
prototype, extensibility, and integrity methods dispatch every object-like
variant before reaching physical storage. `Reflect.set` uses a receiver-aware
semantic recursion across descriptors and prototypes. Proxy set/delete/define,
descriptor, and own-key traps retain Symbol values, and Proxy integrity
operations compose the same observable methods instead of freezing or sealing
the wrapper record directly. The shared `ownKeys` path also validates unique
trap keys, every non-configurable target key, and exact key sets for
non-extensible targets before Object or Reflect projects string/Symbol keys.

AS-02c adds `semantic_is_callable`, `semantic_call`,
`semantic_is_constructor`, and `semantic_construct` in
`runtime/semantic_object/invocation.rs`. Generic bytecode calls and
construction, Function helpers, accessors, callbacks, Promise reactions,
JSON, Reflect, and Proxy enter through this boundary. Direct-native and
call-site-cache hits remain guarded backends; every miss returns to the common
semantic path. Callable and constructable Proxy capabilities are captured from
the target at Proxy creation, so revocation removes target/handler access but
does not change the Proxy's internal-method shape. Bound functions inherit the
target's constructor capability and preserve explicit `newTarget` replacement
rules. The complete local corpus validates this consolidation with 87 new
Test262 pass variants, no lost passes, and 95/95 QuickJS differential cases.

This facade rejects ids whose slots are not defined in the receiving `Context`.
It does not yet prove VM identity or generation: a foreign id can still alias a
live local slot with the same numeric index. AS-05a remains responsible for
VM-bound identity, generations, and public handle validation.

### Object-Backed Exotic Data

`runtime/object/mod.rs` stores ordinary properties and many unrelated payloads
in one `Object` record.

| Object behavior | Physical representation | Semantic dispatch |
| --- | --- | --- |
| Ordinary object | named properties, shape, prototype, extensibility | `ObjectHeap` property/prototype modules |
| Array | `array_storage`, `array_length`, and length attributes on `Object` | ordinary property code plus array-specific branches and bytecode/native fast paths |
| Boxed String | `string_value` on `Object` | string virtual properties plus ordinary properties |
| Boxed Boolean/Number/Symbol | `primitive_value` on `Object` | built-in receiver checks plus ordinary properties |
| Date | `date_value` on `Object` | Date built-ins plus ordinary properties |
| RegExp | `regexp_value` on `Object` | RegExp built-ins plus ordinary properties |
| Proxy | `proxy_value` on `Object` | repeated pre-dispatch in get/has/set/delete/prototype/descriptor/call/construct paths |
| ArrayBuffer | `byte_buffer` on `Object`; bytes are `Rc<RefCell<Vec<u8>>>` | typed-array built-ins plus ordinary properties |
| Uint8Array | `uint8_array` on `Object`, referencing a buffer object and shared buffer | typed-array indexed branches plus ordinary properties |
| Raw JSON marker | `is_raw_json` boolean on `Object` | JSON built-ins |

The following object kinds use `Value::Object` identity but keep their internal
slots in `Context` side tables instead of `Object`:

| Object behavior | Side store | Object-to-slot association | Important property |
| --- | --- | --- | --- |
| Promise | `promises: Vec<Promise>` | `promise_object_slots: Vec<Option<PromiseId>>` indexed by `ObjectId` | reactions and settled values hold strong `Value` edges; jobs are separate |
| Map/Set/WeakMap/WeakSet | `collections: Vec<CollectionData>` | `collection_object_slots: Vec<Option<(CollectionKind, CollectionId)>>` | all four kinds share strong `Vec<(Value, Value)>` storage |
| Collection and RegExp iterators | `collection_iterators: Vec<CollectionIteratorState>` | iterator id captured by a native `next` function | iterator contents are snapshots of strong `Value` edges |

Bound functions use another split representation: the visible value is an
ephemeral `NativeFunction`, while bound target, `this`, and arguments live in
`Context.bound_functions`. AS-02c keeps that physical split but routes both
calls and construction through the semantic boundary; constructor capability
is derived recursively from the bound target rather than encoded in the native
kind allowlist.

### Immediate Identity Invariants

Until AS-05 lands, compatibility work must preserve these
rules:

1. Do not add another object-like `Value` variant.
2. Do not add an object side table without recording its internal-slot owner,
   object association, strong/weak edges, limit, teardown behavior, and AS-02
   migration path.
3. Do not expose another raw id-bearing `Value` through the embedding API.
4. Treat `Vm::clone` and `Context::clone` as known debt, not a model for a new
   public owner.
5. Weak behavior may not be claimed while keys remain strong `Value` entries.

## Property And Internal-Method Map

There is no single current implementation of the full ECMAScript object
internal-method set. `ObjectHeap` is the physical owner for ordinary objects;
`Context` methods add dispatch for functions, errors, primitives, globals, and
proxies; several built-ins repeat the same matches.

| Operation | Widest current facade | Physical and parallel paths | Required owner |
| --- | --- | --- | --- |
| `ToPropertyKey`-like conversion | `property::property_key` plus `Context::dynamic_property_key` | conversion uses `Value` display text for non-string/non-symbol values; `Object` and `Reflect` wrappers add their own argument handling | AS-03b `ToPropertyKey` |
| `[[Get]]` | `Context::semantic_property_read[_with_receiver]` plus `finish_semantic_property_read` | AS-02b1 now owns object-like dispatch; `get_property_value` still owns primitive string/prototype behavior, while static caches optimize only the returned ordinary-object tail | AS-02b1 complete after PR #401; AS-03b later owns spec-level `Get`/primitive coercion |
| `[[HasProperty]]` | `Context::semantic_property_presence` plus `finish_semantic_property_presence` | AS-02b1 now owns object-like dispatch; static presence caches optimize only the returned ordinary-object tail, while primitive rejection remains in the property layer | AS-02b1 complete after PR #401; AS-03b later owns the abstract operation |
| `[[Set]]` | `semantic_property_write` plus `finish_semantic_property_write`; `semantic_reflect_property_write` for explicit receivers | Static/dynamic caches optimize only ordinary tails; physical accessor/object/function stores remain backends | AS-02b2 complete in PR #403; AS-03b later owns full strict/sloppy assignment semantics |
| `[[DefineOwnProperty]]` | `semantic_define_own_property_*` | ObjectHeap and function stores remain physical backends; function paths still reject accessors | AS-02b2 complete in PR #403 |
| `[[Delete]]` | `semantic_property_delete` plus `finish_semantic_property_delete` | Static/dynamic caches optimize only ordinary tails; primitive fallback remains in `property::delete_property` | AS-02b2 complete in PR #403; AS-03b owns primitive coercion |
| `[[OwnPropertyKeys]]` | `semantic_own_property_keys` plus string/Symbol projections | ObjectHeap and function key stores remain physical backends; Proxy order and Symbol identity are preserved | AS-02b2 complete in PR #403 |
| `[[GetOwnProperty]]` | `semantic_own_property_descriptor` | Object/string/global/function physical descriptor stores remain backends; HostFunction is rejected | AS-02b2 complete in PR #403 |
| `[[GetPrototypeOf]]` | `semantic_get_prototype` | ObjectHeap/function/error prototype owners remain physical backends; HostFunction is rejected | AS-02b2 complete in PR #403 |
| `[[SetPrototypeOf]]` | `semantic_try_set_prototype` | ObjectHeap stores only `ObjectId` prototypes, so function-valued prototypes remain unsupported storage debt | AS-02b2 complete in PR #403; AS-05 owns handle/storage redesign |
| extensibility/integrity | `semantic_{is,prevent}_extensions` and semantic integrity-level methods | Ordinary objects use ObjectHeap directly; Proxy integrity composes traps and descriptors; function stores still lack extensibility state | AS-02b2 complete in PR #403; AS-05 owns complete accounting/state |

### Known Property Divergences

- `HostFunction` is callable but lacks ordinary function object properties,
  descriptors, a prototype path, and own keys.
- Error properties are synthesized as writable/enumerable/configurable data
  descriptors in some paths rather than stored as real own properties.
- Function and native-function properties share a data structure but still
  require separate physical id lookup and dispatch functions; their current
  own-key backend also exposes only enumerable custom names rather than a full
  string/Symbol key list.
- Array indexed properties, string virtual properties, and global bindings
  still span physical and semantic layers, but object-like read/presence callers
  now receive an explicit generic `ObjectHeap` tail instead of repeating that
  dispatch.
- `property::get_property` and `property::has_property` cover fewer value kinds
  than the similarly named `Context` facades. Their generic names make the
  distinction easy to miss.
- Static-name caches now receive only explicit ordinary-object tails from the
  semantic facade; the remaining cache code owns shape/version mechanics, not
  value-kind dispatch.

For new compatibility code after AS-02b, use the semantic `Context` facade.
Direct `ObjectHeap` access is acceptable only for storage creation
or a proven ordinary-object-only operation; it must not silently become the
only semantic implementation of a language feature.

## Call And Construct Map

### Call

| Layer | Entrypoint | Coverage and bypasses |
| --- | --- | --- |
| Semantic internal method | `Context::semantic_call` | checked Function, NativeFunction, HostFunction, bound-function, and callable Proxy dispatch with one completion-preserving contract |
| Capability predicate | `Context::semantic_is_callable` | the single `IsCallable`-style decision used by Function helpers, callbacks, Promise, JSON, Reflect, Proxy traps, coercion hooks, `typeof`, and object tagging |
| Compatibility wrappers | `Context::eval_call_completion` and `Context::eval_call_value` | borrow the callee and delegate to `semantic_call`; the value wrapper converts the resulting completion at the existing native boundary |
| Cached calls | `Context::eval_cached_call_completion` and `CallValueCache` | caches Function, NativeFunction, and HostFunction; every guard miss returns to `semantic_call` through the compatibility wrapper |
| Identifier direct path | `CallReference` and `eval_bytecode_identifier_call_*` | can bypass value dispatch for bytecode functions, cached native kinds, and `NativeCallTarget`; generic case returns to `eval_call_completion` |
| Static member direct path | `runtime/native/function/direct.rs` plus property caches | recognizes native targets and falls back to native-kind or value call paths |
| Accessors/callbacks | accessor, array, collection, JSON, Promise, Proxy, Reflect, and iterator code | callability and dispatch are shared, but several callers still translate arbitrary thrown values into engine errors; AS-04 owns that completion debt |

The direct paths are guarded optimization backends, not alternative semantic
predicates. Architecture checks allow the callable and constructor predicates
only in `runtime/semantic_object/invocation.rs` and reject growth in the former
split entrypoint set.

### Construct

| Layer | Entrypoint | Coverage and bypasses |
| --- | --- | --- |
| Semantic internal method | `Context::semantic_construct` | checked Function, constructable NativeFunction, constructable bound function, and constructable Proxy dispatch with an explicit `newTarget` |
| Capability predicate | `Context::semantic_is_constructor` | derives JavaScript-function capability, native-kind capability, recursive bound-target capability, and the Proxy capability captured at creation |
| Generic bytecode | `Context::eval_new_value` | delegates to `semantic_construct` with the constructor itself as `newTarget` |
| Identifier bytecode | `eval_bytecode_new_value` and `eval_bytecode_function_constructor` | retains a guarded direct-native backend and the unbound `Test262Error` special case; the generic path returns to `semantic_construct` |
| Function construction | `eval_function_constructor_value` | creates the receiver from explicit `newTarget.prototype`, executes with that `new.target`, and uses the checked semantic-object boundary for return override |
| Proxy and bound construction | `proxy_construct` and `eval_bound_function_construct` | preserve explicit `newTarget`; Proxy traps receive it, fallbacks retain it, and a bound function replaces itself with its target only for the spec-defined case |
| Reflect | `eval_reflect_construct` | validates both target and optional explicit `newTarget` with the shared predicate and preserves the latter through dispatch |

Native constructor payloads still create their built-in-specific receiver and
do not yet consume an alternate `newTarget.prototype`. Derived-class `super()`
also remains an in-place JavaScript-function specialization because the current
execution model creates `this` before entering the parent. These are recorded
exceptions, not templates for new paths: native receiver generalization belongs
to AS-03b, while complete derived-constructor activation belongs to AS-06.

## Equality And Conversion Map

### Equality

| Semantics | Current owner(s) | Split |
| --- | --- | --- |
| Rust `PartialEq<Value>` | `value/kind.rs` | identity/value equality used as a building block, but not named as one ECMAScript operation |
| abstract and strict equality | `runtime/bytecode/coercion.rs` | bytecode-owned rather than a runtime abstract-operation owner; numeric bytecode has additional specialized equality instructions |
| `SameValueZero` | `runtime/collections.rs`, `runtime/object/array/search.rs`, and `runtime/native/builtins/array/generic.rs` | three implementations plus an array numeric helper |
| `SameValue` | `runtime/native/builtins/object_static.rs` | local to `Object.is` |

AS-03a should create one equality owner and make optimized numeric instructions
call or prove equivalence to it. Migration should delete local implementations
in the same pull request that redirects their callers.

### Conversion And Numeric Indexing

| Semantics | Current owner(s) | Split or limitation |
| --- | --- | --- |
| number conversion | `Context::value_to_number` in `native/builtins/number.rs`; bytecode coercion and many built-ins call it | location is a built-in module; object-to-primitive ordering is handled separately by selected built-ins |
| primitive conversion | Date has `ordinary_to_primitive`; Math and JSON contain their own method probing/coercion paths | no shared `ToPrimitive` with hints and abrupt completion |
| property-key conversion | `property::property_key`, `dynamic_property_key`, Object wrapper, Reflect wrapper | no single spec-named operation; nonprimitive conversion relies on display text |
| integer-or-infinity | String and Array built-ins contain separate helpers | semantics and supported numeric ranges differ by caller |
| length/index conversion | Array generic code, Function.apply, Reflect list creation, and String helpers each convert/cap lengths | several maxima use current storage limits rather than one `ToLength`/`ToIndex` contract |
| boolean conversion | `Value::is_truthy` | broadly reused and a good candidate to move behind AS-03a without changing representation |
| string conversion | `Display for Value`, string constructors, concatenation helpers, JSON, Date, and property-key code | formatting and semantic conversion are not cleanly separated |

The migration order is equality and primitive conversion first (AS-03a), then
property-key, numeric-index, property, call, and iterator operations (AS-03b).

## Iterator Map

`runtime/bytecode/for_of.rs` is the widest current iterator implementation. It
owns `ForOfSource`, protocol acquisition, step, and a best-effort close path.
Destructuring, spread, Math iterable methods, Object.fromEntries, Map/Set, and
WeakMap/WeakSet reuse portions of this bytecode-owned API.

The current paths are:

- primitive strings use a direct character snapshot;
- arrays use direct live index iteration when no protocol method is found;
- other supported values use `Symbol.iterator`, cache `next`, and read
  `{ done, value }` through property access;
- collection/RegExp iterator objects use `Context.collection_iterators`, which
  stores a snapshot and advances through a native `next` function;
- set algebra implements another manual iterator loop in
  `native/builtins/set_operations.rs`;
- IteratorClose ignores failures in the `return` lookup/call in the current
  `close_for_of_source` interface because it returns no completion.

AS-03b should move `GetIterator`, `IteratorStep`, `IteratorValue`, and
`IteratorClose` out of the bytecode module. Direct array/string iteration may
remain an optimization only with guards and a generic protocol fallback.

## Completion, Exception, And Diagnostic Map

The engine already has a useful JavaScript `Completion` enum with `Normal`,
`Throw`, `Return`, `Break`, and `Continue`. The split occurs at its boundaries:

| Boundary | Current behavior | Required migration |
| --- | --- | --- |
| Public `eval` | `Completion::into_result` formats `Throw(Value)` as `Error::Runtime("uncaught throw: ...")` | AS-04 typed uncaught-JS exception result |
| Native built-ins | mostly return `Result<Value>`; specification errors use `Error::Exception`, `Error::Runtime`, or `Error::ResourceLimit` | AS-04 separates JavaScript abrupt completion from engine failure |
| Runtime-to-throw conversion | `runtime_exception_value` converts `Error::Exception`; it also parses a `Runtime` message beginning with `ReferenceError:` | AS-04 removes message-prefix classification |
| Reference errors | `reference_error_undefined` and `reference_error_uninitialized` manufacture `Error::Runtime` text | AS-04 creates real JS errors and `Throw` directly |
| Accessors and native callbacks | local matches translate `Completion::Throw(Value::Error)` back into `Error::Exception`; non-Error throws often become formatted runtime errors | AS-04 preserves arbitrary thrown values across native frames |
| Error instances | `Value::Error(ErrorObject)` with synthetic properties/prototype | AS-02/AS-04 migrate to ordinary object identity and internal error slots |
| Source diagnostics | lexer/parser errors carry an offset; runtime bytecode and `Value::Error` carry no `SourceId`/span | AS-04b adds stable source metadata without retaining the AST at runtime |

Resource limits should continue to bypass JavaScript catch unless the embedding
contract explicitly changes. Host failures and invariant failures also need
typed engine channels rather than becoming catchable based on message text.

## VM Store, Root, And Accounting Map

`Context` is the aggregate owner. There is no trace/root trait or root
enumeration function in `src`; roots are implicit wherever a store contains a
`Value`, id, shared binding cell, or callback capture.

| Store category | Current fields/owners | Implicit strong edges | Current public accounting |
| --- | --- | --- | --- |
| interned names and text | `atoms`, `strings`, `symbols`, well-known caches | strings/symbol registry retain shared text | atom count, string count, string bytes |
| bindings and closures | globals, builtin globals, locals, upvalue frames, `BindingCell(Rc<Mutex<Binding>>)` | binding values and captured cells | global binding count and upvalue cell count only |
| executable functions | functions, native functions/registry, bound functions, host functions | bytecode metadata, properties, upvalues, bound args/targets, callback `Rc` captures | native function count only |
| objects | `ObjectHeap`, shapes, prototypes, properties, arrays, buffers, typed-array views | property values, prototypes, accessors, shared buffers | shape count and prototype version; no object/property/buffer bytes |
| collections | collection stores, object slots, iterator snapshots | keys, values, iterator items | none; count limit reuses `max_objects` |
| promises/jobs | promises, object slots, reaction queue | results, handlers, settled values, queued job state | none; no job-count limit |
| active execution | local frame bases, `this`, `new.target`, super frames, bytecode operand stacks held by Rust calls | live values and activation metadata | call depth is enforced internally, but no public frame/stack bytes |
| caches | static name/binding caches, call caches, function fast paths | ids, shapes, native kinds, metadata | hit/miss counters for selected call caches |
| embedder-visible state | output, host callbacks, `Vm`, public `Value` | output strings and callback captures | output entry count, not bytes |
| nondeterministic/runtime state | clock, random state, step counters | no JS edges | runtime steps and selected execution counters |

`RuntimeLimits` currently covers source length, statement count, expression
depth, runtime steps, string length, bindings, object count, and per-object
property count. It has no complete heap-byte, atom, function, collection-entry,
buffer-byte, output-byte, promise, job, stack, frame, module, or host-callback
budget.

`VmResourceUsage` exposes a useful subset, but `VmTeardownReport` simply wraps
that subset. It cannot yet prove that every VM-owned category was released or
state how many bytes each owner retained.

### Public Handle Boundary

- `Vm` and `Context` derive `Clone`.
- cloning copies indexed stores but shares selected mutable state through
  `Rc<Mutex<...>>` binding cells and `Rc<RefCell<Vec<u8>>>` buffers; host
  callbacks and metadata are also shared through `Rc`;
- `Vm::get_global` returns public `Value`, including raw VM-local ids;
- `HostCall` exposes borrowed public `Value` arguments;
- host return validation rejects Function, NativeFunction, HostFunction, and
  Object, but permits `HeapString`, `Symbol`, and `Error`, which can still carry
  VM-derived identity/data;
- ids have neither VM identity nor generation checks.

AS-05a must define owned cross-VM primitives versus VM-bound local handles
before public object/function/promise handles are expanded.

### Provisional Root Set For AS-05b

The root contract must at least enumerate:

1. global, builtin-global, local, and captured binding cells;
2. active operand stacks and future explicit activation frames;
3. `this`, `new.target`, super, class-field, and function-property values;
4. native/host/bound function state and temporary native-call arguments;
5. object properties, prototypes, accessors, and typed internal slots;
6. promise state, reactions, and queued jobs;
7. collection entries and live iterator state, with weak keys marked as weak;
8. embedder-owned local handles and callback-retained handles;
9. module state when modules are introduced;
10. temporary construction, descriptor, iterator, and Proxy-trap values that
    survive an allocation point.

## Generic And Optimized Execution Map

The bytecode-first architecture remains the correct outer boundary. The risk is
that several optimization owners can perform semantic work directly.

| Layer | Current owner | Generic fallback status | Migration owner |
| --- | --- | --- | --- |
| bytecode interpreter | `runtime/bytecode/mod.rs` and focused operation modules | base execution path | preserve through AS-06 |
| ordinary object operations | `ObjectHeap` plus `Context` facades | widest current semantic path, but split by value kind | AS-02/AS-03 |
| static property/name caches | `runtime/property/static_names` and object cacheable lookups | guarded misses generally return to value/object helpers | AS-08 central optimizer owner after AS-02 |
| call caches/direct native calls | `CallValueCache`, `CallReference`, `NativeCallTarget`, `runtime/native/function/direct.rs` | most guarded misses return to native-kind or generic value dispatch | AS-08, after common `Call` |
| linear plans/superinstructions | seven files under `runtime/bytecode/linear` | pattern compilation is optional; executor contains specialized member/numeric/property paths | AS-08 equivalence and optimizer-off coverage |
| function fast paths | `bytecode/fast_path.rs` and `runtime/function/fast_path.rs` | optional compilation with normal bytecode fallback | AS-08 equivalence and accounting |
| structured-control specializations | sixteen files under `runtime/bytecode/control`, including many named `*_loop.rs` recognizers | each recognizer may decline, but accepted paths often reproduce property/call/control semantics | AS-08 audit after resumable frames |
| dense array/native built-in paths | object array modules and `runtime/native/function` | a mix of explicit generic fallback and separate implementations | AS-03 first, AS-08 guards second |
| harness opcodes | compiler/runtime handling of `Print` and `AssertThrows` | selected from source names rather than ordinary binding semantics | remove in AS-08; prevent growth in AS-01b |

The compiler recognizes an identifier spelled `print` and a member expression
spelled `assert.throws`, emitting dedicated `BytecodeInstruction::Print` and
`BytecodeInstruction::AssertThrows`. Identifier construction also recognizes an
unbound `Test262Error` name. These are known baseline exceptions. No additional
harness or benchmark name may enter language compilation.

The control specialization directory currently contains:

- reusable control machinery: `try_catch.rs`, `loop_helpers.rs`;
- named recognizers/executors: `array_add_loop`, `array_fill_loop`,
  `block_lexical_loop`, `compound_assignment_loop`,
  `constructor_prototype_loop`, `for_loop`,
  `function_apply_has_instance_loop`, `object_literal_loop`,
  `string_concat_loop`, `switch_for_loop`, `try_catch_loop`,
  `try_finally_loop`, `update_expression_loop`, and `while_loop`.

AS-08 must classify each accepted path as a reusable guarded operation or
remove/replace it. AS-01a does not judge performance from file names alone; it
records these files because they are separate semantic implementations that
need optimizer-on/off equivalence.

## Canonical-Path Decision For New Work

Until the migration tasks land, compatibility changes should follow this
decision sequence:

1. If the feature needs a new object-like value variant, side table, resumable
   activation, weak edge, public handle, or source-shaped fast path, stop and
   select the relevant AS task.
2. For property/call/construct/iteration behavior, enter through the widest
   `Context` facade listed here.
3. If the facade lacks required semantics, extend it once or place the new code
   behind the planned AS-02/AS-03 boundary; do not copy dispatch into a built-in.
4. Keep storage-only operations in their physical owner, but keep observable
   JavaScript ordering, conversion, callbacks, and throws in the semantic layer.
5. Add focused semantic tests before adding a benchmark. Add a benchmark only
   for a meaningful cross-cutting path.
6. Do not add a fast path until the generic path exists and a guard miss is
   demonstrably equivalent.

## Migration Ownership Matrix

| Task | Inputs from this inventory | First deletion or consolidation target |
| --- | --- | --- |
| AS-01b | exact `Value` allowlist, harness-name sites, runtime/frontend boundary, side-table and duplicate-operation allowlists | add mechanical no-growth guards without pretending baseline debt is fixed |
| AS-02a | five object-like variants, object payloads, function/error stores, Promise/collection associations | checked semantic object reference implemented in PR #400; local slot existence is validated without claiming VM identity |
| AS-02b1 | property/internal-method table | object-like get/has boundary implemented in PR #401, including receiver-aware and Symbol-preserving Proxy dispatch |
| AS-02b2 | property/internal-method table | set/define/delete/keys/prototype/descriptor/integrity boundary implemented in PR #403 with ordinary cache tails and Symbol-preserving Proxy dispatch |
| AS-02c | call/construct tables and repeated object-like lists | shared call/construct predicates and dispatch implemented in draft PR #408, including callable/constructable Proxy and bound functions |
| AS-03a | equality/conversion table | one equality and primitive-conversion owner; delete three `SameValueZero` copies |
| AS-03b | key/property/call/iterator tables | move iterator protocol out of bytecode and add spec-level method lookup and invocation operations over the AS-02c internal methods |
| AS-04a/b | completion/error table and inline Error representation | preserve arbitrary throws across native frames; remove ReferenceError prefix parsing; add real errors/spans |
| AS-05a/b | id, clone, store, root, handle, and limit maps | remove ambiguous VM cloning; add VM identity/generation and root/accounting contracts |
| AS-06 | active execution roots and structured nested bytecode | explicit activation/block stacks and suspend/resume results |
| AS-07 | strong weak-collection entries and implicit roots | safe collection with explicit weak edges |
| AS-08 | caches, direct calls, linear/function/control paths, harness opcodes | one optimizer owner, optimizer-off equivalence, and removal of source-name semantics |

## AS-01b Guard Specification

AS-01b should create one focused architecture-check script and tests for that
script. Existing debt should be an explicit, inspectable allowlist; the guard
must fail on growth.

| Guard | Baseline to allow temporarily | Failure condition |
| --- | --- | --- |
| `Value` representation | the exact thirteen variants in `value/kind.rs`, with Function, NativeFunction, HostFunction, Object, and Error marked object-like | any unreviewed enum variant or a second public value enum |
| runtime/frontend separation | no `crate::ast`, parser, or lexer imports under `src/runtime` or `src/bytecode` | a runtime dependency on parser AST/frontend implementation |
| harness source names | compiler recognition of only `print` and `assert.throws`, plus the constructor fallback for `Test262Error` | another compiler/runtime source-name special case or harness opcode |
| harness opcodes | only `Print` and `AssertThrows` | another harness-only bytecode instruction or use site |
| semantic duplicates | three `SameValueZero` owners, one `SameValue` owner, the recorded length/integer helpers, and the AS-02c callable/constructor predicates | a new definition instead of delegation to an existing or new shared operation |
| object side tables | Promise, collection, and iterator associations recorded above; bound-function payload store | a new object-id-indexed association without an inventory/plan update |
| optimization owners | current linear/function/control modules | a new workload-shaped control module or compiler source-shape recognizer without plan evidence |
| clone debt | current `Clone` implementations on `Vm` and `Context` | another public VM-state clone boundary or use of cloning as handle transfer |

The script should report the specific changed boundary and point to this
document. It should run from `scripts/check-fast.sh` and the correctness gate,
remain deterministic, and avoid brittle line-number matching. Structural token
or normalized-text checks are preferable to raw whole-file hashes.

AS-01b implements this contract in
`scripts/check-architecture-boundaries.sh`. Both `scripts/check-fast.sh` and
the correctness/full entrypoint run its self-test mode before compilation. The
self-tests copy only `src/` to temporary fixtures and prove that every guarded
allowlist rejects a representative mutation; they never modify the worktree.

## Reproducible Inventory Checks

The following read-only commands produced the snapshot and are suitable inputs
to AS-01b:

```bash
rg -n 'pub enum Value|Function\(|NativeFunction\(|HostFunction\(|Object\(|Error\(' src/value/kind.rs
rg -n 'crate::ast|crate::parser|crate::lexer' src/runtime src/bytecode
rg -n 'BytecodeInstruction::(Print|AssertThrows)' src/compiler src/runtime src/bytecode
rg -n 'name\.as_str\(\) == "print"|assert\.throws|Test262Error' src/compiler src/runtime
rg -n 'fn (same_value_zero|same_value|semantic_is_callable|semantic_is_constructor)' src/runtime
rg -n 'pub struct Context|collections:|promises:|_object_slots:|_jobs:' src/runtime
rg -n 'trait .*Trace|fn trace|root_set|roots|garbage|collect' src tests
git ls-files 'src/runtime/bytecode/control/*.rs'
git ls-files 'src/runtime/bytecode/linear/*.rs'
```

Snapshot observations:

- runtime and bytecode have no parser-AST imports;
- the compiler has exactly the recorded `print` and `assert.throws` name
  recognizers;
- `Print` and `AssertThrows` each appear in compiler, bytecode type/metrics,
  runtime dispatch, and runtime implementation paths;
- `SameValueZero` has three owners, plus a numeric array helper; `SameValue` has
  a fourth local owner;
- the runtime has no root enumeration or trace contract;
- the structured-control and linear optimizer areas contain sixteen and seven
  tracked Rust files respectively.

## AS-01a Exit Decision

The architecture can continue growing without a rewrite if the next tasks
respect this order:

1. prevent expansion of the recorded split boundaries (AS-01b);
2. add the semantic object facade before adding more exotic object families
   (AS-02a);
3. move existing property and invocation dispatch behind that facade
   (AS-02b/AS-02c);
4. centralize abstract operations and completion semantics before broad
   standard-library growth (AS-03/AS-04);
5. define handle/root/accounting contracts before GC, weak semantics, or public
   object handles (AS-05/AS-07);
6. make frames resumable before expanding async/generator/module execution
   (AS-06);
7. consolidate optimization ownership only after generic semantics are stable
   (AS-08).

The highest rework risk is not the parser or the bytecode format. It is adding
more observable semantics to parallel object, call, conversion, completion, and
side-table paths. This inventory makes those paths explicit and gives each one
a bounded migration owner.
