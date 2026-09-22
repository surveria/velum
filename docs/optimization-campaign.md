# Performance and Memory Optimization Campaign

## Objective and completion criteria

Improve representative embedded JavaScript performance without weakening
ECMAScript behavior, independent-VM isolation, safe-Rust boundaries, or resource
accounting. A faster synthetic sentinel alone does not complete this program.

The approved program has four milestones:

1. Refresh a broad exact-source baseline, including prepared workloads,
   structurally different holdouts, direct embedding API calls, and JetStream.
2. Profile representative programs, implement common runtime improvements, and
   accept them only after paired latency measurements and correctness checks.
3. Measure process memory, logical retention, explicit collection, teardown,
   and multiple independent VMs in a separate reproducible lane.
4. Evaluate profile-guided compilation on a training cohort and an independent
   holdout cohort. Record gains, regressions, code size, and build cost; retaining
   the ordinary build is a valid outcome if PGO provides no reliable benefit.

The program is complete only after each milestone has recorded evidence, its
limitations have been reviewed, and accepted changes have passed the normal
pull-request correctness gate. Canonical July reports are historical evidence,
not proof of the current checkout's performance.

## One-command local entrypoint

```bash
./scripts/run-performance-campaign.sh
```

The launcher requires a clean committed checkout, exports that exact commit to
an independent source snapshot, rebuilds its release runner with the pinned
in-process QuickJS reference, and executes every selected lane
sequentially. It does not run Test262 or fuzzing, refresh tracked report history,
or change the five default per-merge sentinels. Ordinary CI does not invoke it.

Run a single lane when investigating a specific question:

```bash
./scripts/run-performance-campaign.sh --lane representative
./scripts/run-performance-campaign.sh --lane holdout
./scripts/run-performance-campaign.sh --lane embedding
./scripts/run-performance-campaign.sh --lane memory
./scripts/run-performance-campaign.sh --lane jetstream
```

The default artifact root is `$HOME/velum-fuzzing-artifacts/performance`, outside
every worktree. Override it with `--artifact-root /absolute/path` or
`VELUM_PERFORMANCE_ARTIFACT_ROOT`. The default Cargo cache is
`$HOME/velum-fuzzing-artifacts/build/performance-runner`; an absolute
`CARGO_TARGET_DIR` may override this cache root. Builds use a commit-specific
subdirectory and a build-and-copy lock. Each invocation creates a unique directory
containing the full tracked source snapshot, source commit/tree identity, compiler information, build diagnostics,
an immutable copy of the executable and its SHA-256, separate lane logs and
reports, local reference snapshots, and `lanes.tsv` completion statuses.
Keep ordinary project reports split by lane: the existing 1,000-line YAML bound
can reject a combined report containing all 22 selected workloads. The launcher
already writes separate reports and does not weaken that reporting bound.

A nonzero lane exit remains visible and makes the campaign exit unsuccessfully,
but does not discard reports from other lanes. A zero lane exit is not a claim
that every external candidate passed: inspect the report's measured, failed,
invalid, unavailable-reference, and skipped totals. Interrupted campaigns retain
their completed reports and logs; an absent final provenance timestamp means
the campaign has not completed. The original checkout can change or be removed
after export without changing a running campaign's sources or fixtures.

## Latency evidence

The ordinary Rust runner owns sampling and the exclusive host-performance lock.
Every prepared workload returns a deterministic primitive checksum, verified
across repetitions, after execution, and against QuickJS. The broader cohort
covers object transformation, polymorphic method dispatch, JSON ingestion,
string processing, Map/Set indexes, and allocation/tree traversal. Holdouts use
different program structures and data distributions, not just renamed inputs.

The launcher explicitly refreshes QuickJS references into the invocation's
external directory. It never overwrites the tracked sentinel or JetStream
baseline. JetStream uses a soft 900-second suite budget by default. Sampling
limits are checked after operations return, so they do not guarantee a hard
wall deadline. The launcher additionally gives each lane a 1,800-second process
watchdog (`VELUM_PERFORMANCE_LANE_TIMEOUT_SECONDS`), followed by a ten-second
termination grace period. Watchdog exits remain failed lane statuses, possibly
without a completed report; the deadline includes waiting for the host lock.
Missing support and invalid measurements remain visible rather than being
removed from the denominator.

Keep cold and steady-state measurements distinct. Velum exposes a separate
compile duration; the current QuickJS prepared adapter includes compilation in
setup. There is therefore no valid isolated cross-engine compile-time ratio.
Direct Rust embedding API benchmarks have no equivalent QuickJS API ratio.

Measure candidate changes against their exact parent on the same host and
cohort. Alternate parent/candidate order to expose drift; inspect variation
and useful-work checksums. Avoid concurrent compiler, fuzzing, or other heavy
host load while collecting final evidence. Use the existing minimum-duration
and variation gates without weakening thresholds to make new workloads pass.

For acceptance comparisons, keep the absolute compilation path and Cargo
metadata stable across variants and record CPU affinity. The snapshot launcher
provides isolation and replay, but independently built timestamped snapshots
are not automatically a controlled A/B experiment. A September 22 neutral
source control exposed roughly 28% variation in a direct Rust-callback case
between distinct-path builds despite unchanged runtime/runner sources and low
within-run CV. Rebuilding from one path and pinning one CPU removed the large
difference. Exact microarchitectural attribution remains unproven; do not call
every binary-level difference a gain or regression in the changed helper.

## Memory evidence boundaries

The separate memory lane uses new processes for each engine/scenario/repetition
and keeps multiple QuickJS VMs in independent runtimes. Phase observations
distinguish empty VM state, live workload, released roots, explicit GC, teardown,
and release of compiled state. Keep the following metrics separate:

- RSS and PSS describe process residency and proportional mapped memory.
- Process high-water RSS includes startup and is not a resettable per-phase peak.
- Velum storage records and payload bytes are logical ownership accounting, not
  allocator bytes, vector capacity, process RSS, or opaque host-capture size.
- QuickJS internal heap counters have a different scope and must not be divided
  by Velum logical counters to manufacture a memory-parity ratio.
- Explicit collection duration is not an automatic-GC pause distribution.
- Retained process RSS after teardown can reflect allocator retention rather
  than a remaining live VM object graph.

Unavailable OS data must have a reason, never a zero value. Exact allocation
counts and per-VM allocator-byte attribution are not available from the current
safe public APIs and remain follow-up work. Pending async callbacks, opaque host
payloads, shared compiled programs, and device-class comparisons also remain
separate extensions to the initial deterministic scenarios.

Defaults are three repetitions and a 120-second child deadline. The bounded
controls are `VELUM_MEMORY_REPETITIONS`, `VELUM_MEMORY_CHILD_TIMEOUT_MS`,
`VELUM_MEMORY_NODES`, `VELUM_MEMORY_BYTES_PER_NODE`, and
`VELUM_MEMORY_CHURN_ROUNDS`. `VELUM_MEMORY_FILTER` selects exact comma-separated
IDs: `hello-world`, `retained-graph`, `cyclic-churn`, `independent-vms-1`,
`independent-vms-10`, or `independent-vms-50`. The report records effective values.

## Reviewed baseline: 2026-09-22

The first tranche is baseline infrastructure, not a runtime optimization. On the
recorded Linux / Ryzen 9 9950X3D host, the completed measurements are:

| Lane | Completed evidence | Velum / QuickJS time, geometric mean |
| --- | --- | ---: |
| Existing sentinels | 5 valid, 0 failed/invalid; matching checksums | 0.14× |
| Representative mixed workloads | 6 valid, 0 failed/invalid; matching checksums | 10.26× |
| Structurally different holdouts | 6 valid, 0 failed/invalid; matching checksums | 8.44× |
| Direct Rust embedding | 5 valid, 0 failed/invalid | No equivalent reference API |
| JetStream shell candidates | 86 selected: 30 measured, 26 failed, 30 skipped; 24 paired measurements | 18.61× across those 24 pairs |
| Isolated memory | 36 workers passed; 0 failed/skipped; 3 repetitions per engine/scenario | Not a latency or allocator-parity ratio |

Ratios above one mean Velum took longer. These are separate workload cohorts,
not one aggregate engine score or an official JetStream score. The five fast
sentinels do not describe the broader mixed workloads. Unsupported JetStream
harness requirements and resource-limit failures remain visible. The 50-VM
memory case has roughly 28 MiB live process RSS for Velum versus 14 MiB for
QuickJS; this includes the worker/runtime and is not a per-VM allocation claim.

Project and memory measurements use source commit `3e09be2e45403352fd8810d28d4df77e44610c2a`
and tree `3ab735572f51d9157950e590a3605edcacd22661`. The JetStream baseline uses
`ae20f7cc46722b5833c461ee70e05e29ba9a709e` / tree
`0812a3c7661e09f10d2abbbb0518ae44bfd400ed`; engine source is unchanged between
these snapshots. The reference is the pinned `rquickjs` 0.9 bundled QuickJS.

Complete local evidence is outside the worktree under
`$HOME/velum-fuzzing-artifacts/performance/campaign-20260922`:

- `baseline/jetstream.yaml` and its bounded timing/component siblings;
- `campaign-20260922T174028Z-3805068/reviewed-{sentinel,representative,holdout,embedding}.yaml`;
- `campaign-20260922T174028Z-3805068/reviewed-memory.{md,json,yaml}`;
- preserved source snapshots, binaries, source/configuration metadata and logs.

Earlier calibration attempts are retained but excluded from the reviewed
totals: the short holdout dispatch workload was doubled to clear the existing
1 ms minimum, and an oversized combined report was rerun as separate lanes.
No quality threshold was relaxed. Local root checks passed 1,818 tests;
runner checks and focused follow-ups cover 150 tests, including 17 memory tests.
These local results do not replace the required exact-head correctness CI gate.

Next: collect profiles of method dispatch, object transformation and tree/GC
work; accept common runtime improvements through paired parent/candidate runs;
then evaluate PGO on separate training and holdout inputs. No runtime speedup
or PGO benefit is claimed by this infrastructure change.

## Reviewed runtime tranche: 2026-09-22

PR #722 removes temporary formatting allocation from canonical array-index
validation and permits guarded own-data slot reuse across ordinary receivers
with the same immutable shape. Values come from the current receiver; accessors,
exotic objects, global bindings, inherited/missing entries and mutation paths
retain their semantic guards. No cache storage, roots or resource exemptions
were added. Fifteen regression cases cover both optimization modes, including
GC identity reuse, descriptor changes, realm globals and independent VMs.

The exact measured parent is `b080c5603570b631a4b5a1f848ee272d2916c895`, and the
candidate is `ecda04b85031f8838220c662b9c2a4e7f8c5012b`. Later PR commits update
documentation only. Both release runners were built from one absolute source
path with the same compiler, features and Cargo metadata, then preserved
separately. Measurements ran sequentially on CPU 0 of the baseline host,
without concurrent compilation or fuzzing, in parent/candidate then
candidate/parent order. These ratios compare absolute Velum timings with its
parent, not ratios to independently sampled QuickJS timings.

| Cohort | Cases | Candidate / parent time | Observed change |
| --- | ---: | ---: | ---: |
| Existing sentinels | 5 | 1.0034× | +0.3% |
| Representative mixed workloads | 6 | 0.9598× | -4.0% |
| Independent holdouts | 6 | 0.9669× | -3.3% |
| Direct Rust embedding | 5 | 1.0095× | +0.9% |
| Focused JetStream: Richards, hash-map, base64 | 3 | 0.9733× | -2.7% |

This is 100 valid timing rows across 25 cases, two builds and two rounds.
Object transformation improved about 9%, method dispatch about 4-5%, and the
representative JSON case about 6.6%. Not every case improved: both collection
cases took about 2% longer, the Rust callback about 1.9%, and Richards about
0.5%. The bounded change is accepted for repeatable general object/dispatch
gains with these measured trade-offs; it is not a universal speedup or a
statistical significance claim.

Evidence selection preserves every earlier observation. Distinct-path builds
are diagnostic only, as described above. Both JSON cases use separate paired
15-second minimum sampling with three samples. One ordinary representative
lane failed because QuickJS collection CV was 13%, exceeding the unchanged
10% gate; all five non-JSON representative cases were repeated across all
four builds/rounds with five-second minimum sampling and five samples.
Cross-round variation in tree allocation then prompted four independent
15-second/three-sample measurements: 63.27/63.55 ms parent versus
61.62/61.89 ms candidate, a consistent 2.6% reduction. No fastest-run selection
or relaxed quality threshold is used. Exact IDs, source identities, complete
status matrices and typed checksums bind the joined reports; these are not
one simultaneous full-suite run.

The 17 prepared JavaScript cases retain identical checksums across both builds
and rounds. Embedding and JetStream reports do not export comparable checksums,
so their successful runner checks are not an additional cross-build output
equivalence proof. All 144 process-isolated memory workers passed. Logical
record/payload counts match across all corresponding phases, while process RSS
remains separately reported; no allocator-parity or automatic-GC pause claim
is made. The full local gate passed 1,833 test results, including strict lint,
no-std, documentation and architecture checks. Required exact-head correctness
CI and its artifact are linked from PR #722 before merge.

External evidence is under
`$HOME/velum-fuzzing-artifacts/performance/property-fastpaths-20260922`:
`same-path/paired-ab-ba/comparison.{md,json}` indexes accepted reports,
explicit replacements, discarded measurements and memory phases. Source
snapshots, executable digests, scripts, raw logs and neutral-build controls
remain beside it. PGO is the next separate experiment; ordinary build defaults
are unchanged. Its frozen protocol and review boundaries are described in
[Controlled PGO Experiment](pgo-experiment.md).
