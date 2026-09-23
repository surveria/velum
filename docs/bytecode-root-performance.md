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

## Reviewed results (2026-09-23)

The accepted matrix is `paired-v2/acceptance-1` under the external artifact root.
It contains 112 valid timing observations and four complete 36-worker memory
campaigns. All 116 command statuses, report identities, configurations, output
checksums where exported, and engine/reference CV gates passed validation.
The external report validator also passes 74 acceptance/rejection fixtures.

The parent is `17b35d407823736c878f7830726de7f5ed70f1e3`, tree
`19dd71f7d94ea53a28c499a61a166633d60439d4`. The measured candidate is
`f08be97dec8d145a944215a2b16a68e7a8448f0e`, tree
`0907fd8c6dfc714cdf1e56de8bfa3f52f1c07dbb`. Subsequent PR changes only document
the reviewed results. Saved runner SHA-256 hashes are:

- Parent: `f7fd17a4109354573311d7c7780ad96419a435d341ff96a138eafc47e5aeabf6`.
- Candidate: `61a58f11a97d382a729c4fcaf41bfb75c29ae86353a56ed7ee3e906f81b5916a`.

Both builds use the ordinary release profile, Rust 1.96.0 / LLVM 22.1.2 and the
same absolute source/target paths. Measurements use CPU 0 on an AMD Ryzen 9
9950X3D. The frozen protocol remains in the archived candidate source, SHA-256
`090850a94e54df01e8ee75d1ab9a943a14a9d8edef2992ff25d9290ae04e8981`.

Changes below are geometric means of paired median time ratios. Negative is
less execution time. They are host/cohort observations, not confidence intervals
or an official JetStream score.

| Cohort | Cases | Time change | First pair | Reverse pair |
| --- | ---: | ---: | ---: | ---: |
| Sentinels | 5 | -1.1% | -1.5% | -0.7% |
| Representative programs | 6 | -3.1% | -2.9% | -3.2% |
| Separate holdouts | 6 | -2.3% | -2.4% | -2.2% |
| Direct embedding API | 5 | -2.4% | -2.5% | -2.2% |
| Selected JetStream programs | 6 | -2.4% | -2.4% | -2.4% |

| JetStream case | Parent seconds | Candidate seconds | Time change | Parent / QuickJS | Candidate / QuickJS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Richards | 5.233 | 5.067 | -3.2% | 36.64x | 35.38x |
| hash-map | 6.687 | 6.601 | -1.3% | 28.56x | 28.05x |
| SunSpider Base64 | 2.709 | 2.663 | -1.7% | 23.98x | 23.46x |
| SunSpider n-body | 3.016 | 2.951 | -2.1% | 33.85x | 33.02x |
| js-tokens | 1.259 | 1.245 | -1.1% | 10.07x | 9.81x |
| SunSpider tagcloud | 1.210 | 1.148 | -5.1% | 10.82x | 10.09x |

All five cohort means improve in both paired orders. The worst individual
paired-mean change is the function-call sentinel at +0.2%; property-read and
host-payload controls are effectively unchanged. Some individual pair results
still move in opposite directions. Reference timings also vary independently:
the ratios use each build's own paired QuickJS observation, never another
campaign's reference. No universal or statistical-significance claim is made.

Prepared-program checksums match exactly across all four observations, including
typed u64 number bits. Embedding/JetStream rows do not export those checksums.
All 144 memory workers pass, with identical corresponding VM logical counters
in every phase and zero logical records/payload after owner teardown. Selected
paired-median RSS deltas range from -0.146 to +0.123 MiB; RSS/PSS and QuickJS
allocator data remain separate from Velum's logical accounting.

### Rejected implementation and experiment accounting

The first candidate, `c01854a4`, completed a valid full matrix but was rejected
as an optimization: representative workloads +1.1%, holdouts +1.6%, selected
JetStream +0.6%; Richards/hash-map +2.8% and n-body +4.7%, despite tagcloud -5.4%.
Its 112 timing observations and 144 memory workers remain in `paired/acceptance-1`.

The revised implementation reuses `TransientRootScope`'s existing inactive state
instead of adding an outer `Option`, and inlines the synchronous root helper.
A predeclared eight-observation Richards/n-body diagnostic justified repeating
the complete matrix. That diagnostic is retained in `paired-v2/diagnostic-1`
and is not mixed into the accepted results. Total campaign volume is 232 timing
observations and 288 memory workers across the two implementations, not 232
observations of the accepted candidate.

### Correctness and remaining work

- Twenty-six new regression tests exercise safepoints, host re-entry, automatic
  GC, finite root budgets, suspension, disposal and Promise completion in both
  interpreter modes. Thirteen disposal/Promise fixtures independently fail on
  the immutable starting main and now pass; these are scenarios, not a claim of
  thirteen distinct bugs. The original discovery fixture fails there as well.
- The final runtime's local fast gate passes 1,944 tests, strict Clippy,
  formatting, no-std checks, architecture guards, examples and documentation.
- The saved candidate passes 7,203 focused Test262 variants in 3,674 files,
  99 QuickJS differential cases, 69 engine fixtures and 121 active cases,
  with no failures or skips.
- Integration still requires the exact-base complete correctness CI gate.
  PR #726 records its exact run and report artifact; local focused tests and
  performance verification do not replace it.

Large QuickJS gaps remain, especially Richards, n-body and hash-map. Fresh
binding/linear-plan profiles are the next candidates; this tranche does not
claim QuickJS parity, introduce JIT/unsafe code or change compiler defaults.
