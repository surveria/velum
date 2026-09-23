# Binding Read Performance

This tranche starts at PR #726, commit
`3e34817ef0e8083a4b3ef2684c7d11055e07343a`. QuickJS performance parity remains
an open objective. This work does not introduce JIT, unsafe code, different
default compiler settings, dependencies, or package versions.

## Mechanism and semantic boundaries

Fresh frame-pointer release profiles on the starting main identify binding
reads as about 7.0% of n-body self cycles. Linear-plan binding accounts for
about 4.8%; its structure and execution remain unchanged in this candidate.
Profiles run sequentially on CPU 0 without QuickJS under the shared host lock.
They select candidates, but are not acceptance timing evidence.

The candidate makes initialized binding reads available for inlining while
keeping indirect live-import resolution and error construction out of line.
It retains the same checked RefCell borrow, owned Value clone, alias target
clone, borrow-release boundary, and TDZ/deleted/invalid-alias errors. Root
visitors still receive a cloned value after all binding borrows have ended.
There is no new binding cache, skipped validation, GC rule, or storage charge.
Both interpreter modes use this common semantic read operation.

Boundary tests also found a pre-existing eval redeclaration defect, independently
reproduced on the immutable starting main. Deleting an eval-created variable
kept its inactive environment index entry, but redeclaration tried to install a
new scope cell through the active-cell identity guard. The fix replaces only
inactive entries, restores their deletability/visibility, and reuses their
existing storage charge. Active entries retain the original identity guard.
This follows the creation of a new deletable variable binding in
[EvalDeclarationInstantiation](https://tc39.es/ecma262/multipage/global-object.html#sec-evaldeclarationinstantiation).

### Deferred catch-environment issue

Two additional fixtures expose a pre-existing catch-shadowing problem: a
closure created before eval does not see the function variable introduced
under an intervening simple catch parameter. An experimental registration
change fixed those fixtures but broke a closure created inside that eval.
That broader change was withdrawn; the original catch/hoisting/capture logic
is unchanged, and an added guard test preserves the inside-eval closure.
The two failing fixtures are retained in the external
`deferred-catch-boundaries.rs` and `parent-redeclaration.log`. They need a
separate ordered-environment design; they are not reported as fixed or passed.

## Frozen experiment protocol

Artifacts are retained outside worktrees under
`$HOME/velum-fuzzing-artifacts/performance/binding-paths-20260923`.
Build immutable parent and candidate ordinary release runners at identical
absolute source and Cargo target paths with clean builds and identical flags.
Archive sources, executable hashes, compiler identity, and every result.

First run an eight-observation Richards/n-body diagnostic: parent/candidate,
then candidate/parent for each case. Advance to the complete matrix only if
neither case is more than 1% slower and their combined paired geometric mean
improves. Keep the diagnostic separate from acceptance evidence. A rejected
candidate or invalid run remains visible; any follow-up gets a new identity.

The complete matrix reuses all 28 timing controls from PR #726: five
sentinels, six representative programs, six separate holdouts, five embedding
API cases, and six selected JetStream programs. Run per-case parent/candidate
then candidate/parent sequentially on CPU 0 under the shared host lock, with
no competing compilation, correctness CI, or fuzzing. Keep five seconds/five
samples for prepared cases, 15 seconds/three samples for JSON and tree cases,
and five seconds/three samples for JetStream. Retain the 1 ms minimum operation
and 10% CV gates; any invalid retry repeats its full paired cohort.

Require exact typed useful-work checksums where exported and validate complete
report/source identities and configurations. Embedding and JetStream do not
export these checksums; their runner checks are not extra equivalence evidence.
Run four complete 36-worker memory campaigns and compare phase-based logical
VM counters separately from process RSS/PSS and QuickJS allocator statistics.
Keep all individual regressions visible. Broad acceptance requires no cohort
geometric-mean regression above 1% and no individual paired-mean regression
above 3%; interpret smaller differences conservatively, not as significance.

Add targeted tests for TDZ, deleted eval bindings, live imports and cycles,
GC/re-entry, closures, isolated VMs, and both interpreter modes. Require the
strict local fast gate, relevant focused Test262, and exact-base complete CI
before integration. Publish reviewed results in this document and README.

## Reviewed results (2026-09-23)

The accepted matrix is `paired/acceptance-1` under the external artifact root.
All 116 command statuses and report identities/configurations passed validation:
112 timing observations over 28 cases, plus four complete memory campaigns.
The report validator passes 74 acceptance/rejection fixtures. No failed run,
fastest rerun, or replacement sample was selected for the accepted matrix.

Parent: `3e34817ef0e8083a4b3ef2684c7d11055e07343a`, tree
`0f283034d837822fda2a35565e0287239b069bba`. Measured candidate:
`5124b3892f9a59cab3f58878ad3591a0b81bf40e`, tree
`cfe0897996fd0a554671a1aea4683e0bc39dbe7f`. Later PR changes only document the
reviewed evidence. Saved runner SHA-256 hashes:

- Parent: `22be60b33f06ea5e47a5ed57540084db61f86f0977442583685d352c890e6084`.
- Candidate: `dfea7b1c1e6f01f25ec35fe6a03eb9a4d4da1f636688e151eb359e53c9bcffe6`.

Both builds use the ordinary release profile, Rust 1.96.0 / LLVM 22.1.2 and
identical absolute source/target paths. Measurements are sequential on CPU 0
of an AMD Ryzen 9 9950X3D under the shared host lock. The archived candidate's
frozen protocol SHA-256 is
`dc22ca095c8dc65d6e648538d0b08a56e23f1d32a2191efcac0967bd31bc674b`.

Changes are geometric means of paired median time ratios. Negative means
less execution time. These are host/cohort observations, not confidence
intervals, a universal speedup claim, or an official JetStream score.

| Cohort | Cases | Time change | First pair | Reverse pair |
| --- | ---: | ---: | ---: | ---: |
| Sentinels | 5 | +0.6% | +0.3% | +0.9% |
| Representative programs | 6 | -1.6% | -1.4% | -1.8% |
| Separate holdouts | 6 | -2.0% | -1.9% | -2.1% |
| Direct embedding API | 5 | -1.1% | -1.1% | -1.2% |
| Selected JetStream programs | 6 | -0.9% | -0.7% | -1.0% |

The sentinel cohort regresses; this is not hidden by the improving groups.
The largest individual regression is array indexing at +2.4%, in both paired
orders. The function-call sentinel is +0.3%, while property-read and string-scan
sentinels are unchanged. The four improving cohort means improve in both
orders. Method dispatch improves by 3.5% in the representative set and 3.6%
in the separate holdout; embedding Rust callbacks improve by 3.3%.
The complete matrix passes the predeclared 1% cohort / 3% individual regression
ceilings. Smaller mixed-direction differences should be treated conservatively.

| JetStream case | Parent seconds | Candidate seconds | Time change | Parent / QuickJS | Candidate / QuickJS |
| --- | ---: | ---: | ---: | ---: | ---: |
| Richards | 5.174 | 5.090 | -1.6% | 36.45x | 35.51x |
| hash-map | 6.597 | 6.526 | -1.1% | 27.99x | 27.89x |
| SunSpider Base64 | 2.626 | 2.621 | -0.2% | 23.23x | 23.34x |
| SunSpider n-body | 2.951 | 2.911 | -1.4% | 33.19x | 32.75x |
| js-tokens | 1.249 | 1.242 | -0.6% | 9.77x | 9.82x |
| SunSpider tagcloud | 1.139 | 1.134 | -0.4% | 10.04x | 10.13x |

QuickJS timings vary independently. Each ratio uses that build's own paired
reference observations, so a slightly lower Velum time does not always imply
a better Velum/QuickJS ratio. The large reference gaps remain open.

Prepared-program useful-work checksums match exactly across all four
observations, including typed u64 number bits. Embedding and JetStream rows
do not export those checksums. All 144 memory workers pass with identical
corresponding VM logical record/payload counters in every phase, and zero
logical records/payload after owner teardown. Selected paired-median RSS
deltas range from -0.078 to +0.156 MiB; process residency is separate from VM
logical accounting and QuickJS allocator statistics.

The earlier eight-observation Richards/n-body diagnostic is retained in
`paired/diagnostic-1`, separately from acceptance. Total campaign volume is
120 timing observations and 144 memory workers. The interrupted build and
withdrawn catch-environment experiment contributed no accepted timing samples.

### Correctness and remaining work

- Twenty new integration tests pass in both interpreter modes. Seven eval
  recreation scenarios independently fail on the immutable starting main and
  now pass; these are scenarios of one defect, not seven distinct fixes.
- The final runtime's local fast gate passes 1,964 tests in 320 suites with
  zero failures or skips, strict Clippy including no-std, formatting,
  architecture guards, examples and documentation.
- The saved candidate passes 4,184 focused Test262 variants in 2,908 files,
  99 QuickJS differential cases, 69 engine fixtures and 121 active cases,
  with zero failures or skips.
- Complete exact-tree correctness CI is the remaining integration gate. Its
  artifact link will be recorded in the PR description before merge.
- The two catch-environment repros above are deliberately unresolved. Future
  performance work should refresh profiles before changing linear-plan
  preparation or long-string hashing; neither is changed by this tranche.
