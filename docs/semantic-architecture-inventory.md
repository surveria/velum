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

`Value` currently has eleven variants. Four variants are object-like in
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

Primitive variants are `Undefined`, `Null`, `Bool`, `Number`, `BigInt`,
`String`, and `Symbol`. `BigInt` is an immutable, ownerless,
arbitrary-precision mathematical value whose shared payload has no JavaScript
identity. `String(JsString)` owns exact UTF-16 code units plus a cached UTF-8 or
diagnostic rendering. Portable strings use the same payload without owner
metadata until runtime admission; retained strings and Symbols carry VM-owned
ids or VM-owned shared data even though public `Value` does not encode a VM
identity directly.

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
propagates its explicit receiver through Proxy and accessor paths. AS-09ad
extends the ordinary finish path so every prototype hop re-enters this semantic
dispatch. A Proxy used as an ordinary object's prototype is therefore
observable through `[[Get]]` and `[[HasProperty]]`, and caches decline the
entire chain as soon as they encounter a Proxy instead of bypassing its traps.

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
| RegExp | `regexp_value` on `Object` retains exact UTF-16 source units and flags; the in-tree `src/regress` compiler/executor is transient per construction or execution | RegExp built-ins plus ordinary properties; compilation and matching consume UTF-16 directly and charge the owning context's runtime-step budget without retaining cross-VM compiled state |
| Proxy | `proxy_value` on `Object` | repeated pre-dispatch in get/has/set/delete/prototype/descriptor/call/construct paths |
| ArrayBuffer / SharedArrayBuffer | `byte_buffer` on `Object`; AS-09ag keeps ordinary buffers in `Rc<RefCell<ByteBufferState>>` and explicitly shared buffers in `Arc<parking_lot::RwLock<ByteBufferState>>`, with both storage kinds exposed through one view API | branded buffer accessors/methods plus ordinary properties; local resize/detach and shared grow update the ObjectHeap byte payload total, while only Atomics takes an exclusive shared-store guard across a complete operation |
| TypedArray | `typed_array` on `Object`, carrying a Number- or BigInt-content element kind and referencing a buffer object plus shared byte buffer | typed-array indexed branches plus ordinary properties; AS-09af extends the AS-09j element-kind owner with exact signed/unsigned 64-bit BigInt conversion without adding an edge category |
| DataView | `data_view` on `Object`, carrying a byte range and referencing an ArrayBuffer object plus shared byte buffer | endian-aware Number and BigInt accessors share one byte-range owner; AS-09af extends the AS-09k dispatch without adding an internal slot or edge category |
| Raw JSON marker | `is_raw_json` boolean on `Object` | JSON built-ins |
| Arguments object | ordinary indexed and `length` properties plus an `arguments_brand` boolean; the brand adds no variable-size payload or strong edge | function-call argument materialization creates an ordinary Object-prototype object and installs the standard iterator as an own property; `Object.prototype.toString` reports `[object Arguments]` without a JavaScript-visible marker property |
| ShadowRealm | `shadow_realm: Option<RealmIndex>` on the visible ordinary object | constructor allocation creates and initializes one independent `RealmState`; `evaluate` and `importValue` switch through the shared checked realm boundary, and callable crossings use the traced bound-function owner without exposing target identity |

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
| `[[Set]]` | `semantic_property_write` plus `finish_semantic_property_write`; `semantic_reflect_property_write` for explicit receivers | Static/dynamic caches optimize only ordinary tails; ordinary objects and JavaScript functions share data/accessor descriptor semantics, while AS-03b2 `Set` chooses false versus throw | AS-02b2 complete in PR #403; JavaScript function accessors added by AS-09c |
| `[[DefineOwnProperty]]` | `semantic_define_own_property_*` | ObjectHeap and JavaScript function stores accept shared data/accessor updates; native functions remain a data-only physical backend | AS-02b2 complete in PR #403; JavaScript function accessors added by AS-09c |
| `[[Delete]]` | `semantic_property_delete` plus `finish_semantic_property_delete` | Static/dynamic caches optimize only ordinary tails; primitive fallback remains in `property::delete_property` | AS-02b2 complete in PR #403; AS-03b owns primitive coercion |
| `[[OwnPropertyKeys]]` | `semantic_own_property_keys` plus string/Symbol projections | ObjectHeap and function key stores remain physical backends; Proxy order and Symbol identity are preserved | AS-02b2 complete in PR #403 |
| `[[GetOwnProperty]]` | `semantic_own_property_descriptor` | Object/string/global/function physical descriptor stores remain backends; HostFunction is rejected | AS-02b2 complete in PR #403 |
| `[[GetPrototypeOf]]` | `semantic_get_prototype` | ObjectHeap/function/error prototype owners remain physical backends; derived class constructors expose their static parent and HostFunction is rejected | AS-02b2 complete in PR #403; static class inheritance aligned by AS-09c |
| `[[SetPrototypeOf]]` | `semantic_try_set_prototype` | ObjectHeap stores only `ObjectId` prototypes, so function-valued prototypes remain unsupported storage debt | AS-02b2 complete in PR #403; AS-05 owns handle/storage redesign |
| extensibility/integrity | `semantic_{is,prevent}_extensions` and semantic integrity-level methods | Ordinary objects use ObjectHeap directly; Proxy integrity composes traps and descriptors; function stores still lack extensibility state | AS-02b2 complete in PR #403; AS-05 owns complete accounting/state |

### Known Property Divergences

- `HostFunction` is callable but lacks ordinary function object properties,
  descriptors, a prototype path, and own keys.
- Error properties are synthesized as writable/enumerable/configurable data
  descriptors in some paths rather than stored as real own properties.
- Function and native-function properties share a structure and common
  `ObjectProperty` payload for custom entries, but still require separate
  physical id lookup and dispatch functions. JavaScript functions accept data
  and accessor updates; native functions remain data-only. Their current
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
| string conversion | `Context::to_string`/`to_string_primitive` for well-formed UTF-8 consumers and `Context::to_utf16_string` for exact ECMAScript values; `Function.prototype.toString` is a real intrinsic used by `ToPrimitive` | AS-03a2b routes observable conversion consumers through this owner; UTF-16 paths preserve lone surrogates, while Rust `Display for Value` remains diagnostic rather than an ECMAScript conversion |

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
- lazy prototype helpers, `Iterator.from` wrappers, and static concat/zip
  combinators use typed variants in the same VM-local iterator arena. Static
  combinators retain cached protocol pairs, padding, and keyed output keys as
  traced `IteratorItem` edges with stable creation-time accounting;
- strict joint iteration uses the shared raw-result `IteratorStep` boundary so
  it can verify remaining `done` fields without observing `value`;
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
| Dynamic compilation | one `dynamic_compilation_error` owner converts only eval and Function/AsyncFunction lexer/parser failures to typed `SyntaxError` requests while retaining the dynamic source span; `CompiledScript` retains the parser-derived strictness bit so eval selects its lexical/variable environment shape without reparsing; sloppy direct eval hoists vars into the caller environment and executes lexical declarations in one disposable compiled local scope, strict eval keeps both declaration classes local, and indirect eval adds an explicit global activation boundary; runtime, resource, and existing JavaScript errors remain unchanged | AS-09b merged in PR #451 as `b11ce20f`; PR #484 extends the same owner with eval environment selection |
| Frontend lexical goals | one checkpointed token stream materializes a provisional suffix; parser expression entry selects `InputElementRegExp`, division and division-assignment grammar select `InputElementDiv`, and conflicting speculative lookahead is invalidated before its token is accepted; the buffering strategy may change as long as parser-selected goals remain authoritative | PR #545 replaces eager previous-token slash authority while retaining one token and source-span owner |
| Frontend grammar lookahead | one strict balanced-delimiter query owns current arrow, destructuring, and for-head boundary discovery; each grammar consumer owns only the token required after that boundary; a future cover grammar may replace the query instead of preserving this representation | PR #552 removes the duplicated kind-strict and kind-blind scanners without treating delimiter scanning as a permanent architecture boundary |
| Frontend static values | append-only name and string vectors assign deterministic source-order ids; hash indexes borrow the same immutable `Rc<str>` and `Rc<[u16]>` payloads for deduplication, and failed for-head speculation rolls back only appended names and counters; the hash implementation may change while stable ids retain one value owner | PR #554 removes sorted-index insertion and whole-table speculation clones without adding a representation guard |
| Frontend literal source semantics | string literals retain whether their source contains an escape or line continuation for Directive Prologue decisions; template elements retain distinct exact-UTF-16 cooked and normalized raw values, and no-substitution templates remain template expressions rather than entering StringLiteral-only grammar; token and AST representations may change as long as these source-level distinctions remain observable to their semantic consumers | PR #556 establishes the raw/cooked and directive boundary without guarding a particular enum or field layout |
| Frontend function `arguments` ownership | each ordinary function parses parameters and body in one isolated usage context; nested ordinary functions own their references independently, while arrows inherit the nearest ordinary owner; the parser state and binding-allocation representation may change as long as these ownership semantics and lazy allocation remain intact | PR #559 replaces the parser-global monotonic reference counter without adding a representation guard |
| Frontend syntax preservation | exponentiation rejects every unparenthesized unary-form left operand including `await`; `debugger` retains statement identity and source span through the frontend while compiling as a no-op until an inspector contract exists; invalid `new import...` syntax is rejected without entering the binding namespace; AST variants and parser organization may change while these observable grammar and layering properties remain intact | PR #561 closes three audit residuals without adding representation guards |
| Frontend source and token ownership | the lexer retains one owned UTF-8 source and scans checked character boundaries without a whole-source character side table; buffered identifier, private-name, string, template, and regexp payloads are immutable shared values, so speculative replay and parser consumption reuse their allocation and static names/strings adopt it directly; cursor, smart-pointer, token-buffer, and interning representations may change as long as compilation does not restore redundant whole-source or per-consumption payload ownership | PR #562 removes the audit allocation copies without adding representation guards |
| Bytecode usage accounting | one exhaustive structural fold owns `CompiledScriptUsage` accounting across instruction child blocks, function defaults and hoisted declarations, patterns, classes, catches, and super-property expressions; metric fields and bytecode representations may change as long as every executable child contributes exactly once and adding an instruction cannot silently fall through an unknown-child wildcard | PR #564 replaces six divergent recursive matches and closes the audit undercounting gaps |
| Runtime-to-throw conversion | `runtime_exception_value` unwraps only typed JavaScript values or allocates a typed built-in error request; Runtime, host, parser, and resource errors are never classified by message text | AS-04a merged in PR #416; AS-04b1 object allocation merged in PR #418 |
| Reference errors | `reference_error_undefined` and `reference_error_uninitialized` create typed ReferenceError requests that become ordinary objects in the active VM | AS-04a merged in PR #416; AS-04b1 merged in PR #418 |
| Accessors and native callbacks | Completion conversion preserves primitive, Symbol, object, and Error throws; public host callbacks may use `Error::javascript(value)` intentionally | AS-04a merged in PR #416 |
| Error instances | `Value::Object(ObjectId)` with ordinary properties/prototype plus an `error_metadata` internal slot | AS-04b1 merged in PR #418; no synthetic property or equality path remains |
| Source diagnostics | `CompiledScript` owns deterministic `SourceId`, an optional bounded source name, and one parser-derived strictness bit used by dynamic evaluation; tokens, lexer/parser errors, every recursive frontend AST node, and instruction-aligned bytecode metadata carry canonical `SourceSpan` ranges; identifier tokens retain escape provenance so shared keyword gates reject escaped ReservedWord spellings and contextual recognizers accept only unescaped names; the provenance representation may change while property `IdentifierName` grammar remains distinct from keyword grammar; structured runtime diagnostics expose the executing range | AS-04b2a merged in PR #419; AS-04b2b1 merged in PR #420; AS-04b2b2 merged in PR #421 without retaining source text or AST; AS-09l extends the guarded token metadata without adding another source range; PR #484 adds only strictness metadata, not source retention; PR #557 centralizes escaped keyword and contextual-name decisions |

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

AS-09d adds one explicit grammar seam between full `Expression` and
delimiter-sensitive `AssignmentExpression` contexts. `Expr::Sequence` is the
only comma-expression AST owner; binding layout and capture collection recurse
through it, and the compiler lowers every non-final operand to the existing
`Pop` instruction. Call arguments, declaration initializers, array elements,
object values and computed keys, binding defaults, and class-field initializers
enter through `assignment_expression`, while parentheses, computed-member
access, return/throw, conditions, and expression statements retain the full
expression entrypoint. `for-of` retains an `AssignmentExpression` RHS, parsed
parameter lists carry their simple/non-simple fact until strict and uniqueness
validation, strict `yield` is rejected as an identifier, and an explicit await
context prevents ordinary function bodies from inheriting top-level or async
await capability. No sequence-specific runtime store, opcode, or semantic
facade exists.

## VM Store, Root, And Accounting Map

`Context` is the aggregate owner. AS-05b1a adds one executable direct-root
visitor and a public counted snapshot. AS-05b1b1 adds a typed strong-edge
visitor and an executable callable-store snapshot. AS-05b1b2 extends that
contract across the ordinary object arena. AS-05b1b3 adds explicit strong,
weak, and ephemeron traversal for Promise, collection, and iterator side
stores. AS-05b1c adds scoped roots for Rust-local execution values. AS-05a2d
adds explicit identity- and generation-checked handles for callback captures
and values retained after an embedding call. AS-07a consumes this contract in
one marker and a safe non-moving sweep over sparse indexed arenas.

| Store category | Current fields/owners | Implicit strong edges | Current public accounting |
| --- | --- | --- | --- |
| interned names and text | permanent `atoms`, sparse `strings` and `symbols`, well-known caches | registered Symbols and their keys are direct roots; live Symbol descriptions trace heap strings | logical Atom/HeapString/Symbol, cache, and association counts plus exact UTF-16 bytes and reserved lazy-rendering bytes; AS-07 reclaims unmarked strings/Symbols |
| bindings and closures | globals, builtin globals, activation-owned upvalue frames, captured `with` object-environment chains, `BindingCell(Rc<Mutex<Binding>>)` | binding values, captured cells, and object environment values | logical Binding, ExecutionFrame, and binding-index CacheEntry counts |
| executable functions | sparse function, native-function, bound-function, and host-function arenas plus the native registry | typed AS-05b1b1 edges cover properties, upvalues, super/static/class/new-target state, native id payloads, and bound args/targets; immutable bytecode contains only VM-independent literals; opaque callback captures use AS-05a2d retained handles | logical JavaScriptFunction/NativeFunction/BoundFunction/HostCallback counts plus callable property, metadata-cache, binding, and source-record counts; AS-07 sweeps unmarked arena records |
| objects | sparse `ObjectHeap`, shapes, prototypes, properties, arrays, buffers, typed-array views | typed AS-05b1b2 edges cover named/dense/sparse properties, accessors, prototypes, boxed strings/Symbols, Proxy state, and typed-array buffer-object links; cached prototypes and shape/property-key metadata are direct anchors | logical Object/ObjectProperty/ByteBuffer counts plus shape CacheEntry and anchor Association counts; AS-07 recomputes live payload counters after sweep |
| collections | collection stores with retained kind, object slots, iterator snapshots | typed object associations, strong Map/Set entries and iterator items, weak WeakSet keys, and WeakMap ephemerons | logical Collection/CollectionEntry/CollectionIterator/IteratorItem and Association counts plus asynchronous edge snapshot |
| promises/jobs | promises, object slots, ordinary reactions, detached async-function awaits, generator-id await reactions, thenable-resolution jobs, and the ready-job queue | typed object associations, strong results/handlers/settled values, suspended locals/activations/bytecode values, async-generator request values and result Promises, and direct queued-job roots | logical Promise/PromiseReaction/PromiseJob and Association counts plus suspended ExecutionFrame/Binding/CacheEntry ownership and the asynchronous edge snapshot |
| active execution | one `activation_frames` stack owns call local bases, upvalues, `this`, `new.target`, super, a function-id or owned-block continuation program key, parked interpreter state, a resumable child completion, typed structured-control records, resumable destructuring tasks, and resumable sync/async `yield*` iterator state plus temporary-this/eval-boundary/standalone-bytecode variants; await suffixes move unchanged into reactions or async-generator associations, while both generator kinds retain the same detached activation representation | explicit call/block activations, active and suspended function program roots, parked bytecode/control/pattern/generator operands, live destructuring and delegated iterators, queued async-generator requests, and scoped traceable operand/call/iterator/descriptor/Proxy values | one logical ExecutionFrame per activation, lexical scope, and structured-control record plus suspended-owner reconciliation, Promise/Association request counts, TransientRoot counts, and a fourteen-category root snapshot |
| caches | static name/binding caches, call caches, function fast paths | ids, shapes, native kinds, metadata | logical CacheEntry counts plus hit/miss counters for selected call caches |
| embedder-visible state | output, host callbacks, VM-owned dynamic module loader, `Vm`, public `Value`, retained registry | callback arguments are scoped roots; the loader is an embedder-owned source capability with no JavaScript `Value` edge; opaque captures and durable results use retained handles; raw Values remain compatibility-only and non-durable | logical HostCallback/RetainedHandle/OutputEntry counts; retained handles also participate in root snapshots |
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
pairs. AS-07a's marker ignores primitives, deduplicates heap identities,
drains strong edges, and repeatedly admits ephemeron values whose keys became
reachable until no new edge is found. Sweep physically removes dead weak
entries and reconciles their collection-entry accounting before vacating
unreachable owner records.

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
  heap-admitted `JsString` and every `JsSymbol`. Portable `JsString` values
  carry the same exact UTF-16/Rc payload without an owner until the central
  runtime admission boundary interns them. `checked_value` rejects a foreign
  identity before validating the numeric slot, and Symbol equality includes
  owner identity;
- `Vm::get_global` returns public `Value`, including raw VM-local ids;
- `HostCall` exposes callback-borrowed `LocalValue` arguments containing the
  active owner identity and the raw Value. Arbitrary host JavaScript throws
  are created from this local capability rather than an unowned Value;
- host return validation rejects Function, NativeFunction, HostFunction, and
  Object. Same-VM `String(JsString)` and `Symbol` values remain permitted,
  portable strings are admitted centrally, and foreign owners are rejected
  with callback context;
- every public evaluation `Error::JavaScript` carries its Context identity,
  and host throws retain the `LocalValue` identity. Throw conversion rejects a
  foreign owner before JavaScript can catch or inspect a colliding raw id;
- raw object/function ids returned by eval/get_global still have neither an
  attached VM identity nor generation check and are now documented as
  compatibility-only, non-durable values. Source-proven retained evaluation,
  global, and callback-local APIs replace that boundary for later VM calls;
- `OwnedValue` is the explicit cross-VM half of the API. It owns only
  undefined/null/Boolean/Number/BigInt/String data, copies heap strings, rejects every
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

### Executable Root Set For AS-07

The collector root/trace contract enumerates:

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
| optimizer policy and counters | `runtime/optimizer.rs` | one VM-local mode and stable snapshot own linear and call-cache counters | AS-08a boundary installed; guard rejects another direct state owner |
| static property/name caches | `runtime/property/static_names` and object cacheable lookups | disabled mode leaves caches cold; guarded misses return to semantic value/object helpers | AS-08a mode gate installed; AS-08b audits invalidation evidence |
| call caches/direct native calls | `CallValueCache`, `CallReference`, `NativeCallTarget`, `runtime/native/function/direct.rs` | disabled mode delegates to common `Call`; enabled guard misses use the same path | AS-08a mode gate installed |
| linear plans/superinstructions | eight files under `runtime/bytecode/linear`, including one guarded numeric-array reduction | optional compilation/execution declines in disabled mode; reduction guard misses execute the structured loop | AS-08b retained only reusable bytecode operations |
| function fast paths | `bytecode/fast_path.rs` and `runtime/function/fast_path.rs` | disabled mode omits the optional plan and callback shortcut | AS-08a mode gate installed |
| structured control | four files under `runtime/bytecode/control` plus the central dispatcher | `for`, `while`, `do-while`, `for-in`, `switch`, and `try` use continuation-owned execution and reusable linear plans | AS-08b removed all named whole-loop recognizers and guards zero regrowth |
| dense array/native built-in paths | bytecode array helpers, `runtime/object/array`, `runtime/native/function`, and array built-ins | bytecode/direct-native shortcuts decline in disabled mode; packed algorithms require storage/descriptor guards and otherwise use the same generic abstract operations | AS-08b classified these as operation-shaped rather than source-shaped |
| test harness | ordinary lazy `print` native binding plus JavaScript `assert.throws` harness | normal binding/property/call semantics; no harness bytecode or compiler name recognition | AS-08a removed the legacy opcodes |

The compiler no longer recognizes `print` or `assert.throws` by source spelling.
`print` materializes as an ordinary native global function and can be shadowed;
`assert.throws` is ordinary JavaScript loaded by test support. The remaining
unbound `Test262Error` construction fallback is recorded legacy test support.
Architecture guards require zero harness-only bytecode variants and zero
compiler source-name comparisons, and reject growth in the remaining
`Test262Error` allowlist.

The control directory now contains only `for_in.rs`,
`structured_do_while.rs`, `structured_switch.rs`, and `try_catch.rs`. The
central dispatcher owns structured `for` and `while` execution directly. AS-08b
removed fifteen named recognizer/executor modules plus compiler-generated catch
and try/finally source shapes. The architecture guard fixes the four-file set,
rejects loop/catch/try recognizer markers in compiler and runtime control code,
and mutation-tests that rejection.

The retained dense/native specializations are operation-shaped:

- packed/holey array storage helpers prove array kind, length, descriptor, and
  prototype-sensitive conditions before a bulk operation and return to generic
  property operations on a miss;
- direct native calls are keyed by compiler operands and the VM-local native
  registry, not by source names, and validate the current callable before use;
- callback acceleration accepts general pure bytecode expression trees and
  declines for observable setup, mutation, or unsupported operands;
- flat-map array construction compiles a general parameter/literal numeric
  expression tree, while numeric sort recognizes only the reusable `a - b` or
  `b - a` comparator operation; both retain ordinary callback fallbacks.

The one retained whole-iteration optimization is a linear numeric-array
reduction plan, not a control executor. It accepts arbitrary binding names and
both common unit-increment spellings, charges runtime steps, handles partial
limit failure, and declines for holes, indexed prototype behavior, non-numeric
values, or non-default storage. Optimizer-on/off and focused fallback tests
prove the common semantic path.

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
| AS-06a2a | outer block state and program position | activation-owned block/program-counter/operand continuation merged in PR #439; synchronous function overhead removed in PR #440 |
| AS-06a2b | recursive loop/try/finally state | typed continuation-owned loop/switch/iterator/try records merged in PR #442 with one in-place record per construct |
| AS-06b | pending async execution | explicit suspended outcomes, detached await reactions, same-owner nested/control/pattern resume without side-effect replay, and Context/Vm run/cancel job APIs merged in PR #445 |
| AS-07 | strong weak-collection entries and implicit roots | PR #446 merged with exact-tree validation and canonical report publication |
| AS-08a | caches, direct calls, linear/function/control paths, harness opcodes | PR #447 merged as `bc52a723`; one optimizer owner, disabled-mode fallbacks, and zero harness source semantics are canonical |
| AS-08b | named control recognizers and built-in specialization evidence | PR #449 merged as `7802932e`; source-shaped control execution is removed, one broad guarded reduction remains, and exact-tree/canonical evidence is green |
| AS-09a | unary bitwise NOT compatibility | PR #450 merged as `672d57a2`; 32 new Test262 variants, exact-tree correctness, paired performance, and canonical publication are green |
| AS-09b | dynamic compilation error boundary | PR #451 merged as `b11ce20f`; one typed SyntaxError mapper preserves dynamic source spans, adds 198 full-corpus variants with no removed pass, and has green exact-tree/canonical evidence |
| AS-09c | static class accessor and function descriptor boundary | PR #452 merged as `6f185e75`; shared JavaScript-function descriptors add 236 full-corpus variants / 118 files with no removed pass, and exact-tree/canonical evidence is green |
| AS-09d | comma/sequence expression grammar boundary | PR #453 merged as `1fc9867`; exact-tree run `29142804842` certified tree `a23bd644`, post-merge run `29142896882` published report `20260711T062919Z` in `f2d2113`, and the baseline gained 335 variants / 205 files with no removed pass |
| AS-09e | named function expression self binding | PR #454 merged as \`6e71c38\`; exact CI \`29144428112\` certified tree \`3b280888\`, post-merge run \`29144572464\` published report \`20260711T072934Z\` in \`90ca1cb\`, and the baseline gained 29 variants / 21 files with no removed pass |
| AS-09f | \`SetFunctionName\` and anonymous-definition name inference | PR #455 merged as `6c71713`; exact CI `29145629944` certified tree `4b42cadb`, post-merge run `29145743199` published report `20260711T081229Z` in `ae507a1`, and the full baseline gained 434 variants / 217 files with no removed pass |
| AS-09g | destructuring assignment | draft PR #456 adds typed assignment-pattern leaves to the existing resumable `BytecodePattern` walker, preserves reference/default/iterator ordering and assignment expression results, raises the focused `assignment/dstr` profile from 131/640 to 580/640 variants, and adds 850 full-corpus variants / 512 files with no removed pass |
| AS-09h | synchronous generators | draft PR #457 extends the single continuation owner with generator-start, yielded-value, raw delegated-result, and generic resume completions; suspended generator associations trace the same activation/binding/iterator roots, the focused generator profile reaches 1,042/1,056 variants, and the reviewed full baseline reaches 42,709 variants after 2,885 additions and eleven explicit module-only false-positive removals; shared dynamic-eval, `with`, realm harness, and a real module parse goal remain separately owned gaps |
| AS-09i | asynchronous generators | draft PR #459 reuses the AS-09h detached activation owner with Promise-valued request queues, mode-aware async/sync iterator delegation, generic thenable jobs, distinct intrinsic prototypes, typed await/resumption reactions, and native `Promise.all` aggregation; the focused async-generator profile advances from 228/1,944 to 1,920/1,944 variants and from 119/995 to 980/995 files, while the reviewed full baseline reaches 45,258 variants after 2,550 additions and one explicit module-parse false-positive removal |
| AS-09j | numeric typed-array payload and constructor compatibility | PR #462 replaces the Uint8-only object payload with one element-kind-tagged view over shared ArrayBuffer bytes, preserves the existing internal-slot strong edge, and adds the nine non-BigInt numeric typed array constructors; relative to AS-09i, the reviewed full baseline reaches 45,684 variants after 436 additions and ten explicit prototype-accessor false-positive removals, while the focused TypedArray and TypedArrayConstructors profile passes 356 variants |
| AS-09k | DataView constructor and numeric accessors | draft PR #465 adds an ArrayBuffer-backed DataView payload with a traced buffer-object edge, real prototype accessors, `Symbol.toStringTag`, explicit endian handling, and all non-BigInt get/set methods including Float16; the focused DataView profile advances from 0/1,122 to 710/1,122 variants, and the reviewed full baseline reaches 46,404 after 720 additions with no removed pass; remaining DataView failures are isolated to detachment, resizable/shared buffers, realms/custom new-target prototypes, and BigInt support |
| AS-09l | asynchronous `for await...of` iteration | draft PR #466 adds an explicit async mode to the shared `ForOf` AST/bytecode/control path, moves all classic/for-in/for-of head parsing into one size-bounded `statement/for_statement.rs` owner, awaits async iterator results and sync-fallback values through the Promise job owner, performs resumable destructuring assignment, and awaits `AsyncIteratorClose`; the focused profile advances from 178/2,433 to 2,427/2,433 variants and from 92/1,235 to 1,230/1,235 files. On the current AS-09m RegExp base, the reviewed full baseline advances from 48,510 to 51,351 variants through 2,841 additions and no removals. Four residual variants require block-scoped function-declaration instantiation and two require the `$262.createRealm` harness. |
| AS-09n | `Object.assign` semantic operations | draft PR #470 replaces the built-in-specific string-key copy loop with the shared semantic `[[OwnPropertyKeys]]`, `[[GetOwnProperty]]`, `Get`, and throwing `[[Set]]` owners, including Symbol identity, Proxy ordering, enumerable descriptor filtering, accessors, integrity levels, abrupt completion, and array-exotic `length` writes. The focused profile advances from 48/76 to 76/76 variants and the reviewed full baseline advances from 51,351 to 51,379 variants through 28 additions and no removals. |
| AS-09o | `Object.create` and descriptor conversion semantics | draft PR #472 moves `ObjectDefineProperties` collection onto shared `[[OwnPropertyKeys]]`, `[[GetOwnProperty]]`, `Get`, and `ToPropertyDescriptor` owners, roots the result and complete deferred descriptor list across observable callbacks, converts object-coercion failures into catchable `TypeError` completions, and records one fixed-size `arguments_brand` payload field for the builtin class tag. The focused `Object.create` profile advances from 608/640 to 638/640 variants and from 304/320 to 319/320 files; its sole residual file requires BigInt. On the current Iterator base, the reviewed full baseline advances by 134 variants / 67 files with no removals to 52,511 variants and 27,071 files. |
| AS-09p | `Object.defineProperties` validation and Array exotic descriptors | draft PR #473 moves compatibility checks for data, accessor, and generic descriptors into the shared physical property owner, adds Array index accessor dispatch and specification-ordered `length` shrink/rollback over materialized elements, and keeps deferred descriptor collection atomic. Correct deletion exposes and fixes the JSON reviver false-delete path, while the standard configurable `Symbol.prototype[@@toPrimitive]` removes the prior illegal-redefinition dependency. The focused profile advances from 868/1,264 to 1,260/1,264 variants and from 434/632 to 630/632 files; the four residual variants require BigInt syntax or resizable BigInt64Array support. Relative to current `origin/main` at `1d9263d`, the reviewed full baseline advances by 1,378 variants / 691 unique source paths with no removals to 58,616 variants, and the full runner reports 30,137 conforming files. |
| AS-09q | shared standard `Symbol.species` accessors | draft PR #476 installs one receiver-preserving ephemeral native getter per Array, Promise, ArrayBuffer, Map, Set, RegExp, and `%TypedArray%` constructor through a shared semantic owner. The focused profile advances from 8/58 to 58/58 variants and from 4/29 to 29/29 files; required exact-tree run `29171770363` adds 62 full-corpus variants / 31 conforming files with no removals, producing a 58,682-variant / 30,170-file pass candidate. |
| AS-09r | `Promise.all` constructor and abrupt-completion semantics | PR #478 routes generic capabilities through shared construct/call owners, preserves derived Promise prototypes, assimilates thenables through the Promise job owner, and performs specification-ordered iterator closing and idempotent element resolution. The exact standard `Promise.all` profile advances from 114/196 to 194/196 variants and from 57/98 to 97/98 files; the sole residual file requires unsupported BigInt syntax. Exact-tree run `29172860321` adds 142 full-corpus variants / 71 conforming files with no removals, producing a 58,824-variant / 30,241-file pass candidate. |
| AS-09s | `Array.from` iterable and array-like semantics | PR #480 installs the standard generic constructor method and routes iterator acquisition/closing, construct/call, conversion, result descriptors, and length writes through shared semantic owners. Captured default Array iterators now retain the existing live index source instead of materializing a stale snapshot. The exact profile advances from 8/92 to 87/92 variants and from 4/48 to 45/48 files; two residual files require `$262.createRealm`, and one requires the separately owned non-strict function `this` normalization. Exact-tree run `29173872552` adds 113 full-corpus variants / 58 conforming files with no removals, producing a 66,928-variant / 34,326-file pass candidate. |
| AS-09t | `Array.fromAsync` continuation and lexical capture durability | PR #481 installs `Array.fromAsync` over one VM-owned continuation retained by Promise reactions, reuses async/sync iterator stepping and resumable close, routes mapper/value awaiting through Promise resolution, and uses shared construct, descriptor, and throwing-Set owners. Function/program lexical cells now exist uninitialized before hoisted functions, and upvalue slot growth cannot truncate already captured higher slots. Exact-tree correctness adds 340 variants / 182 files with no removals, and canonical report `20260712T035912Z` closes the tranche. |
| AS-09u | Promise settlement combinators and AggregateError ownership | PR #483 represents `all`, `allSettled`, `any`, and `race` with one combinator kind and one traced element-function state owner. The methods reuse generic capability, observable resolve, iterator-close, reaction, and array owners; generic Promise resolve/reject and AggregateError errors/cause/prototype semantics share the same boundary. Exact-tree run `29179973809` adds 661 variants with no removals; PR #483 merged as `ca0f073`, and run `29180057863` published canonical report `20260712T044049Z` in `7f200dd`, reaching 68,553 variants and 35,357 files. |
| AS-09v | `Promise.prototype.finally` species and reaction capability ownership | PR #485 routes Finally through shared species construction, generic capabilities, observable `then`, and thenable jobs. One traced native-function state category retains Then/Catch Finally handlers and value-thunk/thrower settlements; Promise reactions settle either intrinsic ids or generic resolve/reject values. Exact-tree run `29180709996` adds 98 variants with no removals; PR #485 merged as `180e5be`, and run `29180795284` published canonical report `20260712T051106Z` in `7cd2799`, reaching 68,651 variants and 35,406 files. |
| AS-09w | Promise keyed combinator dictionary ownership | PR #486 adds `allKeyed` and `allSettledKeyed` to the shared combinator kind and traced element-state owner. Shared semantic own-key and descriptor operations select enumerable string/Symbol keys in observable order; one keys/value state builds a null-prototype result with ordinary descriptors, and settled entries reuse the standard settlement-record owner. Exact-tree run `29181722226` adds 168 variants with no removals; post-merge run `29181826750` publishes canonical report `20260712T055326Z` at 68,926 variants and 35,566 files. |
| AS-09x | static Iterator combinator continuation ownership | Draft PR #489 adds `Iterator.concat`, `Iterator.zip`, and `Iterator.zipKeyed` over one typed state variant in the existing VM-local iterator arena. Cached methods, protocol pairs, padding, and keyed output keys are traced and accounted as stable `IteratorItem` edges; stepping and reverse-order close reuse the shared iterator abstract operations, including raw-result strict checks. The exact combined profile advances from 8/228 to 196/228 variants and from 4/114 to 98/114 files; residuals are 28 BigInt syntax variants and four exhaustive longest-mode variants above the current Test262 object budget. |
| AS-09y | shared ArrayBuffer lifecycle ownership | PR #492 replaces the fixed byte-vector owner with one shared resizable/detachable state used by ArrayBuffer, TypedArray, and DataView clones. Branded accessors, `isView`, resize, species-aware slice, and both transfer modes preserve one physical byte owner and update ObjectHeap payload accounting on mutation. The exact focused profile advances from 56/442 to 322/442 variants and from 28/221 to 161/221 files; exact-tree run `29184135161` and canonical report `20260712T073236Z` close the tranche with no lost pass. |
| AS-09z | Test262 assertion harness contract | PR #494 merged as `6fc94d21`; exact-tree run `29185022524` certified tree `ee558043`, and post-merge run `29185109272` published canonical report `20260712T080258Z` in `905eb10d`. The runner-owned syntax-compatible `assert.js` replacement adds the mandatory globals through one compatible source plus bounded runner-only object headroom, and the canonical baseline gains 736 variants / 373 files with no lost pass. |
| AS-09aa | strict property-write reference ownership | Draft PR #497 carries one strictness bit through ordinary static, array-index, computed, logical, compound, update, and destructuring bytecode references, then delegates rejected writes to the shared semantic `Set` owner with explicit throw/false behavior. Strict restricted-identifier early errors and the postfix update line-terminator boundary remain parser-owned. The exact combined profile advances from 1,637/2,014 to 1,749/2,014 variants and from 907/1,159 to 1,009/1,159 files through 112 additions with no removed pass; all 69 engine fixtures, 120 active cases, and 99 QuickJS differential cases remain green. |
| AS-09ab | update-expression numeric coercion ownership | PR #501 merged as `af014a28` and routes binding, static, computed, array-index, private, super, cached, linear, and direct `++`/`--` execution through one `bytecode_update_values` owner. AS-09ae extends that same owner from `ToNumber` to `ToNumeric`, preserving Number behavior while returning the converted BigInt previous/updated pair without repeated observable conversion. Computed references reject a nullish base after evaluating the property expression but before `ToPropertyKey`. Exact-tree run `29188157482` certified merge tree `0ff87c5c`, and post-merge run `29188096231` attempt 2 published canonical report `20260712T095437Z` in `8f1fb7fa`; the baseline gained 134 variants / 67 files with no removed pass. |
| AS-09ac | Number prototype internal-slot and formatting order | PR #504 merged as `4d7a4722` and represents `%Number.prototype%` with the existing boxed-number payload and `+0` data instead of an ordinary object, so receiver validation, primitive conversion, object tagging, and inherited methods share the same physical owner as constructed Number objects. `toExponential` and `toPrecision` convert provided digit arguments before returning non-finite text while retaining non-finite precedence over range validation. The exact Number profile advances from 566/680 to 670/680 variants and from 283/340 to 335/340 files through 104 additions with no removed pass; post-merge run `29188954596` published canonical report `20260712T102015Z` in `7a0b1eb7`. |
| AS-09ad | recursive Proxy get/has and prototype-chain dispatch | Draft PR #506 makes missing `get` traps forward through the receiver-aware semantic `[[Get]]` owner, replaces physical ordinary-prototype traversal in generic and cache-miss paths with semantic recursion, and marks any cache candidate crossing a Proxy as uncacheable. Virtual own properties remain owned by the existing `has_own` boundary, preserving TypedArray, Array, and boxed-string behavior. The exact `target-is-proxy` profile advances from 46/77 to 60/77 variants and from 23/39 to 30/39 files through 14 additions with no active or QuickJS differential regression; remaining semantic failures are independently reproducible descriptor/delete/set exotic-object gaps, plus one module-parser variant. |
| AS-09ae | arbitrary-precision BigInt primitive and numeric ownership | PR #509 merged as `80a6ddce` and adds one immutable ownerless `JsBigInt` payload, scans literals without binary64, and routes `ToNumeric`, arithmetic, bitwise, comparison, update, boxing, constructor, and prototype behavior through shared owners. `RuntimeLimits::max_bigint_bits` bounds materialized results, while zero and unit identities avoid width-dependent allocation. Correct `Array.prototype.toString` exposure also restores the Arguments object to an ordinary Object-prototype representation with ordinary indexed and `length` properties plus an own standard iterator, rather than encoding Array identity as a storage optimization. Exact-tree run `29192247661` adds 857 full-corpus variants / 431 conforming files with no removals; post-merge run `29192379753` publishes canonical report `20260712T121815Z` in `e96fd5fd`. |
| AS-09af | BigInt buffer-view content ownership | PR #513 merged as `03e95ccb` and extends the existing typed-array element-kind owner with `BigInt64Array` and `BigUint64Array`, exact modulo-2^64 signed/unsigned writes, and `Value`-typed reads. One Number/BigInt content discriminator selects `ToNumber` or `ToBigInt` before storage, rejects cross-content construction/species results, and leaves Number fast paths on the same byte owner. DataView reuses the same discriminator for four endian-aware BigInt accessors. Exact-tree run `29194021019` and post-merge run `29194125651` add 1,741 full-corpus variants / 877 conforming files with no removed pass and publish canonical report `20260712T131711Z` in `027f606f`. |
| AS-09ag | synchronized shared-buffer and Atomics ownership | Draft PR #515 represents ordinary and shared backing stores as distinct local and synchronized owners behind one `ByteBuffer` API. Ordinary ArrayBuffer, TypedArray, and DataView access remains lock-free; SharedArrayBuffer uses `Arc<parking_lot::RwLock<_>>`, and each Atomics read-modify-write holds one exclusive guard across the complete operation. SharedArrayBuffer construction, growth, species-aware slice, metadata, Number Atomics, and BigInt Atomics advance the exact built-ins profile from 0/986 to 684/986 variants. The 224-variant agent cluster remains owned by the missing `$262` host-agent harness, immutable-buffer cases remain a separate storage mode, and asynchronous wait queues require an embedder coordinator rather than process-global JavaScript state. |
| AS-09ah | module source goal and graph ownership | Draft PR #517 adds a strict module parse goal, public compiled import/export metadata, an embedder-controlled canonical source loader, VM-local graph instantiation, persistent module scopes, live import aliases, cycle-aware evaluation, stable namespace objects, and checked synchronous settlement of top-level await through the VM-local Promise job queue. One checked Context-local evaluation depth makes top-level module `this` undefined without changing nested call activations. Module records are real checked `VmStorageKind::Module` owners; their scopes and namespace values participate in the existing direct-root boundary. The Test262 runner now executes module variants through this graph API and advances `language/module-code` from 163/602 to 545/602 variants while preserving 121/121 active cases and 99/99 differential cases. Dynamic import, source-phase imports, import attributes, fully asynchronous graph scheduling, and remaining namespace exotic operations stay separate residuals. |
| AS-09ai | Proxy target-invariant ownership | Draft PR #521 adds one descriptor-compatibility owner shared by Proxy `[[DefineOwnProperty]]` and `[[GetOwnProperty]]`, then validates target invariants after successful get, set, has, delete, prototype, extensibility, and prevention traps. `hasOwn`, `for-in`, and `instanceof` now cross Proxy boundaries through semantic descriptors, enumerable keys, and prototype reads instead of physical object shortcuts. The exact `built-ins/Proxy` profile advances from 446/607 to 516/607 variants and from 228/311 to 264/311 files through 70 additions while preserving 121/121 active cases and 99/99 differential cases. Of the 91 residual variants, 73 require the separately owned Test262 cross-realm host API and the remaining 18 depend on String, Function, or Array exotic-object behavior. |
| AS-09aj | VM-local realm state ownership | Draft PR #522 separates VM-shared storage from multiple realm-owned environments. Capability-bearing `RealmId` handles validate the owning `VmIdentity`; `Context` keeps one active `RealmState` plus inactive states that can be swapped without moving shared heap values. Each realm owns its global and builtin binding scopes, global object, native intrinsic registry, core Object/Array prototype working slots, `%ThrowTypeError%`, and generator/async/promise intrinsic prototypes. Bytecode and native functions retain their creation realm, suspended function execution resumes there, and optional identity-call fast paths are disabled once a VM becomes multi-realm until their caches carry realm provenance. Direct-root and storage-accounting guards enumerate every realm state, including inactive globals and intrinsic anchors. Public `Context` and `Vm` APIs create realms, expose their VM-local globals, and evaluate source with restoration after normal or abrupt completion; the Test262 host maps `$262.createRealm()` onto this same engine surface. |
| AS-09ak | ShadowRealm boundary and dynamic module ownership | Draft PR #524 builds `ShadowRealm` on AS-09aj instead of adding another VM or global state owner. Each instance retains one checked `RealmIndex`; standard globals are initialized as configurable properties, source evaluation uses indirect-eval environments, parse failures retain caller-realm `SyntaxError`, and abrupt child completions become caller-realm `TypeError`. Callable values cross through fresh non-constructable wrappers backed by the existing traced bound-function arena, while primitives preserve VM identity and non-callable objects are rejected. A VM-owned, embedder-installed dynamic module loader feeds the AS-09ah graph owner; `importValue` settles a caller-realm Promise with a wrapped export or a caller-realm `TypeError`. Function binding-layout owners remain attached even when optional lookup caches are disabled in multi-realm mode. The exact `built-ins/ShadowRealm` profile advances from 0/124 to 124/124 variants and from 0/64 to 64/64 files while preserving the active and differential gates. |
| AS-09al | frontend binding and lowering fact ownership | Draft PR #535 replaces the compiler's divergent `for` lexical-scope predicate with the binding-layout owner, stamps parser strictness onto ordinary `for-in`/`for-of` assignment targets and delete references, and distinguishes StatementListItem lexical declarations from Statement-position sloppy `let` expressions before applying declaration lookahead. Switch selection now creates and initializes its lexical environment before case tests, with the selection phase, discriminant, and suspended test state retained by the existing continuation-owned control record. Focused engine coverage pins all five boundaries, including escaped `let`, statement-position ASI, and an awaited case test inside a lexical switch scope. |
| AS-09am | TypedArray view-record lifecycle semantics | Draft PR #537 replaces TypedArray delegation through generic Array length/property paths with one explicit view record. Callback, search, reduce, map/filter, slice, copyWithin, reverse, subarray, with, and iterator paths now distinguish the initial internal-length snapshot from live buffer reads and explicit post-coercion/species witness refresh. Raw byte offsets and length-tracking slots remain available while a view is out of bounds; same-buffer slice copies preserve sequential byte aliasing; Array iterator state is reused lazily with TypedArray-specific out-of-bounds validation; and `%TypedArray%.prototype.toString` shares the Array function object. The exact `built-ins/TypedArray/prototype` profile advances from 2,570/2,794 to 2,728/2,794 variants and from 1,293/1,405 to 1,372/1,405 files with no lost pass. The 66 residual variants are explicitly separate surfaces: 42 Intl-backed `toLocaleString`, 16 immutable-buffer support, six large detach fixtures above the current local byte limit, and two comparator-detach coercion cases. |

## AS-01b Guard Specification

AS-01b created one focused architecture-check script and mutation tests for
that script. The guard is a living executable record of architectural
properties. It is not an endorsement of the current representation and must
not make file layout, complete struct shape, helper counts, or source spelling
effectively immutable.

Existing debt may use an explicit, inspectable no-growth allowlist. Canonical
ownership and layering checks should describe the property being protected,
while behavioral tests own completeness details that cannot be verified
without pinning implementation statements.

| Guard | Baseline to allow temporarily | Failure condition |
| --- | --- | --- |
| `Value` representation | one identity-stamped JavaScript string representation and a portable `OwnedValue` with no VM-local ids or identity | reintroducing a second/ownerless JavaScript string form or admitting a VM-local payload to `OwnedValue`; object-like representation may evolve with its architecture evidence |
| runtime/frontend separation | no `crate::ast`, parser, or lexer imports under `src/runtime` or `src/bytecode` | a runtime dependency on parser AST/frontend implementation |
| harness source names | zero compiler comparisons for `print` or `assert.throws`; only the runtime constructor fallback for `Test262Error` remains | another compiler/runtime source-name special case or growth in the recorded fallback |
| harness opcodes | none | any harness-only bytecode instruction or use site |
| sequence expressions | one `Expr::Sequence` parser/compiler owner, explicit assignment/await/parameter early-error contexts, shared binding traversal, ordinary `Pop` lowering, and no dedicated bytecode variant | a second parser/compiler owner, a sequence runtime/opcode path, delimiter drift in `for-of`, or loss of intermediate-value discard semantics |
| named function expressions | one private compiled self-binding scope between the outer and parameter scopes; typed immutable-write policy; direct eval strictness carried by bytecode metadata | a runtime source-name comparison, self binding through an outer/global lookup, a fast path that bypasses the private scope, or loss of strict/sloppy assignment behavior |
| function name inference | one \`compile_expr_with_inferred_name\` compiler owner and one runtime \`set_function_name\` owner; static names travel as bytecode metadata, computed keys use the resolved property key, and lexical self bindings remain separate | another naming owner, parser mutation of anonymous definitions, member/parenthesized assignment inference, Symbol display text used instead of bracketed descriptions, or an accessor prefix assembled outside the shared runtime owner |
| destructuring assignment | one typed `AssignmentPattern` frontend path and one resumable `BytecodePattern` runtime walker shared with binding patterns; assignment references resolve before iterator/property reads, suspend through the shared bytecode state, remain rooted through defaults and `PutValue`, and close every still-live nested iterator on abrupt completion | a second bytecode/runtime pattern tree, assignment-only walker, target evaluation after source reads, loss of iterator close, or assignment mode that discards the RHS result |
| strict property mutations | one parser-owned strict assignment-target validator and one strictness bit on every ordinary bytecode write or delete reference, including `for-in`/`for-of` targets; optimized chains accept only explicit sloppy instructions, while generic strict writes and failed deletes delegate rejection to the shared semantic owners | a property assignment, update, compound, logical, destructuring, loop-target, or delete path that drops strictness, bypasses the semantic operation, accepts restricted strict identifiers, or lets postfix update cross a line terminator |
| frontend lowering facts | binding layout owns the single `for` lexical-scope predicate; parser nodes retain strictness needed by lowering; switch case tests and bodies share one initialized lexical scope and one resumable selection record; sloppy `let` first distinguishes StatementListItem from Statement grammar and then uses declaration lookahead independent of line terminators | a compiler-side copy of a layout/parser fact, case selection before switch lexical initialization, loss of selection state across suspension, or line-terminator-based `let` declaration disambiguation |
| update numeric coercion | one bytecode runtime owner performs shared `ToNumeric` and returns the numeric previous/updated pair for Number or BigInt; every binding, property, array, private, super, cached, linear, and direct update path delegates to it | another update conversion owner, an already-number-only update path, repeated observable conversion, a postfix result that preserves the unconverted operand, or computed-key conversion before nullish-base rejection |
| `with` object environments | binding layout records the applicable lexical `with` suffix per reference; call, bytecode, and eval-boundary activation frames own and trace captured object chains, with eval boundaries starting from an isolated empty chain; one semantic reference path performs `HasProperty`, `@@unscopables`, `Get`, `Set`, delete, and call receiver behavior while static inner lexical bindings retain direct slots | runtime source-name heuristics, caller-dynamic scope leakage, an unrooted closure environment, assignment reference resolution after RHS evaluation, or property operations that bypass Proxy and accessor semantics |
| semantic duplicates | the AS-03a1 equality owner, the AS-03a2 primitive/number/string/boolean owners, the AS-03b1a property-key owner, the AS-03b1b integer/length/index owners, and the AS-02c callable/constructor predicates | a new definition instead of delegation to an existing shared operation |
| object side tables | Promise, collection, and iterator associations recorded above; bound-function payload store | a new object-id-indexed association without an inventory/plan update |
| optimization owners | one direct optimizer-state owner and zero loop/catch/try source-shape recognizers | direct optimizer state access elsewhere or a compiler/runtime workload-shaped recognizer without reusable plan evidence; splitting or consolidating optimization modules is allowed |
| VM clone boundary | no `Clone` implementation on `Vm` or `Context`; one capability identity/generation owner | reintroducing public VM-state cloning, removing the identity owner, or using cloning as handle transfer |
| VM primitive owner boundary | one identity on each StringHeap, SymbolTable, and JsSymbol plus owner metadata on every heap-admitted JsString and central admission/checked-value validation | removing an admitted primitive owner stamp/check, retaining a detached string, or accepting a foreign colliding slot |
| host local-value boundary | LocalValue and HostCall carry the active owner and retained registry; public JavaScript errors retain the owner and throw conversion validates it | accepting an unowned host throw, a foreign bound JavaScript value, or callback retention without the active registry |
| portable owned-value boundary | OwnedValue contains only undefined/null/Boolean/Number/BigInt/String and copies local heap text; BigInt is an immutable ownerless mathematical payload | a Symbol/object/function/id/identity variant or removal of explicit conversion entrypoints |
| retained-value boundary | RetainedValue is non-cloneable and privately carries identity, registry capability, slot generation, and release state; creation is source-proven | exposing raw ids, relabeling arbitrary Value, removing generation/owner checks, or retaining without root participation |
| direct-root boundary | one typed visitor, public bounded snapshots, and scoped transient roots cover durable and temporary VM reachability | bypassing the visitor/snapshot contract or losing scoped temporary ownership; root categories and traversal organization may change with behavioral reachability evidence |
| storage accounting boundary | typed owner categories feed checked counts, payload totals, full reconciliation, public snapshots, and consuming teardown | dropping typed accounting, reconciliation, limits, snapshots, or teardown evidence; categories and owner traversal may change without mirroring exact source statements in the guard |
| callable strong-edge boundary | six categories enumerate every current JavaScript/native/bound function reference slot through one typed visitor; native id-bearing variants have an exact allowlist | removing a callable slot, adding an unreviewed id-bearing native kind, or exposing raw edge ids in the diagnostic API |
| object strong-edge boundary | three categories enumerate named/dense/sparse properties, accessors, prototypes, boxed strings/Symbols, Proxy slots, and typed-view links; cached prototypes and key metadata are direct anchors | removing an object slot/cache root, adding an unreviewed object payload, or folding side-table associations into ordinary object traversal |
| asynchronous edge boundary | eight categories enumerate Promise state/reactions, typed object associations, Map/Set entries, iterator items, WeakSet keys, and WeakMap ephemerons through explicit strength visitors | removing a side-store source, treating weak keys/ephemerons as ordinary strong entries, or exposing raw association ids publicly |

The script should report the specific changed boundary and point to this
document. It should run from `scripts/check-fast.sh` and the correctness gate,
remain deterministic, and avoid brittle line-number matching. Structural token
or normalized-text checks are preferable to raw whole-file hashes.

### Guard evolution policy

- A guard failure requests an architectural review; it does not require the
  implementation to preserve the existing design.
- An intentional architecture pull request may update, replace, add, or remove
  a check or allowlist when it records the new invariant and updates the
  corresponding behavioral or mutation evidence.
- Exact source statements, complete field inventories, exact helper counts,
  and module filename lists are not valid proxies for an invariant unless the
  exact representation is the documented contract.
- Feature pull requests must not weaken unrelated guards merely to pass CI. If
  the feature genuinely changes the architecture, the pull request becomes an
  architecture change and must explain that scope explicitly.

AS-01b implements this contract in
`scripts/check-architecture-boundaries.sh`. Both `scripts/check-fast.sh` and
the correctness/full entrypoint run its self-test mode before compilation. The
self-tests copy only `src/` to temporary fixtures and prove that each retained
rule rejects a representative architectural regression; they never modify the
worktree. Rules and their mutations evolve together with the architecture.

## Reproducible Inventory Checks

The following read-only commands produced the snapshot and are suitable inputs
to AS-01b:

```bash
rg -n 'pub enum Value|Function\(|NativeFunction\(|HostFunction\(|Object\(|Error\(' src/value/kind.rs
rg -n 'crate::ast|crate::parser|crate::lexer' src/runtime src/bytecode
rg -n 'BytecodeInstruction::(Print|AssertThrows)' src/compiler src/runtime src/bytecode
rg -n 'name\.as_str\(\) == "print"|assert\.throws|Test262Error' src/compiler src/runtime
rg -n '\.optimizer|optional_optimizations_enabled' src/runtime
rg -n 'fn (abstract_equality|strict_equality|same_value|same_value_zero|semantic_is_callable|semantic_is_constructor)' src/runtime
rg -n 'pub struct Context|collections:|promises:|_object_slots:|_jobs:' src/runtime
rg -n 'trait .*Trace|fn trace|root_set|roots|garbage|collect' src tests
git ls-files 'src/runtime/bytecode/control/*.rs'
git ls-files 'src/runtime/bytecode/linear/*.rs'
```

Snapshot observations:

- runtime and bytecode have no parser-AST imports;
- the initial snapshot contained the recorded `print` and `assert.throws`
  recognizers and harness opcodes; AS-08a removes them and the guard now
  requires both searches to be empty;
- optimizer policy and counters have one direct state owner, while optional
  runtime paths consult the common Context gate;
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
