# Safe Runtime Performance: Second Tranche

This opt-in follow-up starts from PR #723 (`bfc190ca`). It covers PGO/build
tuning, allocation and GC overhead, object/property/call paths, existing
bytecode plans, and built-ins. JIT, unsafe code, and a separate embedding/startup
optimization program are out of scope. Existing embedding measurements remain
regression controls. Ordinary release settings and routine CI stay unchanged.

## Baseline and candidate selection

Refresh the complete shell-adapted JetStream cohort against the pinned in-process
QuickJS reference. Keep failed, skipped, invalid and unavailable-reference rows
visible. Only successful, quality-qualified paired rows support speed ratios;
this is not an official JetStream score. Save sources, binaries, hashes, logs
and reports outside worktrees under
`$HOME/velum-fuzzing-artifacts/performance/next-campaign-20260922`.

The first run uses commit `50405bae81597603c04a33caf20252e92ff8e6e7`, whose
engine tree matches merged PR #723. Old profiles predate the preceding property
optimizations: refresh relevant diagnostic profiles before treating their
percentages as current evidence. Profiles identify candidates, not speedups.

Initial source-based candidates, subject to measurement and review:

- Reuse an admitted RegExp input through matching, legacy state and result
  creation instead of repeatedly copying and interning the full string.
- Represent a full-block, single-operation linear plan directly instead of
  allocating several containers. Preserve per-activation and per-iteration
  lexical binding; do not cache mutable binding cells in compiled programs.
- Avoid copying the entire ephemeron list at each WeakMap reachability pass.
  Preserve fixed-point iteration boundaries and exact reclamation/accounting.
- Inspect fresh object/property/call profiles before selecting another guard
  or cache change; the previous same-shape own-data slot reuse is already merged.
- Productize the reviewed PGO experiment as a separate opt-in entrypoint.
  Evaluate ThinLTO with one codegen unit as an explicit alternative preset;
  keep profiles and conclusions separate from the ordinary-release experiment.

## Measurement and acceptance protocol

Run measured workloads sequentially under the shared host-performance lock,
without competing compilers or fuzzers. Build parent and candidate from the same
absolute source and Cargo target paths, preserving flags and metadata inputs;
copy each finished binary to immutable external storage before replacing build
inputs. Pin paired measurements to CPU 0 and alternate parent/candidate then
candidate/parent. Do not compare differently located builds as a causal test.

Use the existing five sentinels, six representative workloads and six distinct
holdouts; retain direct embedding regression controls. Add focused measurements
only for a concrete mechanism not exercised by those cohorts. Keep useful-work
checksums, status, variation and compiler/source identity in every comparison.
Use the existing 1 ms minimum operation and 10% maximum CV for acceptance.
Freeze longer JSON/tree sampling for both variants before collecting results:
15 seconds minimum with three samples and a 60-second budget; other prepared
cases use five seconds, five samples and a 30-second budget. A failed quality
gate is inconclusive, not a zero or a selected best sample. Any retry repeats
the complete paired case cohort and retains the excluded attempt with a reason.

Targeted JetStream comparisons use the same protocol and resource limits on
both variants. Baseline ranking diagnostics and controlled paired acceptance
remain distinct. Unsupported workloads do not become performance wins.

For PGO, train only the six representative cases, merge fresh raw profiles with
matching LLVM tools, then freeze the profile before measuring holdouts. Use a
separate fresh training/profile directory for each compiler preset. Preserve
the existing source, profile, checksum, diagnostics and process-cleanup checks;
validate the saved profile-use executable, not an ordinary rebuilt substitute.
See [the initial PGO experiment](pgo-experiment.md) for the reviewed precedent.

Correctness requires optimizer-enabled/disabled regressions, relevant focused
Test262 slices, the local fast gate and exact-head full correctness CI. GC and
string changes must also preserve resource-limit failures, ownership across
independent VMs, and storage-ledger checks. Report measured gains, regressions,
inconclusive results and deferred candidates separately; do not promise a
general speedup from one synthetic case. Update the README quality summary
only after the tranche reaches a reviewed milestone.

## Current status

Implementation and measurement are in progress. No gain from this tranche has
yet been accepted, and no PGO or compiler preset is enabled by default.
