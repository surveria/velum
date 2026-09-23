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

## Reviewed results: 2026-09-22

The complete AB/BA campaign passed validation without replacement runs: all
116 commands succeeded, all 112 timing observations passed the 10% CV gate,
and all 144 memory workers passed. The report validator also passes 74 positive
and negative fixtures. Artifacts, raw reports and the comparison are retained
under the external campaign root above, in `paired/reviewed-ab-ba/`.

The frozen runtime candidate is `2b88f319d59f384fafce8cecd285c46e94958b58`,
tree `360c65da8eef8a0533d6af5a01cbb4d5afb771d2`. Later regression-test and
documentation commits do not change the measured runtime. The parent tree is
`c75520ce4c52f6a06c01fa210ff0e68e1962c509`. Saved runner SHA-256 hashes are:

- Parent: `a94fd76c6ce462a65161f59bd130a6fd45cf18a1e86a1e889aab3b005d1264d0`.
- Candidate: `5104c19b3597fe5c6b97a82e1404bf6141b6b5ae980a7ca986cab0b56838034d`.

The machine is an AMD Ryzen 9 9950X3D, with CPU 0 affinity, Rust 1.96.0 and
LLVM 22.1.2. Builds use the ordinary release profile, without PGO or optional
ThinLTO. The frozen protocol is archived with the candidate source; its SHA-256
is `faf43db6ab603b1988abac87e97019442e272eff81aa8e19c8e412c588512175`.

Times below are geometric means of the two paired median ratios. Negative
changes mean less execution time; positive changes mean slower execution.

| Cohort | Cases | Candidate time change | First pair | Reverse pair |
| --- | ---: | ---: | ---: | ---: |
| Sentinels | 5 | +0.5% | +0.9% | +0.1% |
| Representative programs | 6 | +0.9% | +1.2% | +0.6% |
| Separate holdouts | 6 | +1.3% | +1.0% | +1.6% |
| Direct embedding API | 5 | -4.6% | -4.0% | -5.2% |
| Selected JetStream programs | 6 | -7.9% | -7.8% | -8.0% |

| JetStream case | Parent seconds | Candidate seconds | Time change | Parent / QuickJS | Candidate / QuickJS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Richards | 5.161 | 5.180 | +0.4% | 36.34x | 36.18x |
| hash-map | 6.527 | 6.760 | +3.6% | 27.88x | 28.84x |
| SunSpider Base64 | 4.027 | 2.723 | -32.4% | 35.96x | 23.99x |
| SunSpider n-body | 2.995 | 2.977 | -0.6% | 33.32x | 33.59x |
| js-tokens | 1.239 | 1.278 | +3.2% | 9.74x | 10.21x |
| SunSpider tagcloud | 1.444 | 1.222 | -15.3% | 12.76x | 10.85x |

QuickJS ratios use each build's paired reference measurements; reference timing
changes explain why a ratio can improve while absolute Velum time worsens.
These six cases do not replace the preceding complete JetStream coverage
inventory or constitute an official suite score.

This is an explicit bounded tradeoff, not a general interpreter speedup.
Both pairs confirm the Base64 and tagcloud improvements, but also show costs:
holdout method dispatch is +4.4%, holdout collection indexing +3.7%, array
indexing +2.0%, and representative object transformation +2.5%. The two pairs
and per-row CV gates are not a statistical confidence interval. Keep these
regressions visible when evaluating the next dispatch/binding/rooting tranche;
do not count this result as a broad-control improvement.

Prepared-program useful-work checksums match exactly across all four
observations, including typed 64-bit number representations. All corresponding
Velum VM logical record/payload counters match in every memory phase, and all
owner-drop phases reach zero logical records and payload bytes. Selected
live/after-GC/owner-drop RSS paired median deltas range from -0.041 to +0.217
MiB. RSS includes allocator and process overhead and is not the logical heap;
retained RSS alone is not a leak proof. Raw RSS/PSS phases remain in the reports.

Correctness evidence for the change:

- Eleven new regression tests exercise ASCII boundaries, all 65,536 single
  UTF-16 code units, malformed sequences, deduplication, independent owners,
  resource-limit failures, GC slot reuse and concatenation. Engine-facing cases
  exercise both optimizer modes; 52 focused tests pass in total.
- The local fast gate passes 1,918 tests, formatting, strict clippy, no-std
  checks, architecture guards and documentation checks.
- The saved candidate passes 3,218 focused Test262 variants in 1,620 files for
  strings, JSON and addition; 99 QuickJS differential, 69 engine fixtures and
  121 active-subset cases also pass, with no failures or skips.
- The final source tree still requires the separate complete correctness CI
  gate before integration. PR #725 records the exact run and artifact; targeted
  timing validation is not a substitute for that gate.

Large QuickJS gaps remain, especially Richards, n-body and hash-map. Their
distributed instruction dispatch, binding access and root-management profiles
are the next investigation targets. No JIT, unsafe code, release-default
change, or claim of QuickJS parity is part of this tranche.
