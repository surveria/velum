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

## Reviewed runtime results: 2026-09-22

The frozen runtime comparison is parent `50405bae81597603c04a33caf20252e92ff8e6e7`
(tree `ff2a2fed92e6c819a15af4c8448db187a6a67cd6`) against
`b91845ef6d571810f587616c31577e2eacaa56dc`
(tree `e3f066f5d6047538b7e3d055e593fe8e8b1e59a6`). Later launcher, validator and
documentation changes do not enter this runtime comparison. Both release
runners use the same absolute source/output paths, Rust 1.96.0 / LLVM 22.1.2,
and CPU 0 on the Ryzen 9 9950X3D host. The archived candidate retains the original
pre-measurement protocol document; its SHA256 is recorded in the comparison.

All 116 commands passed: 28 workloads with four observations each and four
36-worker memory campaigns. Every engine and available QuickJS timing meets
the frozen CV gate; no failed cohort, replacement sample or fastest rerun was
selected. Exact typed prepared-workload checksums agree across all observations.
Embedding and JetStream reports do not export equivalent checksums, so successful
runner verification is not an additional cross-build output-equivalence proof.

| Cohort | Cases | Candidate / parent time | Round 1 | Round 2 |
| --- | ---: | ---: | ---: | ---: |
| Sentinels | 5 | 0.9968 | 0.9923 | 1.0014 |
| Representative mixed workloads | 6 | 0.9757 | 0.9760 | 0.9755 |
| Distinct holdouts | 6 | 0.9914 | 0.9900 | 0.9928 |
| Existing embedding controls | 5 | 0.9510 | 0.9504 | 0.9516 |
| Selected JetStream workloads | 6 | 0.7678 | 0.7703 | 0.7653 |

These are geometric means of paired absolute Velum times. The runtime holdouts
are not entirely blind: `holdout_method_dispatch` was among the diagnostic CPU
profiles. They are separate programs, not an untouched statistical test set.
The separate PGO experiment trains only on the representative cohort.

The largest gains are Base64 (-38.4% time), js-tokens (-27.1%) and tagcloud
(-51.6%). Representative method dispatch improves 5.4%; the existing Rust
callback control improves 21.6%. Conversely, the arithmetic sentinel is 2.6%
slower in aggregate (2.1% and 3.1% in the two pairs); synchronous embedding calls
and host-object payload controls are about 0.3% slower. These regressions remain
visible. The combined tranche is accepted for its bounded workload gains, not
as a claim that every optimization or arbitrary program becomes faster. Two
rounds and a CV threshold do not establish statistical significance.

All 144 memory workers pass. Every corresponding Velum VM logical record and
payload counter matches at every measured phase, including churn and teardown.
Selected phase median paired RSS changes range from -0.055 to +0.129 MiB; this
is process residency, not allocator-byte equality or proof of a memory saving.
The PGO validator additionally compares category-level logical counters.

### JetStream and remaining QuickJS gaps

The fresh complete baseline selected 86 shell-adapted workloads: 30 measured,
26 failed and 30 skipped, with 24 valid QuickJS pairs and a geometric mean
Velum/QuickJS time ratio of 18.0568. Failed, unsupported and unavailable-reference
workloads remain visible and never become speedups. This is not an official
JetStream score. That full baseline preceded this tranche; only the six selected
workloads received the controlled before/after comparison below.

| Workload | Candidate / parent time | Candidate / QuickJS time |
| --- | ---: | ---: |
| Richards | 0.9875 | 36.18x |
| hash-map | 0.9794 | 27.34x |
| SunSpider Base64 | 0.6164 | 35.46x |
| SunSpider n-body | 0.9747 | 32.54x |
| js-tokens | 0.7287 | 9.48x |
| SunSpider tagcloud | 0.4838 | 12.63x |

Fresh profiles identified eager UTF-16-to-UTF-8 conversion during string-ID
validation, repeated RegExp subject admission, bytecode-plan construction, and
property/binding dispatch. The changes target those paths. Large call/object
and numeric-workload gaps remain; the JavaScript `hash-map` workload must not be
misclassified as a direct measurement of the built-in `Map` implementation.

### Correctness and artifacts

The tranche adds 49 engine regression cases across the six runtime test files.
Collecting-callback tests exposed one new RegExp optimization lifetime defect
and two pre-existing input-lifetime gaps; all are fixed. Module namespace
coverage also exposed pre-existing missing persisted-scope storage ownership,
reproduced before the write optimization and repaired with six regression cases.
The local engine gate passes, and the saved runtime candidate passes all 6,416
focused Test262 variants from 3,591 files in the affected areas. Full ready-PR
correctness remains the final integration gate.

Complete raw reports, immutable binaries/sources and validator diagnostics live
under the external campaign root. Key paths are
`baseline/campaign-20260922T204537Z-399232/`, `profiles/`,
`paired/reviewed-ab-ba/comparison.{json,md}`, and `focused-test262.yaml`.
The comparison validator passes 74 positive/negative fixtures. Build-time logs
from the initial runtime binaries overlap correctness compilation and are
diagnostic only; do not use them for a controlled compiler build-cost ratio.

## Focused ephemeron GC diagnostic

The standalone public-API probe keeps 8,192 WeakMap edges alive in chains of
depth 1, 8 and 32. Setup and JavaScript execution are outside the GC timer;
repeated steady-state collections must reclaim nothing and preserve exact
checksums, entries and logical storage counts. Each of the twelve AB/BA runs
has 31 calibrated samples targeting at least 250 ms of accumulated GC time per
sample and at least five seconds per run. The per-collection figures are
derived from these batches; this is not a new microsecond-scale active corpus
benchmark or the prepared runner's 1 ms operation gate.

| Chain depth | Parent / candidate speedup | AB pair | BA pair |
| --- | ---: | ---: | ---: |
| 1 | 1.079x | 1.085x | 1.073x |
| 8 | 1.756x | 1.763x | 1.748x |
| 32 | 2.692x | 2.649x | 2.734x |

All 372 samples and twelve processes pass the independent raw-statistics,
identity, checksum and quality checks. Total measured GC time is 136.21 seconds;
the largest within-run CV is 1.61%. This supports the focused fixed-point GC
change, not a general GC or application speedup. The compared binaries contain
the entire runtime tranche, not an isolated single-commit ablation. Artifacts:
`gc-pairs/reviewed-ab-ba/`, including raw TSV, captured lock ownership, immutable
probe inputs and `summary.json`.

## Separate ThinLTO configuration experiment

The same runtime candidate and absolute source/target paths were used for a
clean build with only `profile.release.lto="thin"` and
`profile.release.codegen-units=1` changed. This measures their combined effect,
not either flag independently; no PGO profile is involved. All 24 AB/BA holdout
observations passed with identical typed checksums and a maximum CV of 7.5%.

| Holdout | ThinLTO / ordinary time | Round 1 | Round 2 |
| --- | ---: | ---: | ---: |
| Object transformation | 0.9209 | 0.9192 | 0.9226 |
| Method dispatch | 0.9514 | 0.9566 | 0.9462 |
| JSON ingestion | 0.9133 | 0.9133 | 0.9133 |
| String processing | 0.9112 | 0.9122 | 0.9102 |
| Collection indexing | 0.9467 | 0.9616 | 0.9321 |
| Tree allocation | 0.9271 | 0.9219 | 0.9324 |

The six-case geometric mean is 0.9283 (rounds 0.9306 and 0.9261): 7.2% less
execution time on this host/cohort. Whole-runner file size decreases from
16,963,584 to 14,889,224 bytes (-12.2%); ELF `.text` decreases from 10,535,463
to 10,013,367 bytes (-5.0%). These are not standalone engine-library sizes.
The separately scheduled clean build takes 78.88 wall seconds and 167.29 CPU
seconds. No controlled build-cost ratio is available because the ordinary
build overlapped correctness compilation. Sources, commands, binary hashes and
raw reports are under `thinlto/build/` and `thinlto/runs/reviewed-ab-ba/`.

Saved-binary correctness and the separate productized release PGO experiment
remain pending. No PGO or compiler preset is enabled by default. Gains from
different experiments must not be added or multiplied into an unmeasured
combined-speedup claim.
