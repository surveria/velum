# Velum — Deep Engine Architecture Audit

**Date:** 2026-07-12 · **Commit:** `a2afa9b2` (main) · **Scope:** ~145k lines of Rust (~43k of which is the vendored regress crate)

**Method:** a multi-agent audit of 99 independent passes: 11 deep subsystem readers (frontend, compiler, bytecode, interpreter, object model, async model, built-in libraries ×2, dispatch/optimizer, GC/resource accounting, and a cross-cutting "hack hunt"), after which every medium/high finding was adversarially verified by a separate skeptic agent that re-read the code at the cited coordinates. Nothing was built or executed — reading only.

**Verification outcome:** 88 significant claims → **86 confirmed** (31 high, 55 medium), 2 refuted. Of the confirmed findings, **71 are recorded nowhere** in the project's own stabilization program (AS-01…AS-10); 15 are known debt with an assigned owner.

Per-subsystem detail reports live in [`detail/`](detail/), refuted claims in [`90-refuted.md`](90-refuted.md).

---

## Verdict in one paragraph

This is **not a typical hack-ridden prototype — it is a project of rare discipline**: strict layering (the AST genuinely never survives into the runtime), single semantic owners (equality/conversions/internal methods), checked resource accounting, honest self-documentation of debt, and mechanical boundary guards. The AS-01…AS-08 stabilization program has actually been executed: the claimed invariants are overwhelmingly confirmed by the code. **However**, underneath this disciplined semantic layer sit three systemic problems: (1) **data representation is poorly chosen in four fundamental places** (two string types, a 12-variant Value with 4 id spaces, an Rc\<Mutex\> per variable, linear Map/Set); (2) **optimization discipline leaks exactly where AS-08 did not reach** — there are fast paths outside the OptimizationMode kill switch that duplicate semantics, and they have already accumulated spec divergences (5 copies of `powf`); (3) **the frontend (lexer/parser) was never part of the stabilization program at all** and contains the crudest language deviations (ASI is unimplemented for return/throw, `undefined` is a keyword, regex-vs-division is a blacklist heuristic).

---

## Question 1. How many kludges are there?

More good news than expected: **the grep census of markers (TODO/FIXME/HACK/workaround) across src/ is effectively zero**, and the single test-harness kludge baked into the core — the `Test262Error` fallback for an unbound constructor ([runtime/mod.rs:619](../../src/runtime/mod.rs)) — is frozen by an allowlist, but lives on the hot path of every `new` and is a real spec deviation in embedding scenarios.

Kludges in this project take a different form — **not "fix later" comments, but frozen architectural decisions**:

1. **The boundary-guard script is a census of frozen violations.** `scripts/check-architecture-boundaries.sh` — 2,750 lines, ~30 check families. Some are genuine invariants (empty legacy-facade lists). But ~150 source lines are pinned verbatim, and all 48 `Context` fields and 12 `Value` variants are frozen — the guard fixates the accidental shape of the code, not the invariant, and will fight every refactor. On top of that, the script uses GNU-only utilities and **does not run on macOS — the development platform**.
2. **The parser desugars semantics into the AST** (undocumented): destructured parameters become synthetic `%pattern0%`/`%rest%` params with a body prologue — which **inverts parameter initialization order** (`function f({a}, b = a)` — the default sees a not-yet-unpacked `a`); `undefined` is lexed as a keyword; module imports ride through the AST as forged initializer-less `const x;` declarations and are filtered back out **by string name**.
3. **~160 source-code names are baked into the bytecode** as `NativeCallTarget` hints (`anything.push()` is speculated to be `Array.prototype.push`). This is compile-time specialization by spelling, with collisions and dead checks on the hot path. Only partially documented.
4. **String-keyed name recognition inside the object model**: `__proto__` is a special case in 4 layers (rather than an accessor on `Object.prototype` as the spec prescribes), array `length` is string-compared in 5 modules, and where a path "forgot" the check, the backstop is an **uncatchable engine error** instead of spec behavior.
5. **Benchmark-shaped superinstructions** — more of them than the documentation admits ("one reduction plan"): masked-index accumulate, masked-index `in`, literal-throw try, and a private callback mini-specializer inside `flatMap`. All are signatures of specific benchmark idioms.
6. **Lint-dodging tricks**: the forbidden `as` cast is circumvented by **formatting a number to a string and parsing it back** — on every `Uint8ClampedArray` element write and in Date conversions. Plus a `Box::leak` in the "unreachable" branch of `ScopeIndex::make_owned` to satisfy the no-panic rule — if that branch ever becomes reachable, bindings will be silently written into leaked throwaway vectors.

**Score: 17 confirmed findings of kind "hack"** (+6 low). Nearly all undocumented. That is not many for an engine of this size, but each of items 2–4 is not a local patch — it is a systemic pattern.

---

## Question 2. What is mathematically or logically inelegant?

This is where the main structural debt is concentrated (**27 confirmed findings of kind "inelegant"**). In decreasing order of fundamentality:

### 2.1. Two representations of one string — the worst offender
`Value::String(String)` (UTF-8) and `Value::HeapString(JsString)` (UTF-16 plus a cached **lossy** UTF-8 rendering). Consequences: lone surrogates are silently corrupted to U+FFFD the moment they pass through any UTF-8 consumer (`'\uD800'.trim()`, `encodeURI`); every string is stored twice (the byte limit counts only UTF-8 → the memory budget is understated ~3×); `Value::String` lives entirely **outside** the StringHeap — invisible to limits, interning, and the GC sweep; ~200 dispatch sites over the two variants; equality between the forms is O(n) with re-encoding. Every string method "chooses" which semantics it operates in — a per-call correctness decision.

### 2.2. Value: 12 variants, 4 unrelated object id spaces
Documented debt (AS-02/AS-05), but the audit exposed its **hidden cost**: functions cannot be WeakMap/WeakSet keys (a real-world compat break — `weakMap.set(callback, state)` throws TypeError); `HostFunction` has no properties/prototype; the side tables `promise_object_slots`/`collection_object_slots` are parallel Vecs glued together by convention; the association plumbing is copy-pasted three times with two different slot-reuse strategies.

### 2.3. The instruction set is a Cartesian product, not an orthogonal core
~110 `BytecodeInstruction` variants = (operation × reference form × call convention): 19 call/construct shapes, 6 update, 6 compound, 7 opcodes just for private names. The ISA is closed under the language **by enumeration**, not composition. Direct consequence: six parallel structural traversals in metrics (~1,050 lines to compute six integers) **have already diverged** — five nested-block instructions are silently missing from the public `CompiledScriptUsage` API, and the `_ => 0` arm guarantees every future nested-block instruction will also be miscounted.

### 2.4. A hybrid IR: structured blocks and flat jumps at the same time
if/`?:`/`&&`/`||` use flat jump-patching; loops/switch/try/with and even `??=` are nested `BytecodeBlock` instructions. Resumability (await) is achieved with a "park & re-execute" model with phase enums per construct and **three inconsistent "am I resuming?" mechanisms**. That is a doubled control-flow surface and hand-written resume protocols per syntactic form. A confirmed crash class: a catch parameter with an awaited default **pushes the catch lexical scope twice** on resume → a TDZ error plus a leaked scope that misaligns the whole scope stack ([try_catch.rs:209](../../src/runtime/bytecode/control/try_catch.rs)).

### 2.5. Completion — one flat enum for three protocols
10 variants mix spec completions (Normal/Throw/Break/Continue/Return), the engine's suspension protocol (AS-06), and caller-specific pairs (`Return` vs `ReturnDirect`, `Yielded` vs `YieldedIteratorResult`). Invalid states are representable everywhere and are rejected by stringly-typed runtime errors. Moreover, the `YieldedIteratorResult` payload is **overloaded**: a real iterator-result object for sync generators versus an ad-hoc `{"0": awaitFlag, "1": value}` object for async — a protocol flag smuggled through property reads on a synthesized object.

### 2.6. The global environment — three stores with manual synchronization
One logical binding lives as a cell in `BindingScope`, as a lazy property of the global object, and as a builtin cell; coherence rests on four hand-written sync functions invoked from ~10 scattered sites (the cache layer had to add the call in **five separate branches**). Any semantic write path to globalThis that bypasses these wrappers (`Reflect.set(globalThis,…)`, `Object.defineProperty`, spread) **silently desynchronizes** var bindings from global-object properties. Annex-B hoisting has the same disease (3 partial owners in 2 layers, keyed by owned strings), and one of its walks **crosses activation-frame boundaries** — a function can reach a caller's `var`.

### 2.7. Data structures from the "how not to" textbook
- **Map/Set** — a linear Vec with SameValueZero scans and permanent tombstones: building an n-entry Map is **O(n²)**; a Map used as a queue grows without bound; `Set.prototype.union` is O(n·m).
- **ShapeTable** — append-only with a linear full-slice dedup scan: an object with k properties costs O(k²) time plus permanently quadratic memory after seal/freeze.
- Atom/name interning — sorted Vec + `Vec::insert` = O(n²), everything stored twice.
- The double-entry storage accounting (26 categories) is maintained **by hand at ~122 sites across 31 files** with magic per-record multipliers; the GC sweep reconciles not incrementally but with **two full-heap recounts** per collection.

---

## Question 3. Which micro-optimizations undermine the foundation?

Key conclusion: **the optimization architecture itself is designed correctly** (the generic path is the source of truth, a guard miss must return to it, and a single `OptimizationMode::Disabled` switch serves as the equivalence-proof instrument). Guard discipline in property caches, native-call caches, and linear plans **was confirmed** under inspection. The problem is that the kill switch has **blind spots**, and divergences have already bred exactly there:

| # | Problem | Where | Why it is dangerous |
|---|---|---|---|
| 1 | **Five copies of exponentiation via bare `f64::powf`** — 4 of 5 spec-incorrect | [numeric.rs:81](../../src/runtime/numeric.rs) + quickened/linear/flatMap copies | `1 ** NaN` → 1 (must be NaN), `(-1) ** Infinity` → 1 (must be NaN). The bug is self-consistent across tiers → invisible to disabled-mode testing. The canonical case of "specialization redefined semantics instead of layering over them" |
| 2 | **The second (call-time) function-body matcher is not connected to OptimizationMode and charges no steps** | [fast_path.rs:243](../../src/runtime/function/fast_path.rs) | Disabled mode does **not** cover this dispatch surface → the AS-08 verification instrument is unsound for a whole class of bugs; step limits depend on whether the pattern matched (a `return a+b` body costs 0 steps) |
| 3 | **`Array.prototype.flatMap` contains its own unguarded callback mini-JIT** | [flatten.rs:98–260](../../src/runtime/native/builtins/array/flatten.rs) | Duplicates numeric semantics and binding resolution a third time, runs even in Disabled mode, and a mid-loop bailout **double-charges steps** (the decline is not step-neutral) |
| 4 | **Comparator bytecode-sniffing in `Array.prototype.sort` outside the kill switch** + the packed-sort fast path feeds `sort_by` a non-total comparator | [sort.rs:187](../../src/runtime/native/builtins/array/sort.rs), [storage.rs:237](../../src/runtime/object/array/storage.rs) | Since Rust 1.81 the std sort **panics** on total-order violations: `[1, NaN, 0].sort((a,b)=>a-b)` can abort the process — in an engine where panics are banned by policy |
| 5 | **Guard-regime asymmetry: builtin array paths do not check for accessors** (the bytecode paths do) | [array/mod.rs:337](../../src/runtime/object/array/mod.rs) | `indexOf`/`pop`/`reverse`/`join`/`includes` on an array with `defineProperty(a,1,{get})` silently read a hole and never invoke the getter — two guard regimes for one optimization |
| 6 | **The for-of array fast path is not guarded against a patched `%ArrayIteratorPrototype%.next`** | [iterator.rs:234](../../src/runtime/abstract_operations/iterator.rs) | Polyfills/instrumentation — an observable divergence in for-of/spread/destructuring |
| 7 | **The linear `in` superinstruction answers own-property only**, skipping the prototype chain | [in_operator.rs:146](../../src/runtime/bytecode/linear/in_operator.rs) | `(i & mask) in arr` inside a linear segment yields `false` where the language requires `true` |
| 8 | **Cache invalidation is one global epoch per VM** | [heap.rs:449](../../src/runtime/object/heap.rs) | Any structural mutation anywhere (including every `Array#shift` and the first property of every new object) flushes **all** ICs: the cache tier is effective only in steady-state code — i.e., shaped backwards from real workloads |
| 9 | **Mirrored enums NativeCallTarget ↔ NativeFunctionKind** (100+ variants, a 458-line hand-written bijection, a silent catch-all `_ => Self::String`) + a hand-numbered 330-slot registry | [call_target.rs:413](../../src/runtime/native/function/call_target.rs), [registry.rs:18](../../src/runtime/native/function/registry.rs) | A new builtin = edits in 4 files and 2 dispatch pyramids; a forgotten table row = **wrong cached kind data**, not a compile error |
| 10 | **Speculative operands inside the semantic ISA** (`ArrayLength`, `ArrayIndex*`, NativeCallTarget) | [types.rs:614–639](../../src/bytecode/types.rs) | The specialization decision is made at compile time from spelling, with no runtime feedback, and lives in the immutable shareable bytecode instead of an optimization tier |

Machinery scale: ~9k lines (~6% of the runtime), 31 explicit OptimizationMode checks, 132 `eval_direct_*` entry points. **Conclusion: the optimization layer is trustworthy where AS-08 touched it, and untrustworthy where specialization grew inside builtins or predates the isolation program.** Items 1–4 directly violate the project's own rules (stop-the-line #8–9) and should be fixed before any new optimization work.

---

## Direct spec divergences (confirmed, separate from optimizations)

These are semantic foundation bugs surfaced by the audit (all undocumented unless noted):

- **The GC does not mark instance private fields** (`class C { #x = {} }`) — a value reachable only through a live instance's #field **is swept**; after slot reuse, it silently aliases an unrelated object. Type confusion in a safe-Rust engine. [gc.rs:200](../../src/runtime/gc.rs)
- **Accounting hard-fails on `arguments`**: the grow formula and the recount disagree by 2 on CacheEntry for any function with `uses_arguments()` → the next `storage_snapshot()`/`collect_garbage()`/`finish()` returns Err. The tests never cover this combination. [accounting.rs:441](../../src/runtime/accounting.rs)
- **ASI is unimplemented for restricted productions**: `return\n42` returns 42 (spec: `return; 42;`), `throw\nerr` is accepted, `return 1 2` silently parses as two statements. [statement.rs:599](../../src/parser/statement.rs)
- **`undefined` is a reserved word in the lexer**: `var undefined` or a parameter named `undefined` is a SyntaxError; references bypass scope resolution entirely. [names.rs:67](../../src/lexer/scanner/names.rs)
- **Regex-vs-division is a previous-token blacklist**: `if (x) /re/.test(y)`, `{} /re/.test(x)`, `yield /re/` lex as division. [classification.rs:72](../../src/lexer/classification.rs)
- **`let\nx = 1`** executes as `let; x = 1` (a global write instead of a lexical binding). [statement.rs:123](../../src/parser/statement.rs)
- **Strict `delete` never throws** (the strictness bit exists on writes but not deletes) and **for-in/for-of member targets hardcode strict=false**. [member.rs:111](../../src/compiler/member.rs), [control.rs:409](../../src/compiler/control.rs)
- **The duplicated for-init predicate has diverged**: lexical destructuring in a for-head compiles with `scoped:false` → bindings leak into the enclosing frame and per-iteration semantics are skipped. [control.rs:488](../../src/compiler/control.rs)
- **switch: case tests evaluate outside the block environment** (a TDZ divergence hard-wired into the Switch instruction shape). [builder.rs:491](../../src/binding_layout/builder.rs)
- **for-in does not let a non-enumerable own property shadow an enumerable prototype property**; **a Proxy in the middle of a prototype chain is invisible to enumeration** (both enumeration paths share the bug). [keys.rs:63, 50](../../src/runtime/object/property/keys.rs)
- **`String.prototype.match/search` treat a string pattern as a literal substring**; two of four replace paths skip GetSubstitution (`$&` does not expand). [string_regexp.rs:195](../../src/runtime/native/builtins/string_regexp.rs)
- **The RegExpExec protocol is bypassed**: `re.exec` overrides/subclasses are ignored by match/matchAll/search/test. [regexp.rs:646](../../src/runtime/native/builtins/regexp.rs)
- **JSON is delegated to serde_json with a foreign dialect**: a 128-level recursion limit, `JSON.parse('1e999')` does not produce Infinity, lone-surrogate escapes break in both directions. [json.rs:118](../../src/runtime/native/builtins/json.rs)
- **Date.parse** is a rigid ISO subset that cannot parse the engine's own `toString` output; **local time = UTC** (getTimezoneOffset is always 0) — possibly a deliberate determinism decision, but recorded nowhere as a contract. [date/support.rs:213](../../src/runtime/native/builtins/date/support.rs)
- **Legal JS produces uncatchable engine errors**: `Object.create(function(){})`, `Object.getPrototypeOf('x')`, `[].push.call(5)`, `new Array(1.5)` — the engine-failure taxonomy papers over missing ToObject/wrapper paths. [object_static.rs:474](../../src/runtime/native/builtins/object_static.rs)
- **TypedArray iterators and RegExp matchAll are eager snapshots**, not lazy live iterators (a detach after `.values()` must make next() throw, not keep yielding stale values). [regexp.rs:194](../../src/runtime/native/builtins/regexp.rs)
- **Iterator internal state is an observable own property** with a `\0` prefix plus a fresh prototype per iterator (`getPrototypeOf(m.entries()) !== getPrototypeOf(m.entries())`). [map_set.rs:52](../../src/runtime/native/builtins/map_set.rs)
- **Escaped reserved words still act as keywords** (`if` parses as `if`); **the directive `"use strict"` enables strict mode** (compared post-cooking). [classification.rs:23](../../src/lexer/classification.rs)
- **Template tokens keep no raw text** → tagged templates / `String.raw` are unimplementable on this token model. [scanner/mod.rs:427](../../src/lexer/scanner/mod.rs)

---

## Question 4. Optimization opportunities (after the foundation is solidified)

The agents recorded 31 opportunities; the main ones, grouped (full list in the detail files, "Optimization opportunities" sections):

**Data representation (largest systemic win):**
1. **One string representation** (UTF-16 + Rc, O(1) Value clone) — immediately cheapens value cloning, host calls, roots, ephemerons; deletes ~200 dispatch forks and the whole class of lossy bugs.
2. **Slot-based locals instead of `Rc<Mutex<Binding>>`**: reading a variable currently costs a lock + an Rc clone + a state clone + an allocation (alias-cycle checking on **every read**, although only module linking can create cycles). Locals become a plain `Vec<Value>` in the activation frame; heap cells only for captured bindings. This is the hottest operation in the engine.
3. **Static depth in `BindingOperand::Local {depth, slot}`** — removes the reverse frame scan on every access.

**Data structures:**
4. Hash-backed Map/Set (currently O(n²) to build), HashMap interning for atoms/names (currently sorted-Vec O(n²)), a transition tree for ShapeTable (currently O(k²) time and memory).
5. **Cache the compiled regex in RegExpValue** — the pattern is currently recompiled by the regress engine **on every exec/test** (and once more at construction, discarded).
6. Bulk TypedArray operations through a single `with_exclusive_bytes_mut` (currently element-at-a-time through ObjectHeap lookup + a RefCell borrow per element).

**Caches and plans:**
7. **Generational ids in cache keys** instead of the global epoch — caches survive GC and unrelated mutations; the sweep counts bytes incrementally instead of two full-heap recounts.
8. **Compile linear plans once** (pattern recognition currently runs on every loop entry), cached by Rc identity of the block.
9. Transient-root scopes: per-scope buckets instead of scanning the whole global vector on drop (nested scopes currently unwind quadratically).
10. Reusable iterator-result objects with a known shape instead of a fresh object + two property stores per protocol step.

**Important:** items 7–10 are pointless before items 1–2 — measurements will bottom out in Rc\<Mutex\> reads and string clones.

---

## Recommended work order

1. **Emergency correctness fixes** (small, isolated): mark private slots in the GC; the CacheEntry formula for `arguments`; a total-order comparator in packed sort (or decline on NaN); a single `Number::exponentiate` owner + deletion of the 4 copies; accessor guards in builtin array paths.
2. **Restore the optimization contract**: put the call-time fast_path and the flatMap specializer under OptimizationMode (or delete them until profiles exist), make declines step-neutral, move the sort sniffer under the switch. After that, disabled-mode equivalence proves something again.
3. **Frontend debt as its own program** (an AS analog for the parser): ASI restricted productions, `undefined`-as-identifier, regex-vs-division (lexer driven by the parser), `let\n`, escaped keywords, raw template components. This is currently the least disciplined layer — and the only one with no debt inventory of its own.
4. **String representation** (item 1 of the optimizations — simultaneously a correctness fix for lossy surrogates and memory accounting).
5. **Environment model**: one owner for the global environment (the global env record is the global object, as in the spec), a varEnv/lexEnv split — closes the triple synchronization, the Annex-B cross-frame scan, and the eval heuristics.
6. **Data structures** (Map/Set, ShapeTable, interning) — as benchmark pressure appears.
7. Then — honest optimizations from Question 4 on a foundation that can actually be verified.

---

## Strengths assessment (so the baby stays in the bath)

- The front-end → bytecode → runtime layering is **real** (greps confirm: the runtime imports no AST) — a foundation worth defending.
- The AS-02/AS-03 semantic facades (internal methods, abstract operations) are single owners, confirmed by spot checks; all new compatibility work should keep going through them.
- The resource model (limits, ledger, teardown reports) is conceptually strong and rare in prototypes; the execution needs fixing (the ~122 manual sites), not the idea.
- Async resumability without a second interpreter — works; the debt is in the protocol (Completion, park/re-execute), not the concept.
- Debt self-documentation is the best seen at this scale; the 71 undocumented findings of this audit are candidates for the inventory.

---

## Report files

| File | Contents |
|---|---|
| [detail/01-frontend.md](detail/01-frontend.md) | Lexer, syntax, parser, AST |
| [detail/02-binding-compiler.md](detail/02-binding-compiler.md) | Scope analysis and the bytecode compiler |
| [detail/03-bytecode-value.md](detail/03-bytecode-value.md) | Bytecode model, Value, storage, errors |
| [detail/04-vm-interpreter.md](detail/04-vm-interpreter.md) | VM interpreter core |
| [detail/05-exec-async.md](detail/05-exec-async.md) | Activations, async/generators, promises, bindings |
| [detail/06-object-model.md](detail/06-object-model.md) | ObjectHeap, properties, shapes, semantic facade |
| [detail/07-abstract-ops-builtins-a.md](detail/07-abstract-ops-builtins-a.md) | Abstract operations + Array/String/Object/JSON/Math/Number |
| [detail/08-builtins-b.md](detail/08-builtins-b.md) | TypedArray, Date, Promise, RegExp, collections, Proxy, Atomics |
| [detail/09-dispatch-optimizer.md](detail/09-dispatch-optimizer.md) | Native dispatch, caches, fast paths, optimizer |
| [detail/10-gc-accounting-api.md](detail/10-gc-accounting-api.md) | GC, roots, resource accounting, embedding API |
| [detail/11-hack-hunt.md](detail/11-hack-hunt.md) | Cross-cutting hack census, allowlists, docs drift |
| [90-refuted.md](90-refuted.md) | Examined and refuted claims (2) |

Each detail file: subsystem assessment → strengths → confirmed findings (with the verifier's verdict, file:line evidence, and a fix suggestion) → low-severity notes → optimization opportunities.

> The audit was performed against the `main` snapshot at commit `a2afa9b2`. The material was generated by an automated multi-agent audit; medium/high findings were adversarially verified by independent agents.
