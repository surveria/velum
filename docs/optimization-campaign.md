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

## Progress

The first implementation tranche adds the opt-in workloads, launcher and memory
lane. Fresh runs are being collected outside the repository; no completed
runtime optimization or PGO benefit is claimed by this infrastructure change.
Record reviewed milestone totals in the README quality-evidence block, without
committing exhaustive local reports.
