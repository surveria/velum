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

## Current status

Implementation is experimental. No performance improvement has been accepted.
