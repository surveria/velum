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

`Value` currently has twelve variants. Four variants are object-like in
JavaScript terms and carry four unrelated numeric ids. Built-in Error instances
now use the ordinary `Object(ObjectId)` representation.

| Value variant | Identity and physical owner | Property owner today | Call/construct owner today | Target migration |
| --- | --- | --- | --- | --- |
| `Object(ObjectId)` | `ObjectId(usize)` into `Context.objects: ObjectHeap` | `ObjectHeap`, with Proxy pre-dispatch in `Context` and built-ins | callable/constructable Proxy objects enter `semantic_call`/`semantic_construct` using immutable capability flags | AS-02a facade and AS-02b/AS-02c internal methods |
| `Function(FunctionId)` | `FunctionId(usize)` into `Context.functions` | `FunctionProperties` in `runtime/function/properties.rs` | `semantic_call`/`semantic_construct`, with bytecode functions as the payload backend | AS-02a/AS-02c; retain the function arena as a payload store if useful |
| `NativeFunction(NativeFunctionId)` | `NativeFunctionId(usize)` into `Context.native_functions`, plus `NativeFunctionRegistry` | a second `FunctionProperties` implementation path | common semantic dispatch with guarded direct-native call/construct backends | AS-02a/AS-02c; native code remains a payload behind common methods |
| `HostFunction(HostFunctionId)` | `HostFunctionId(usize)` into `Context.host_functions` | no ordinary property, descriptor, prototype, or own-key path | `semantic_call`; never constructable | AS-02a/AS-02c and AS-05a host-handle boundary |

AS-04b1 stores stable built-in Error class/message metadata in an `Object`
internal slot. The JavaScript-visible `name`, `message`, descriptor, prototype,
mutation, key, extensibility, equality, and identity behavior uses the same
ordinary object paths as other `ObjectId` values.

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
- Error instances enter through their validated `ObjectId`; the former inline
  error representation no longer exists;
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
| Map/Set/WeakMap/WeakSet | `collections: Vec<CollectionData>` whose backing store retains `CollectionKind` | `collection_object_slots: Vec<Option<(CollectionKind, CollectionId)>>` | Map/Set slots trace strongly; WeakSet keys trace weakly; WeakMap key/value pairs trace as ephemerons, although physical reclamation waits for AS-07 |
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
4. Preserve the non-cloneable `Vm` and `Context` owner boundary established by
   AS-05a1.
5. Weak behavior may not be claimed while keys remain strong `Value` entries.

## Property And Internal-Method Map

There is no single current implementation of the full ECMAScript object
internal-method set. `ObjectHeap` is the physical owner for ordinary objects;
`Context` methods add dispatch for functions, errors, primitives, globals, and
proxies; several built-ins repeat the same matches.

| Operation | Widest current facade | Physical and parallel paths | Required owner |
| --- | --- | --- | --- |
| `ToPropertyKey` | `Context::to_property_key` in `runtime/abstract_operations/conversion.rs`; `dynamic_property_key` is the interning/cache facade | AS-03b1a applies string-hint `ToPrimitive`, preserves Symbol identity, routes non-symbol primitives through shared `ToString`, and makes Object/Reflect/Proxy/dynamic bytecode consumers delegate | AS-03b1a complete in PR #412 |
| `Get` | `Context::get` in `runtime/abstract_operations/property_call.rs`; `get_named` only adapts named keys | delegates object-like dispatch to AS-02b1, owns primitive string/prototype behavior, and leaves static caches as guarded backends rather than semantic owners | AS-03b2 complete in PR #414 |
| `GetMethod` | `Context::get_method`; `get_named_method` only adapts named keys | composes shared `Get` with the AS-02c callable predicate; Proxy traps, `@@toPrimitive`, `@@hasInstance`, Object invocation, and Set-record methods delegate | AS-03b2 complete in PR #414 |
| `Set` | `Context::set` with `SetFailureBehavior` | composes receiver-aware AS-02b2 `[[Set]]`; Reflect and Proxy use false-return behavior, while strict RegExp `lastIndex` writes throw | AS-03b2 complete in PR #414; bytecode assignment remains a guarded language-level path |
| `Call` | `Context::call`; `call_value` only converts at native-value boundaries | composes AS-02c `semantic_call` and preserves `Completion`; generic consumers and cache misses delegate while guarded direct backends remain separate | AS-03b2 complete in PR #414 |
| `[[Get]]` | `Context::semantic_property_read[_with_receiver]` plus `finish_semantic_property_read` | AS-02b1 owns object-like dispatch; the AS-03b2 `Get` operation owns specification composition, while static caches optimize only the returned ordinary-object tail | AS-02b1 complete after PR #401 |
| `[[HasProperty]]` | `Context::semantic_property_presence` plus `finish_semantic_property_presence` | AS-02b1 now owns object-like dispatch; static presence caches optimize only the returned ordinary-object tail, while primitive rejection remains in the property layer | AS-02b1 complete after PR #401; AS-03b later owns the abstract operation |
| `[[Set]]` | `semantic_property_write` plus `finish_semantic_property_write`; `semantic_reflect_property_write` for explicit receivers | Static/dynamic caches optimize only ordinary tails; physical accessor/object/function stores remain backends; AS-03b2 `Set` chooses false versus throw | AS-02b2 complete in PR #403 |
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
  than `Context::get` and the semantic presence facade. They are physical
  backends, despite their generic names.
- Static-name caches now receive only explicit ordinary-object tails from the
  semantic facade; the remaining cache code owns shape/version mechanics, not
  value-kind dispatch.

For new compatibility code after AS-03b2, use the abstract `Context` operation
when one exists and extend it once when it does not. Direct `ObjectHeap` access
is acceptable only for storage creation or a proven ordinary-object-only
operation; it must not silently become the only semantic implementation of a
language feature.

## Call And Construct Map

### Call

| Layer | Entrypoint | Coverage and bypasses |
| --- | --- | --- |
| Semantic internal method | `Context::semantic_call` | checked Function, NativeFunction, HostFunction, bound-function, and callable Proxy dispatch with one completion-preserving contract |
| Capability predicate | `Context::semantic_is_callable` | the single `IsCallable`-style decision used by Function helpers, callbacks, Promise, JSON, Reflect, Proxy traps, coercion hooks, `typeof`, and object tagging |
| Abstract operation | `Context::call` and native-value adapter `call_value` | `call` preserves `Completion` over `semantic_call`; `call_value` converts only where the surrounding native API still returns `Result<Value>` |
| Cached calls | `Context::eval_cached_call_completion` and `CallValueCache` | caches Function, NativeFunction, and HostFunction; every guard miss returns to the shared `Call` operation |
| Identifier direct path | `CallReference` and `eval_bytecode_identifier_call_*` | can bypass value dispatch for bytecode functions, cached native kinds, and `NativeCallTarget`; generic case returns to `Context::call` |
| Static member direct path | `runtime/native/function/direct.rs` plus property caches | recognizes native targets and falls back to native-kind or value call paths |
| Accessors/callbacks | accessor, array, collection, JSON, Promise, Proxy, Reflect, and iterator code | callability and dispatch are shared, but several callers still translate arbitrary thrown values into engine errors; AS-04 owns that completion debt |

The direct paths are guarded optimization backends, not alternative semantic
predicates. Architecture checks allow the callable and constructor predicates
only in `runtime/semantic_object/invocation.rs` and reject growth in the former
split entrypoint set. They also fix the AS-03b2 abstract-operation owner set and
reject restoration of the deleted property/call facades.

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
| Rust `PartialEq<Value>` | `value/kind.rs` | representation-level identity/value building block; observable JavaScript callers no longer select an equality relation by using it directly |
| abstract and strict equality | `runtime/abstract_operations/equality.rs` | AS-03a1 owns the relation; AS-03a2a replaces the former boxed-string exception with shared `ToPrimitive` |
| `SameValueZero` | `runtime/abstract_operations/equality.rs` | Map/Set and generic/packed/holey array paths delegate to the same value and numeric operations |
| `SameValue` | `runtime/abstract_operations/equality.rs` | `Object.is` delegates to the shared owner |
| optimized numeric equality | the same owner plus guarded operand-selection paths in bytecode/control/function modules | fast paths call `number_strict_equality`; they may invert the result for `!=` but do not redefine NaN or signed-zero behavior |

AS-03a1 establishes this owner in merged PR #409 and deletes the former local
implementations in the same change. AS-04b1 removes the former structural
inline-error equality case: Error instances now compare by ordinary `ObjectId`
identity without a special equality exception.

### Conversion And Numeric Indexing

| Semantics | Current owner(s) | Split or limitation |
| --- | --- | --- |
| number conversion | `Context::to_number` plus `to_number_primitive` and `string_to_number` in `runtime/abstract_operations/conversion.rs` | AS-03a2a routes generic arithmetic, bitwise/unary/relational operators, Number/Math/global numeric built-ins, numeric String/Array arguments, Date components, JSON boxing, apply/Reflect lengths, Set records, and array sorting through this owner |
| primitive conversion | `Context::to_primitive` and `ordinary_to_primitive` in `runtime/abstract_operations/conversion.rs` | AS-03a2a owns hints, `@@toPrimitive`, callable validation, method order, primitive-result validation, and abrupt completion; Date delegates its exotic method body to shared `OrdinaryToPrimitive` |
| property-key conversion | `Context::to_property_key` in `runtime/abstract_operations/conversion.rs`; `dynamic_property_key` adds known-atom reuse | AS-03b1a owns string-hint primitive conversion and Symbol-preserving dynamic keys; Object, Reflect, Proxy, and dynamic bytecode consumers delegate |
| integer-or-infinity | `Context::to_integer_or_infinity` plus the primitive `integer_or_infinity_from_number` in `runtime/abstract_operations/conversion.rs` | AS-03b1b routes observable Array, String, Number, and buffer arguments through the Context owner; Date and Set reuse the primitive normalization only after their specification-required prechecks |
| length/index conversion | `Context::to_length` and `Context::to_index` in `runtime/abstract_operations/conversion.rs` | AS-03b1b preserves the full safe-integer range for array-like objects and separates specification values from checked `usize`, array-storage, byte-buffer, execution-step, and allocation limits |
| boolean conversion | `to_boolean` in `runtime/abstract_operations/conversion.rs` | AS-03a2b removes `Value::is_truthy`; bytecode control flow, logical operators, callbacks, Proxy traps, RegExp, Set, and the Boolean constructor delegate to one pure owner |
| string conversion | `Context::to_string` plus `to_string_primitive` in `runtime/abstract_operations/conversion.rs`; `Function.prototype.toString` is a real intrinsic used by `ToPrimitive` | AS-03a2b routes observable conversion consumers through this owner; Rust `Display for Value` remains diagnostic, including error reporting rather than ECMAScript property-key semantics |

AS-03a2 is split into AS-03a2a (`ToPrimitive`/`ToNumber`) and AS-03a2b
(`ToString`/`ToBoolean`) because the two groups have independent consumer
graphs and validation surfaces. The merged pair adds 1,714 reviewed passes,
for 35,987 of 102,578 full variants with 95 of 95 QuickJS differential cases.
Merged AS-03b1a adds another 96 property-key variants without losing a prior
expected pass, for 36,083 full variants and the same QuickJS result. Merged
AS-03b1b adds 102 integer, length, and index variants with no loss, for 36,185
full variants. Merged AS-03b2 adds 24 strict RegExp `lastIndex` variants with
no loss, for 36,209 full variants and 95 of 95 QuickJS cases. Merged AS-03b3
adds 12 iterator-closing variants with no loss, for 36,221 full variants and
the same QuickJS result. AS-04a locally adds 332 typed negative/error variants
with no loss, for 36,553 full variants and the same QuickJS result.

## Iterator Map

`runtime/abstract_operations/iterator.rs` is the semantic iterator owner. It
defines the iterator record backends and owns `GetIterator`,
`GetIteratorFromMethod`, `IteratorStep`/`IteratorValue`, and `IteratorClose`.
`runtime/bytecode/for_of.rs` now owns only for-of loop control. Destructuring,
spread, Map/Set/WeakMap/WeakSet, Math.sumPrecise, Object.fromEntries, and Set
algebra delegate to the shared owner.

The current paths are:

- primitive strings and arrays use direct guarded implementations only when no
  installed protocol method is available; callable observable overrides take
  the generic path;
- generic values use `Symbol.iterator`, cache `next`, require object iterator
  and step results, and read `{ done, value }` through shared property access;
- collection/RegExp iterator objects use `Context.collection_iterators`, which
  stores a snapshot and advances through a native `next` function;
- early completion and consumer processing failures invoke the shared close
  path; original JavaScript throws retain precedence over ordinary close
  failures, while resource-limit failures are never suppressed;
- Set algebra no longer has an independent protocol loop.

The remaining iterator representation debt is narrow: the direct array/string
backends stand in for built-in iterator methods that have not yet been
installed as ordinary observable intrinsics. They may remain only behind the
generic method check and must not acquire independent closing or property
semantics. AS-04 owns the remaining conversion of arbitrary thrown values at
native `Result<Value>` boundaries.

## Completion, Exception, And Diagnostic Map

The engine already has a useful JavaScript `Completion` enum with `Normal`,
`Throw`, `Return`, `Break`, and `Continue`. The split occurs at its boundaries:

| Boundary | Current behavior | Required migration |
| --- | --- | --- |
| Public `eval` | every `Throw(Value)` returns a value-preserving `Error::JavaScript`; built-in Error objects also carry structured diagnostic metadata | AS-04a merged in PR #416; AS-04b1 merged in PR #418; AS-05 later adds VM-bound handle identity |
| Native built-ins | `Result<Value>` boundaries preserve arbitrary throws; typed built-in exception requests allocate an ordinary Error object in the active VM before becoming `Throw(Value)` | AS-04a merged in PR #416; AS-04b1 merged in PR #418 |
| Runtime-to-throw conversion | `runtime_exception_value` unwraps only typed JavaScript values or allocates a typed built-in error request; Runtime, host, parser, and resource errors are never classified by message text | AS-04a merged in PR #416; AS-04b1 object allocation merged in PR #418 |
| Reference errors | `reference_error_undefined` and `reference_error_uninitialized` create typed ReferenceError requests that become ordinary objects in the active VM | AS-04a merged in PR #416; AS-04b1 merged in PR #418 |
| Accessors and native callbacks | Completion conversion preserves primitive, Symbol, object, and Error throws; public host callbacks may use `Error::javascript(value)` intentionally | AS-04a merged in PR #416 |
| Error instances | `Value::Object(ObjectId)` with ordinary properties/prototype plus an `error_metadata` internal slot | AS-04b1 merged in PR #418; no synthetic property or equality path remains |
| Source diagnostics | `CompiledScript` owns deterministic `SourceId` plus an optional bounded source name; tokens, lexer/parser errors, every recursive frontend AST node, and instruction-aligned bytecode metadata carry canonical `SourceSpan` ranges; structured runtime diagnostics expose the executing range | AS-04b2a merged in PR #419; AS-04b2b1 merged in PR #420; AS-04b2b2 merged in PR #421 without retaining source text or AST |

Resource limits should continue to bypass JavaScript catch unless the embedding
contract explicitly changes. Host failures and invariant failures also need
typed engine channels rather than becoming catchable based on message text.

`Error::JavaScript` carries a VM-local `Value`, so it is deliberately not a
cross-VM or cross-thread ownership claim. Callers may inspect or return it only
within the owning VM contract. AS-05 replaces raw id-bearing transfer with
checked VM identity/generation and explicit local/owned handle boundaries.

The Test262 negative-case adapter also uses the typed JavaScript error name.
Formatted `Runtime` text is never accepted as a JavaScript exception, and the
architecture guard rejects new raw `ReferenceError:` construction in runtime
source. The complete local AS-04a corpus retains all 36,221 previous expected
variants and adds 332 reviewed variants, for 36,553 of 102,578 with 95 of 95
QuickJS differential cases.

AS-04b1 removes `Value::Error` and all synthetic Error property, descriptor,
key, prototype, integrity, equality, JSON, and host-value branches. Error
constructors and typed runtime exceptions allocate ordinary objects with the
correct intrinsic or `newTarget` prototype; stable class/message diagnostics
live in one Object internal slot. The complete local corpus retains all 36,553
previous expected variants and adds 106 reviewed Object, Array, Promise, Error,
and NativeError variants, for 36,659 of 102,578 with 95 of 95 QuickJS cases.

AS-04b2a adds canonical diagnostic identity without changing executable
semantics. `SourceId` derives from framed source name/text bytes and stays
stable across compiled-script reuse; `SourceSpan` carries half-open byte ranges.
Named and anonymous compilation bind lexer/parser failures to that identity,
while `CompiledScript` retains neither source text nor AST.

AS-04b2b1 makes the same `SourceSpan` the canonical range on tokens and on
every recursive `Expression` and `Statement` node. Parser composition therefore
preserves complete ranges instead of reconstructing offsets later, while
binding analysis and bytecode compilation continue to consume one AST owner.
The AST and source text are still discarded before runtime, and the
architecture guard rejects a second source map or an unspanned frontend root.

AS-04b2b2 lowers those ranges through the single compiler `emit` boundary into
an instruction-aligned `BytecodeBlock` side table. Normal and linear execution
consume the same table; fused operations retain the range of their final
lowered instruction rather than inventing optimizer-only locations. Runtime,
JavaScript, and resource-limit errors remain separate typed channels while
`Error::source_span` and built-in Error metadata expose their optional origin.
No runtime owner retains source text, tokens, or AST.

## VM Store, Root, And Accounting Map

`Context` is the aggregate owner. AS-05b1a adds one executable direct-root
visitor and a public counted snapshot. AS-05b1b1 adds a typed strong-edge
visitor and an executable callable-store snapshot. AS-05b1b2 extends that
contract across the ordinary object arena. AS-05b1b3 adds explicit strong,
weak, and ephemeron traversal for Promise, collection, and iterator side
stores. AS-05b1c adds scoped roots for Rust-local execution values. AS-05a2d
adds explicit identity- and generation-checked handles for callback captures
and values retained after an embedding call.

| Store category | Current fields/owners | Implicit strong edges | Current public accounting |
| --- | --- | --- | --- |
| interned names and text | `atoms`, `strings`, `symbols`, well-known caches | strings/symbol registry retain shared text | logical Atom/HeapString/Symbol, cache, and association counts plus existing string bytes |
| bindings and closures | globals, builtin globals, activation-owned upvalue frames, `BindingCell(Rc<Mutex<Binding>>)` | binding values and captured cells | logical Binding, ExecutionFrame, and binding-index CacheEntry counts |
| executable functions | functions, native functions/registry, bound functions, host functions | typed AS-05b1b1 edges cover properties, upvalues, super/static/class/new-target state, native id payloads, and bound args/targets; immutable bytecode contains only VM-independent literals; opaque callback captures use AS-05a2d retained handles | logical JavaScriptFunction/NativeFunction/BoundFunction/HostCallback counts plus callable property, metadata-cache, binding, and source-record counts |
| objects | `ObjectHeap`, shapes, prototypes, properties, arrays, buffers, typed-array views | typed AS-05b1b2 edges cover named/dense/sparse properties, accessors, prototypes, boxed strings/Symbols, Proxy state, and typed-array buffer-object links; cached prototypes and shape/property-key metadata are direct anchors | logical Object/ObjectProperty/ByteBuffer counts plus shape CacheEntry and anchor Association counts; payload bytes remain AS-05b2b |
| collections | collection stores with retained kind, object slots, iterator snapshots | typed object associations, strong Map/Set entries and iterator items, weak WeakSet keys, and WeakMap ephemerons | logical Collection/CollectionEntry/CollectionIterator/IteratorItem and Association counts plus asynchronous edge snapshot |
| promises/jobs | promises, object slots, reaction queue | typed object associations, strong results/handlers/settled values, and direct queued-job roots | logical Promise/PromiseReaction/PromiseJob and Association counts plus asynchronous edge snapshot |
| active execution | one `activation_frames` stack owns call local bases, upvalues, `this`, `new.target`, super, a function-id or owned-block continuation program key, and future parked interpreter state plus temporary-this/eval-boundary/standalone-bytecode variants; scoped transient registry | explicit call/block activations, active function program roots, parked bytecode operands, and scoped traceable operand/call/iterator/descriptor/Proxy values | one logical ExecutionFrame per activation and lexical scope plus TransientRoot counts and a fourteen-category root snapshot; loop/try/finally control records remain AS-06a2b |
| caches | static name/binding caches, call caches, function fast paths | ids, shapes, native kinds, metadata | logical CacheEntry counts plus hit/miss counters for selected call caches |
| embedder-visible state | output, host callbacks, `Vm`, public `Value`, retained registry | callback arguments are scoped roots; opaque captures and durable results use retained handles; raw Values remain compatibility-only and non-durable | logical HostCallback/RetainedHandle/OutputEntry counts; retained handles also participate in root snapshots |
| nondeterministic/runtime state | clock, random state, step counters | no JS edges | runtime steps and selected execution counters |

The AS-05b1a direct-root registry currently enumerates initialized global,
builtin-global, local, and captured binding cells; active `this`,
`new.target`, and `super` values; global-object and Promise-prototype runtime
anchors; registered reusable native-function ids; and result Promise ids plus
handler/settled values in queued Promise jobs. These are roots owned
independently of heap traversal.

AS-05b1b1's generic `StrongEdgeVisitor<Kind>` uses a private typed target enum
rather than raw integers. Its callable traversal covers every physical strong
slot in JavaScript, native, and bound function stores. AS-05b1b2 reuses the
same visitor for object properties, prototypes, boxed primitives, Proxy
state, and typed-array links. AS-05b1b3 adds typed object-to-side-store
associations plus a separate `WeakEdgeVisitor` contract. Map/Set and iterator
slots remain strong, WeakSet emits weak keys, and WeakMap emits ephemeron
pairs. Counts intentionally include primitive and duplicate strong slots; a
future marker ignores primitives and deduplicates identities. Physical weak
entry reclamation remains AS-07 work.

`RuntimeLimits` retains its source, syntax, step, string, binding, object, and
per-object property limits. AS-05b2c1 adds `VmStorageLimits`, an unlimited-by-
default immutable policy keyed by all twenty-six `VmStorageKind` categories.
AS-05b2c1 enforces atom, heap-string, Symbol, object, buffer, host-callback,
output, and source-record growth. AS-05b2c2 adds Binding,
JavaScriptFunction, NativeFunction, BoundFunction, ObjectProperty, and
CacheEntry enforcement through one VM-local O(1) ledger. AS-05b2c3 extends
that ledger to collection, Promise/job, retained/transient root,
execution-frame, and association owners; Module remains an explicit zero-store
policy. Every snapshot reconciles the ledger and checks all twenty-six owner
totals against the configured policy.

`VmResourceUsage` retains its existing hot counters. AS-05b2a adds a separate
on-demand `VmStorageSnapshot` with twenty-six stable logical owner categories
and checked category/total record sums. AS-05b2b adds an independent per-kind
logical payload-byte array and checked total for directly owned UTF-8 and raw
buffer data. Fixed record layout, capacity, allocator metadata, immutable
compiled artifacts, and opaque host captures are deliberately excluded.
`VmTeardownReport` includes the complete snapshot released by consuming
`Vm::finish`; hard enforcement remains AS-05b2c.

### Public Handle Boundary

- `Vm` and `Context` are non-cloneable as of AS-05a1. The previous clone path
  copied indexed stores while sharing selected binding cells, buffers,
  callbacks, and metadata;
- every Context owns an opaque `VmIdentity`: a private `Rc` capability plus an
  explicit `VmGeneration`. Independent owners cannot alias and no mutable
  process-global JavaScript state or wrapping numeric allocator is required;
- `StringHeap` and `SymbolTable` clone that owner capability into every
  `JsString` and `JsSymbol`. `checked_value` rejects a foreign identity before
  validating the numeric slot, and Symbol equality includes owner identity;
- `Vm::get_global` returns public `Value`, including raw VM-local ids;
- `HostCall` exposes callback-borrowed `LocalValue` arguments containing the
  active owner identity and the raw Value. Arbitrary host JavaScript throws
  are created from this local capability rather than an unowned Value;
- host return validation rejects Function, NativeFunction, HostFunction, and
  Object. Same-VM `HeapString` and `Symbol` values remain permitted, while
  foreign owners are rejected with callback context;
- every public evaluation `Error::JavaScript` carries its Context identity,
  and host throws retain the `LocalValue` identity. Throw conversion rejects a
  foreign owner before JavaScript can catch or inspect a colliding raw id;
- raw object/function ids returned by eval/get_global still have neither an
  attached VM identity nor generation check and are now documented as
  compatibility-only, non-durable values. Source-proven retained evaluation,
  global, and callback-local APIs replace that boundary for later VM calls;
- `OwnedValue` is the explicit cross-VM half of the API. It owns only
  undefined/null/Boolean/Number/String data, copies heap strings, rejects every
  VM-local variant, and can be used as a typed host return after the source VM
  is gone;
- `Vm::root_snapshot` and `Context::root_snapshot` expose checked reference
  counts for fourteen direct-root categories: bindings, active calls, parked
  bytecode frames, queued jobs, runtime anchors, retained handles, and
  transient operand, call, and temporary values. `HostCall::root_snapshot`
  captures the same registry while JavaScript activation roots are live,
  without exposing raw visitor internals;
- `Vm::callable_edge_snapshot` and `Context::callable_edge_snapshot` expose
  checked physical-slot counts for callable stores. Typed edge targets remain
  private, so the diagnostic API does not leak arena ids;
- `Vm::object_edge_snapshot` and `Context::object_edge_snapshot` expose the
  same checked contract for the ordinary object arena without claiming
  Context side-table associations;
- `Vm::async_edge_snapshot` and `Context::async_edge_snapshot` expose checked
  Promise/collection/iterator category counts together with strong, weak, and
  ephemeron totals. Typed associations and forgeable ids remain private.

AS-05b1c closes allocation-point roots without pretending Rust closure
captures are inspectable. AS-05a2d roots embedder-held data through opaque
identity- and slot-generation-stamped handles. Collector-enabled APIs must use
that boundary; legacy raw-result calls cannot authorize arena reclamation.

### Provisional Root Set For AS-05b

The root/trace contract must at least enumerate:

1. global, builtin-global, local, and captured binding cells: direct roots are
   executable in AS-05b1a;
2. active `this`, `new.target`, and super values: AS-06a1 consolidates the
   AS-05b1a vectors into one explicit activation owner;
3. promise queued-job handlers and settled values: direct queue roots are
   executable in AS-05b1a;
4. global-object, Promise-prototype, registered-native-function, cached object
   prototype, iterator-symbol, descriptor/well-known property-key, and shape
   metadata anchors: executable through AS-05b1b2;
5. JavaScript function properties/upvalues/super/static/class/new-target state,
   native id payloads, and bound target/this/arguments: executable in
   AS-05b1b1;
6. object properties/prototypes/accessors/Proxy/boxed primitive/typed internal
   slots: executable in AS-05b1b2;
7. Promise state/reactions, collection entries, and iterator state with weak
   keys classified separately: executable in AS-05b1b3;
8. active operand stacks, native-call arguments, and temporary construction,
   descriptor, iterator, class-field, and Proxy-trap values that survive an
   allocation point: executable through the AS-05b1c scoped bridge, followed
   by durable AS-06 frames;
9. embedder-owned retained handles: AS-05a2d after AS-05b1c;
10. module state: add a direct-root/edge owner when modules are introduced.

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
| AS-02c | call/construct tables and repeated object-like lists | shared call/construct predicates and dispatch completed in PR #408, including callable/constructable Proxy and bound functions |
| AS-03a1 | equality table and numeric fast paths | shared equality owner merged in PR #409; bytecode, Object.is, collections, and arrays delegate to it |
| AS-03a2a | conversion table | shared `ToPrimitive`/`ToNumber` owner merged in PR #410; local method probing and primitive-only number facades are deleted, with all prior Test262 passes retained |
| AS-03a2b | string/boolean conversion sites | shared `ToString`/`ToBoolean` owners merged in PR #411; direct runtime truthiness and semantic concat formatting are deleted |
| AS-03b1a | property-key conversion sites | shared `ToPropertyKey` owner merged in PR #412; dynamic bytecode, Object, Reflect, and Proxy paths delegate, and Rust `Display` no longer defines keys |
| AS-03b1b | integer, length, and index helpers | shared `ToIntegerOrInfinity`, `ToLength`, and `ToIndex` owners merged in PR #413; consumers delegate without silently replacing specification ranges with storage limits |
| AS-03b2 | property and call tables | shared `Get`, `Set`, `Call`, and `GetMethod` operations merged in PR #414; legacy facades are deleted and guarded against return |
| AS-03b3 | iterator map | shared iterator protocol and closing owner merged in PR #415; bytecode owns only loop control and all consumers delegate |
| AS-04a | completion/error table | typed arbitrary-throw round trip and ReferenceError prefix removal merged in PR #416; engine and resource failures stay non-catchable |
| AS-04b1 | inline Error representation | ordinary Error object identity and one metadata internal slot merged in PR #418; `Value::Error` and synthetic semantic branches are deleted |
| AS-04b2a | frontend source diagnostics | canonical source ids, named compilation, and structured lexer/parser errors merged in PR #419 without retaining source text or parser AST |
| AS-04b2b1 | frontend AST source ranges | canonical token ranges and span-bearing expression/statement AST merged in PR #420 without runtime AST retention |
| AS-04b2b2 | bytecode/runtime source diagnostics | instruction-aligned span side tables and structured executing ranges merged in PR #421 across normal and linear execution |
| AS-05a1 | clone and VM owner map | non-cloneable Vm/Context plus opaque owner capability and explicit generation merged in PR #422 |
| AS-05a2a | heap string, Symbol, and host success crossing | identity-stamped VM primitives and central foreign-owner rejection merged in PR #423 |
| AS-05a2b | host callback arguments and JavaScript error crossing | borrowed LocalValue capability plus foreign throw rejection merged in PR #424 |
| AS-05a2c | portable owned primitive values | VM-independent five-variant OwnedValue plus explicit local/evaluation copying merged in PR #425 |
| AS-05b1a | direct roots in bindings, active call vectors, runtime anchors, and queued jobs | nine durable categories plus one executable visitor and counted Context/Vm/HostCall snapshot merged in PR #426 |
| AS-05b1b1 | callable arena edges and native registry anchors | generic typed strong-edge visitor, six callable categories, complete JS/native/bound traversal, and bounded Context/Vm snapshot merged in PR #427 |
| AS-05b1b2 | object arena edges and object/property-key cache anchors | three object categories cover properties, prototypes, boxed strings/Symbols, Proxy state, typed views, and bounded Context/Vm diagnostics; merged in PR #428 |
| AS-05b1b3 | Promise, collection, iterator, weak-key, and ephemeron edges | typed side-store associations, eight strength-classified categories, and bounded Context/Vm diagnostics merged in PR #429 |
| AS-05b1c | transient operand, call, iterator, descriptor, Proxy, and class-key values | scoped RAII registry plus three direct-root categories and collector safepoint contract merged in PR #430 |
| AS-05a2d | retained object/function ids and handles | non-cloneable identity/registry/slot-generation capability, source-proven creation, consuming release, Drop safety-net cleanup, and retained-root category merged in PR #431 |
| AS-05b2a | every current variable-size Context owner and nested logical record | twenty-six-category checked snapshot plus consuming teardown reconciliation merged in PR #432 |
| AS-05b2b | retained payload bytes by the same owner map | independent checked UTF-8/raw-buffer payload map merged in PR #433 without claiming allocator-resident RSS |
| AS-05b2c1 | public owner-limit policy plus payload/top-level arenas | immutable sparse custom policy and pre-commit limits for atoms, strings, Symbols, objects, buffers, callbacks, output, and source merged in PR #434 |
| AS-05b2c2 | binding, callable, property, and cache growth | VM-local O(1) ledger, pre-commit reservations, release/rollback paths, and independent snapshot reconciliation merged in PR #435 |
| AS-05b2c3 | async, root, frame, and association growth | exact collection/Promise/root/frame/anchor transitions plus full snapshot-to-limit reconciliation merged in PR #436 |
| AS-06a1 | parallel call-state vectors | one VM-owned call/temporary/evaluation activation stack merged in PR #438 |
| AS-06a2a | outer block state and program position | activation-owned block/program-counter/operand continuation implemented in draft PR #439 |
| AS-06a2b | recursive loop/try/finally state | explicit durable structured-control continuation records |
| AS-06b | pending async execution | suspend/resume outcomes and embedder-controlled job/frame APIs |
| AS-07 | strong weak-collection entries and implicit roots | safe collection with explicit weak edges |
| AS-08 | caches, direct calls, linear/function/control paths, harness opcodes | one optimizer owner, optimizer-off equivalence, and removal of source-name semantics |

## AS-01b Guard Specification

AS-01b should create one focused architecture-check script and tests for that
script. Existing debt should be an explicit, inspectable allowlist; the guard
must fail on growth.

| Guard | Baseline to allow temporarily | Failure condition |
| --- | --- | --- |
| `Value` representation | the exact eleven internal/runtime variants in `value/kind.rs`, with Function, NativeFunction, HostFunction, and Object marked object-like; one separate five-variant portable `OwnedValue` | any unreviewed runtime variant or a portable variant that retains VM identity/id |
| runtime/frontend separation | no `crate::ast`, parser, or lexer imports under `src/runtime` or `src/bytecode` | a runtime dependency on parser AST/frontend implementation |
| harness source names | compiler recognition of only `print` and `assert.throws`, plus the constructor fallback for `Test262Error` | another compiler/runtime source-name special case or harness opcode |
| harness opcodes | only `Print` and `AssertThrows` | another harness-only bytecode instruction or use site |
| semantic duplicates | the AS-03a1 equality owner, the AS-03a2 primitive/number/string/boolean owners, the AS-03b1a property-key owner, the AS-03b1b integer/length/index owners, and the AS-02c callable/constructor predicates | a new definition instead of delegation to an existing shared operation |
| object side tables | Promise, collection, and iterator associations recorded above; bound-function payload store | a new object-id-indexed association without an inventory/plan update |
| optimization owners | current linear/function/control modules | a new workload-shaped control module or compiler source-shape recognizer without plan evidence |
| VM clone boundary | no `Clone` implementation on `Vm` or `Context`; one capability identity/generation owner | reintroducing public VM-state cloning, removing the identity owner, or using cloning as handle transfer |
| VM primitive owner boundary | one identity on each StringHeap, JsString, SymbolTable, and JsSymbol plus central checked-value validation | removing a primitive owner stamp/check or accepting a foreign colliding slot |
| host local-value boundary | LocalValue and HostCall carry the active owner and retained registry; public JavaScript errors retain the owner and throw conversion validates it | accepting an unowned host throw, a foreign bound JavaScript value, or callback retention without the active registry |
| portable owned-value boundary | OwnedValue contains only undefined/null/Boolean/Number/String and copies local heap text | a Symbol/object/function/id/identity variant or removal of explicit conversion entrypoints |
| retained-value boundary | RetainedValue is non-cloneable and privately carries identity, registry capability, slot generation, and release state; creation is source-proven | exposing raw ids, relabeling arbitrary Value, removing generation/owner checks, or retaining without root participation |
| direct-root boundary | fourteen categories enumerate binding, active-call, parked-bytecode, runtime-anchor, Promise-job, retained-handle, transient operand/call, and temporary sources through one visitor | removing a current root/safepoint source, adding an unreviewed category, or bypassing scoped cleanup and Context/Vm/HostCall snapshots |
| storage accounting boundary | twenty-six logical owner categories map every current variable-size Context store and nested record into checked counts and consuming teardown reconciliation | removing an owner source, adding unreviewed Context storage, changing count semantics into byte estimates, or dropping the teardown snapshot |
| callable strong-edge boundary | six categories enumerate every current JavaScript/native/bound function reference slot through one typed visitor; native id-bearing variants have an exact allowlist | removing a callable slot, adding an unreviewed id-bearing native kind, or exposing raw edge ids in the diagnostic API |
| object strong-edge boundary | three categories enumerate named/dense/sparse properties, accessors, prototypes, boxed strings/Symbols, Proxy slots, and typed-view links; cached prototypes and key metadata are direct anchors | removing an object slot/cache root, adding an unreviewed object payload, or folding side-table associations into ordinary object traversal |
| asynchronous edge boundary | eight categories enumerate Promise state/reactions, typed object associations, Map/Set entries, iterator items, WeakSet keys, and WeakMap ephemerons through explicit strength visitors | removing a side-store source, treating weak keys/ephemerons as ordinary strong entries, or exposing raw association ids publicly |

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
rg -n 'fn (abstract_equality|strict_equality|same_value|same_value_zero|semantic_is_callable|semantic_is_constructor)' src/runtime
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
- the initial snapshot found three `SameValueZero` owners plus numeric array
  helpers and a fourth local `SameValue` owner; AS-03a1 collapses them into
  `runtime/abstract_operations/equality.rs`;
- the runtime has an AS-05b1a direct-root contract plus AS-05b1b1 callable,
  AS-05b1b2 object, and AS-05b1b3 asynchronous typed-edge contracts;
  AS-05b1c adds scoped allocation-point roots and AS-05a2d adds retained
  embedder-handle roots;
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
