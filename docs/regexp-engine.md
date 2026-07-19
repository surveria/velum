# Native RegExp Engine

## Status

Velum now executes ECMAScript regular expressions through the project-owned
`velum-regexp` crate. The root engine depends on that crate in production and
keeps the vendored `regress` crate only as a development-time behavioral
oracle. `velum-regexp-unicode-gen` remains maintenance-only tooling.

The native backend is a ready integration candidate. On pinned Test262 commit
`64ff467c0c1d60c077995bb7c5f93a9d8cc8ade1`, the expanded RegExp profile passes
all 4,618 selected variants. That includes all 3,756 `built-ins/RegExp`
variants, 476 regular-expression literal variants, 188 SpiderMonkey staging
variants, 140 Annex B variants, 34 RegExp iterator variants, and 24 related
String, statement, and expression variants. The complete ready-PR correctness
gate remains the final integration gate. The retained oracle stays
development-only after integration so future changes can continue to use an
implementation-independent differential check.

The implementation is specification-led. Existing engines may be queried as
behavioral or performance oracles, but their implementation structure is not
the design source for this subsystem.

Runtime storage and built-ins depend on one project-owned `CompiledRegExp`
seam in `regexp_syntax`; production source does not name or call `regress`.
The root integration test links the retained oracle as a development
dependency and compares it with the native backend. The deterministic corpus
now
covers 12,650 syntax and match comparisons: structured short patterns,
seed-reproducible generated expression shapes, Unicode and Unicode Sets
properties, captures, lookarounds, named backreferences, scoped modifiers,
search starts, sticky matching, and exact UTF-16 lone-surrogate and
mid-surrogate positions. Its nested-repetition matrix exposed and now guards a
zero-progress rollback rule: captures written by an empty final iteration are
undone, while backtracking inside that iteration may still select a consuming
alternative.

Two deterministic property-fuzz tests add 20,000 seed-reproducible raw and
structure-aware cases directly at the crate boundary. They cover arbitrary
UTF-16 patterns and inputs, including lone surrogates, all supported matching
modes, valid and invalid start positions, sticky execution, constrained compile
and execution limits, exact error and match coordinates, repeated-execution
determinism, retained-size arithmetic, and host cancellation. Every successful
compilation is executed twice and internal-program or size-overflow failures are
treated as test failures rather than accepted fuzz outcomes.

A bounded AddressSanitizer and sanitizer-coverage Fuzzilli campaign against the
same draft tree generated 1,000 JavaScript programs, accepted 721, performed
28,418 engine executions, and added 469 coverage-corpus samples. It reported
zero crashes, zero timeouts, and zero sanitizer findings. This smoke campaign
does not replace longer continuous fuzzing, but verifies that the persistent
whole-engine target and reproducer pipeline are operational for this backend.

The exact pre-switch parent and native tree were also measured with the same
host, runner, corpus, and exclusive performance lock. On `regexp_baseline`, the
native backend measured 530.69 ms with 0.1% coefficient of variation versus
550.21 ms with 0.3% for the retained backend, about 3.5% lower latency. This
evidence covers the current representative RegExp workload. A separate run
with the in-process reference feature measured native evaluation at 523.11 ms
and QuickJS at 59.47 ms, an 8.79x gap with 0.2% and 0.8% coefficients of
variation. Broader representative and adversarial performance coverage remains
follow-up work; the QuickJS result is a baseline, not a parity claim.

The standalone suite also adapts 41 matcher-level cases from the pinned
Test262 lookbehind corpus under its BSD license. Exact expected UTF-16 spans
and captures cover reverse capture order, alternation priority, atomicity,
nested assertions, greedy and variable-length bodies, backreferences, negative
capture rollback, and word boundaries without using the retained oracle as the
expected-result source.

The native runtime backend implements literals, alternation, captures, greedy
and lazy repetition, anchors, word boundaries, character classes, predefined
classes, numeric and named backreferences, atomic positive and negative
lookaheads, variable-length positive and negative lookbehinds, and Unicode
binary, General Category, Script, and Script Extensions property escapes.
Lookbehind executes through reverse VM instructions, including reverse capture
and backreference semantics, while retaining UTF-16 coordinates. Capture names
use generated Unicode identifier data and are retained as bounded program
metadata. Legacy and Unicode-aware ignore-case modes use separately generated
canonicalization tables and preserve the distinct `u`/`v` property complement
order. Decimal, octal, control, identity, hexadecimal, and Unicode escapes use
separate legacy and Unicode-mode validation, including forward capture counts
and escaped surrogate-pair composition. Unicode 17.0.0 generation currently
emits all 53 ECMAScript binary properties, 38 General Category values, and 176
Script values. It also emits the seven properties of strings defined by
ECMAScript from the pinned emoji sequence sources: 7,906 property-sequence rows
including the deterministic `RGI_Emoji` union. Unicode Sets mode implements
nested union, intersection, subtraction, complement, `\q{...}` string
disjunctions, and properties of strings. One-code-point strings are normalized
into the code-point domain before set algebra; remaining strings use
longest-first matching with explicit backtracking alternatives in both forward
and reverse execution. Scoped `(?ims-ims:...)` modifiers are represented in the
semantic IR and baked into affected VM instructions; they do not mutate shared
executor state. Parser-sensitive Unicode Set algebra observes the same scoped
ignore-case mode before lowering.

Duplicate named captures are accepted only when their parser-recorded
disjunction paths are mutually exclusive. Named backreferences lower to the
bounded set of capture slots sharing that name and select the single slot that
participated in the current path or repetition. Slot selection is charged to
the same execution and host-control budgets as ordinary backreference work.
Legacy mode also accepts Annex B quantifiers on lookaheads. Because assertions
are zero width, lowering retains only the required iterations: a zero minimum
does not leak captures, while a positive minimum executes exactly that bounded
count. Unicode modes and all other assertion kinds reject quantifiers.
Malformed braced quantifiers remain literal text only in legacy mode. Unicode
and Unicode Sets modes reject them, along with unescaped `]`, `{`, and `}`
outside character classes, while preserving valid escaped syntax.

## Crate Boundary

`velum-regexp` is a dependency-light library. It owns pattern parsing, semantic
IR, compilation, immutable Unicode lookup data, matching, captures, and its
logical retained-storage measurement. It does not own JavaScript objects,
`lastIndex`, replacement strings, RegExp iterators, legacy constructor statics,
or realm state. Those remain Velum runtime responsibilities.

`velum-regexp-unicode-gen` is maintenance-only tooling. It never runs from a
normal engine build and is not a runtime dependency. Its complete output is
checked into the repository so an ordinary build requires no network, Unicode
archive, build script, or host-specific cache.

## Semantic Coordinates

Pattern source, match input, match spans, captures, and public start positions
use UTF-16 code-unit offsets. Unicode and Unicode Sets modes decode scalar
values through checked views over those units without changing the public
coordinate system. Lone surrogates remain observable code units wherever
ECMAScript requires legacy behavior.

The parser produces a RegExp-specific semantic IR. The compiler lowers that IR
to a specialized immutable program; it does not emit Velum JavaScript bytecode.
The executor is an explicit stack machine so matching does not consume the Rust
call stack.

## Safety And Resource Contract

Both crates forbid unsafe code. Production and test code use checked indexing,
checked arithmetic, explicit error propagation, and bounded nesting. No input
may cause a process abort, intentional panic, native-stack exhaustion, silent
integer wrap, or unbounded retained allocation.

`CompileLimits::MAXIMUM` and `ExecutionLimits::MAXIMUM` are immutable engine
ceilings. Caller-provided values are constrained to those ceilings at the
public API boundary. Conservative standalone defaults leave bounded headroom
for an embedding to select larger pattern and search budgets without exceeding
the hard ceilings. The standalone pattern default is 65,536 UTF-16 units; the
33,554,432-unit hard ceiling matches the existing maximum VM source and string
profiles needed by official conformance stress cases. Velum uses that headroom
only behind the VM-owned source, string, storage, and runtime-step limits. Wide
disjunction compilation is iterative rather than recursive; structural
recursion remains protected by the enforced nesting ceiling.

Compilation limits cover at least:

- pattern UTF-16 units;
- parser nesting;
- semantic IR nodes;
- captures and named-capture payload;
- character-class ranges and string alternatives;
- Unicode Set expression depth, evaluation work, and retained tree storage;
- emitted instructions and auxiliary table bytes.
- ECMAScript repeat counts through `Number.MAX_SAFE_INTEGER`, with oversized
  executable work represented by a bounded guard instead of instruction
  expansion.

Execution limits cover at least:

- executed instructions;
- candidate start positions;
- backtrack frames and undo-log entries;
- capture slots;
- lookaround nesting;
- temporary input and Unicode-set work.

The standalone crate reports structured syntax, compile-limit, execution-limit,
and interruption errors. Velum supplies an execution-control adapter that
charges the VM runtime-step budget and observes cancellation. Resource failures
remain non-catchable embedder errors, matching the existing VM architecture.
Long terminal repetitions avoid retaining one backtrack and undo record per
consumed code point when the end assertion makes the branch deterministic.
Whole-pattern literals, code-point classes, dot atoms, and their simple
quantifiers use a bounded specialized execution plan with constant
backtracking storage. The same local and host-owned work limits apply to that
plan.

## Matching Strategy

The correctness baseline is a bounded backtracking VM because ECMAScript
backreferences, lookarounds, capture rollback, and legacy behavior are not a
pure regular-language surface. A specialized plan handles an entire simple
literal, code-point class, dot atom, or simple quantifier without allocating
backtracking state. More general optimization remains representation-neutral:
literal prefixes, required first sets, anchored starts, compact ASCII classes,
and allocation-free successful paths.

A future linear engine may handle a proven regular subset, but it must use the
same parser, semantic IR, Unicode data, result type, and resource contract. The
backtracking VM remains the semantic fallback. Optimization must never select a
different observable capture or matching order.

## Unicode Provenance

Unicode maintenance pins an explicit Unicode and Emoji version. A tracked
manifest records every source URL, SHA-256 digest, generator version, output
format version, and output digest. Inputs are archival versioned URLs, never a
`latest` alias.

Generation validates scalar bounds, sorted non-overlapping intervals, alias
uniqueness, case-mapping shape, script coverage, and every property permitted
by the ECMAScript specification. It rejects properties that ECMAScript does not
permit. Generated data is split or packed so project-authored Rust files remain
within the source-size gate.

Unicode version updates are explicit maintenance changes with regenerated
artifacts, focused property tests, full RegExp Test262 coverage, and a recorded
behavioral delta.

## Validation Matrix

Replacement requires all of the following:

1. Specification-derived parser, compiler, executor, UTF-16, Unicode, capture,
   and resource tests in `velum-regexp`.
2. Deterministic generator golden tests plus corruption, truncation, duplicate,
   overlap, alias, and version-mismatch tests.
3. Any licensed behavioral cases imported from another project must record
   source and license provenance; implementation code is not transliterated.
4. Generated differential cases compared with the retained oracle and QuickJS,
   including syntax acceptance, failure class, full match, captures, names, and
   UTF-16 spans.
5. Complete pinned Test262 RegExp and Unicode-property coverage with no lost
   pass in the full corpus.
6. Parser and executor fuzz targets, structured pattern generation, corpus
   minimization, and permanent regression fixtures for every discovered issue.
7. Adversarial tests for catastrophic backtracking, empty quantified matches,
   deep grouping, huge counts, capture rollback, lookaround, lone surrogates,
   Unicode Sets, string properties, cancellation, and every configured limit.
8. Direct embedding tests proving independent VM budgets, immutable shared data,
   retained-storage accounting, teardown, and non-catchable resource errors.
9. Benchmarks against both the retained oracle and QuickJS across literal search,
   alternation, classes, captures, lookarounds, backreferences, Unicode, failure,
   and adversarial bounded workloads.

The native path is now the runtime default. The vendored compatibility oracle
remains development-only and intentionally available for future differential
testing; it is not part of the production dependency graph or safety boundary.
