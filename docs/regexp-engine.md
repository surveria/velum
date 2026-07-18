# Native RegExp Engine

## Status

Velum currently executes ECMAScript regular expressions through the vendored
`regress` crate. The project-owned `velum-regexp` and
`velum-regexp-unicode-gen` crates are being developed beside that path. The
runtime must not switch until the replacement gates in this document pass.

The implementation is specification-led. Existing engines may be queried as
behavioral or performance oracles, but their implementation structure is not
the design source for this subsystem.

The current native slice implements literals, alternation, captures, greedy
and lazy repetition, anchors, word boundaries, character classes, predefined
classes, and Unicode binary, General Category, Script, and Script Extensions
property escapes. Legacy and Unicode-aware ignore-case modes use separately
generated canonicalization tables and preserve the distinct `u`/`v` property
complement order. Unicode 17.0.0 generation currently emits all 53 ECMAScript
binary properties, 38 General Category values, and 176 Script values. This is
an in-progress compatibility surface, not yet a runtime replacement.

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

Compilation limits cover at least:

- pattern UTF-16 units;
- parser nesting;
- semantic IR nodes;
- captures and named-capture payload;
- character-class ranges and string alternatives;
- emitted instructions and auxiliary table bytes.

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

## Matching Strategy

The correctness baseline is a bounded backtracking VM because ECMAScript
backreferences, lookarounds, capture rollback, and legacy behavior are not a
pure regular-language surface. Initial optimization is representation-neutral:
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
3. Licensed behavioral cases imported from `regress` only with source and
   license provenance recorded; implementation code is not transliterated.
4. Generated differential cases compared with the existing engine and QuickJS,
   including syntax acceptance, failure class, full match, captures, names, and
   UTF-16 spans.
5. Complete pinned Test262 RegExp and Unicode-property coverage with no lost
   pass in the full corpus.
6. Parser and executor fuzz targets, structured pattern generation, corpus
   minimization, and permanent regression fixtures for every discovered issue.
7. Adversarial tests for catastrophic backtracking, empty quantified matches,
   deep grouping, huge counts, capture rollback, lookaround, lone surrogates,
   Unicode string properties, cancellation, and every configured limit.
8. Direct embedding tests proving independent VM budgets, immutable shared data,
   retained-storage accounting, teardown, and non-catchable resource errors.
9. Benchmarks against both the current engine and QuickJS across literal search,
   alternation, classes, captures, lookarounds, backreferences, Unicode, failure,
   and adversarial bounded workloads.

The runtime switches only after these gates pass on the exact integration tree.
The vendored dependency and its compatibility oracle remain available until the
new path is the default and the complete correctness gate is green.
