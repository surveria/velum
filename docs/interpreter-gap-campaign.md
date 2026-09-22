# Remaining Interpreter Gaps: String Admission

This tranche starts from PR #724, commit
`584b714d501c08702bac5e14b5063ca064da4d8e`. It targets measured interpreter costs
without JIT, new unsafe code, PGO, or release-profile changes. The earlier
27-36x selected QuickJS gaps are motivation, not a promise to eliminate them
with this change.

## Fresh diagnostic profiles

Four current-main CPU profiles cover Richards, SunSpider Base64, SunSpider
n-body, and hash-map. The frame-pointer release build has debug information
and no QuickJS feature; workloads run sequentially on CPU 0 under the shared
benchmark host lock. These profiles select work, not before/after speedups.

Base64 has approximately 29% self cycles in string payload preparation, 22%
in long-byte hashing, and 7% in memory copying. Inspection confirms repeated
lookup/hashing during string admission and an intermediate UTF-16 allocation.
Richards, n-body and hash-map have distributed dispatch, binding, rooting and
property costs; those remain separate follow-up candidates.

## Selected changes

- Use one borrowed interning entry across lookup, reservation and insertion.
  Existing entries do not reserve new cache records; failed admission does not
  commit a reservation. Record storage remains separate from the lookup index
  so the safe entry borrow can span insertion without runtime dependencies in
  the storage module.
- Copy a borrowed UTF-16 input directly into its shared payload, avoiding the
  temporary vector. Heap-owned and portable strings retain their shared payload.
- Detect all-ASCII UTF-16 with a reducible code-unit scan. Keep the full decoder
  for non-ASCII text, including malformed surrogate sequences; preserve exact
  UTF-16, replacement rendering, owner checks and logical payload accounting.

## Frozen acceptance protocol

Archive all sources, binaries, hashes and reports outside worktrees under
`$HOME/velum-fuzzing-artifacts/performance/interpreter-gaps-20260922`.
Build ordinary parent and candidate release runners at the same absolute
source and Cargo target paths, with clean builds and identical flags. Do not
use profiled binaries for acceptance timing.

Reuse the preceding tranche's complete paired cohort: five sentinels, six
representative programs, six distinct holdouts, five existing embedding
controls, six selected JetStream programs, and four 36-worker memory campaigns.
Each timing case runs parent/candidate followed by candidate/parent on CPU 0,
sequentially on a quiet host. No competing compilers or fuzzers during timing.
Use five seconds/five samples for ordinary prepared cases; JSON/tree cases use
15 seconds/three samples. JetStream uses five seconds/three samples. Keep the
1 ms minimum operation and 10% CV gates, exact typed useful-work checksums,
source identity, failures, and exclusions visible. A retry repeats the complete
affected case cohort and preserves the failed attempt; never select best times.

Compare absolute Velum times, then report QuickJS ratios separately. This is a
shell-adapted workload comparison, not an official JetStream score. Embedding
and JetStream reports do not provide the prepared corpus's cross-build output
checksum evidence. Full correctness remains a separate gate.

Acceptance requires targeted UTF-16, deduplication, GC, VM ownership and
transactional resource-limit tests, the local fast gate, relevant focused
Test262 coverage, and exact-tree full correctness CI. Update the README quality
table only after the bounded results are reviewed. Broad interpreter gaps
remain an active objective beyond this tranche.
