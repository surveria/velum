# Bytecode Root Registration Performance

This tranche starts from PR #725, commit
`17b35d407823736c878f7830726de7f5ed70f1e3`. The full QuickJS parity objective
remains open. No JIT, new unsafe code, default compiler change, dependency or
version bump is part of this work.

## Current evidence and selected mechanism

Fresh frame-pointer release profiles of Richards, Base64, n-body and hash-map
run sequentially on CPU 0 with no QuickJS execution. These are diagnostic
profiles, not acceptance timings. Richards spends about 5.3% of self cycles in
the slice-based transient-root registration function; hash-map about 3.8%.
N-body has distributed costs, including about 7.5% in binding reads and 4.7%
in linear-plan binding. Those binding costs remain investigation candidates.

Every bytecode instruction previously copied its traceable operand stack into
the transient registry even when the instruction could neither collect nor
invoke user code. The selected change elides that registration only for an
explicit allowlist of non-reentrant stack, literal, truthiness and completion
instructions, and only when automatic GC is not pending. Property access,
binding lookup, coercion, calls and unknown instructions keep full roots.

Finite transient-root budgets deliberately retain the old registrations, so
the optimization does not silently relax a configured resource-limit failure.
Suspended state and every linear segment retain full roots. Both ordinary
interpreter modes use the same safepoint invariant. No collection trigger,
runtime-step charge, ownership identity or root-visitor contract is removed.

## Correctness findings from boundary tests

The new safepoint regressions found a pre-existing lifetime gap: an object
returned from a function with `using` could be collected inside `Symbol.dispose`.
The same test fails on the immutable starting commit, independently of the
optimization. Disposal must explicitly root completions and detached resource
values/methods while callbacks run, including new suppressed errors produced by
earlier callbacks. Async disposal must also keep its result Promise rooted while
its continuation is no longer queued. Promise reaction result resolvers need the
same protection when a handler collects garbage.

The fix uses scoped roots and the existing active-Promise owner, without disabling
collection. Dedicated integration tests exercise synchronous and asynchronous
disposal, return/throw/tail-call operands, pending callbacks, suppression chains,
unobserved result Promises, handler re-entry, root-budget rejection and cleanup in
both interpreter modes. Resource-free function exits keep their empty fast path.

## Frozen acceptance protocol

Artifacts live outside worktrees under
`$HOME/velum-fuzzing-artifacts/performance/bytecode-roots-20260923`.
Build parent and candidate ordinary release runners at identical absolute
source and Cargo target paths, with identical flags and clean builds. Archive
the finished sources, executable hashes, compiler identity and reports.

Reuse all 28 timing cases from the preceding tranche: five sentinels, six
representative programs, six separate holdouts, five embedding controls, and
six selected JetStream programs. Measure parent/candidate followed by
candidate/parent, sequentially on CPU 0 under the shared host lock and without
compilers, fuzzers or correctness CI. Keep the five-second/five-sample prepared
protocol; JSON/tree cases use 15 seconds/three samples and JetStream uses five
seconds/three samples. Retain the 1 ms operation and 10% CV gates. Any retry
repeats the entire affected paired cohort and preserves the invalid attempt.

Require exact typed useful-work checksums where exported, all report/source
identities, and all four 36-worker memory campaigns. Compare corresponding
Velum VM logical counters separately from process RSS/PSS and QuickJS allocator
metrics. Embedding/JetStream rows do not export the prepared programs' output
checksums; their runner verification is not additional equivalence evidence.
Keep all failures and regressions visible, including the preceding tranche's
method-dispatch and collection-index holdouts. These host-specific selected
workloads are not an official JetStream score or a universal speedup claim.

Correctness requires dedicated safepoint/re-entry/limit regressions, relevant
focused Test262, the local fast gate, and exact-base complete correctness CI.
Update this document and README only with reviewed measured outcomes.
