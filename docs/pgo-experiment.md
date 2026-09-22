# Controlled PGO Experiment

This is the fourth milestone of the [optimization campaign](optimization-campaign.md),
after the representative baseline and guarded property-read changes in PRs #721
and #722. Ordinary release settings remain unchanged. The initial experiment
is local and opt-in; it does not add heavy work to routine CI.

## Frozen protocol

Use one clean source commit, one absolute exported source path, and one
experiment-owned Cargo target/build path for three release runners: ordinary,
instrumented, and profile-use. Keep the compiler, target, features, lockfiles and
all non-PGO flags identical. Clean only this experiment's Cargo output before
each variant. This measures cold Cargo-output build cost, not a cold OS or
dependency-download cache. Record compiler versions, source and binary hashes,
ELF sections, separate logs, process exit status, wall time, CPU time and maximum
resident memory. Whole-runner size is not standalone engine-library size.

Train only the six `representative_*` prepared workloads. Each exact-ID process
must emit a new nonempty raw profile. Merge with the matching Rust-bundled LLVM
tool, reject corrupt inputs and profile mismatch diagnostics, and establish
that recognized Velum-related functions executed. The conservative symbol guard
also protects trait implementations whose top-level self type or trait belongs
to Velum; this is not an exhaustive defining-crate demangler. Freeze the merged
profile before evaluating any `holdout_*` workload or memory scenario. Never incorporate
holdout measurements or correctness runs into training.
Training weights follow the observed execution counts, not equal weights for
the six workload families. Rust dependencies are instrumented too; precompiled
standard-library code and the bundled C QuickJS reference are not.

Evaluate six structurally different holdouts in ordinary/PGO order, then
PGO/ordinary order, sequentially on CPU 0. JSON and tree-allocation cases use
15,000 ms minimum sampling, three samples and a 60,000 ms total budget. Other
cases use 5,000 ms, five samples and a 30,000 ms budget. All retain a 150 ms
warmup, the existing 1 ms minimum operation, 10% maximum CV and three attempts.
The maximum operation is 2,000 ms. These sampling settings apply to both
training and evaluation; the 144 memory workers cover ordinary/PGO only.
These settings are fixed before training. A quality failure remains
inconclusive; do not tune the protocol after inspecting holdout results.

For every variant and round, run the separate six-scenario memory lane with
both engines and three repetitions: 144 isolated workers in total. Keep physical
RSS/PSS, logical records/payload and collection timing separate. Paired timings
compare absolute Velum execution time against its own ordinary build, not ratios
to independently sampled QuickJS timings. Prepared useful-work checksums must
match across variants and rounds.

## Execution and acceptance boundaries

The repository now provides an explicit opt-in launcher. Run it from a clean,
committed checkout; it never changes ordinary Cargo settings or CI. Its Rust
artifact validators live in the runner crate, with no additional engine
dependencies or Python requirement. Prerequisites are Linux x86_64, the selected
Rust toolchain's matching LLVM 22 `llvm-profdata` and `llvm-size`, `setsid`,
`taskset`, `/usr/bin/time`, and the ordinary shell/Git/Cargo tools. Install missing
tools separately: the launcher never installs anything.

```bash
bash scripts/run-pgo-experiment.sh --execute \
  --repo /absolute/path/to/clean/velum-checkout \
  --artifact-root "$HOME/velum-fuzzing-artifacts/performance/pgo" \
  --preset release --rounds 2 --cpu 0

# A separate, freshly trained experiment; not six builds on every invocation.
bash scripts/run-pgo-experiment.sh --execute \
  --repo /absolute/path/to/clean/velum-checkout \
  --artifact-root "$HOME/velum-fuzzing-artifacts/performance/pgo" \
  --preset thin-lto --rounds 2 --cpu 0

# After timing, execute that experiment's exact saved PGO binary without rebuilding.
bash scripts/check-pgo-correctness.sh \
  /absolute/path/to/completed-experiment \
  /absolute/path/to/pinned-patched-test262 /absolute/path/to/qjs
```

Each invocation builds only ordinary, instrumented and profile-use variants of
one preset. `release` preserves the repository's release configuration;
`thin-lto` applies Cargo overrides `profile.release.lto="thin"` and
`profile.release.codegen-units=1` consistently to all three variants. This tests
the combined configuration, not each setting's independent effect. Cargo's
default `lto=false` may still perform local ThinLTO, so this is not a comparison
against an entirely LTO-free compiler. Neither preset promises a speedup.
Profiles are never shared between presets, source snapshots or toolchains.
Each preset invocation has its own absolute source path. Its ordinary/PGO pair
is controlled, but separate invocations do not isolate the causal effect of
ThinLTO: cross-preset attribution requires an additional same-path comparison
with all other settings fixed. Inherited compiler/profile overrides and visible
Cargo configuration files are rejected instead of silently altering a preset.
The launchers clear inherited `GIT_*` routing/configuration variables, ignore
system/global Git configuration, and bind Git operations explicitly to the
requested checkout. The correctness wrapper uses only experiment-owned temporary
index/object storage when checking the pinned, patched corpus.

CPU 0 is the default, with no automatic fallback. Select another available CPU
explicitly or use `--cpu inherit`; compare results only with matching affinity
and report the choice. The default two rounds preserve AB/BA ordering; one to
five rounds may be selected before execution. The frozen sampling and quality
thresholds are not adjustable launcher knobs. The Rust validators reject exact
ID/source/checksum/build-identity mismatches, malformed or missing profiles,
unclassified PGO diagnostics, and missing functions observed during training.
They preserve per-round absolute Velum medians and ratios in
`holdout-comparison.{tsv,json}`. Build/training costs remain in `steps/*.time`,
whole-runner bytes and hashes in `binaries.tsv`, ELF sections in
`steps/*-sections.log`, and memory observations in the per-round JSON reports.
Memory validation requires the complete worker/phase matrix, workload checksums,
per-VM indices and category totals. The comparison also checks Velum's logical
records, payload, runtime steps and reclaimed records across variants and
rounds. Logical drift retains a diagnostic and blocks acceptance; RSS/PSS and
QuickJS allocator bytes remain separate observations, not equality gates.
A failed revalidation invalidates prior derived comparisons instead of leaving
stale successful results visible; raw inputs and diagnostics remain preserved.

`complete-needs-review` is collection status, not adoption. The correctness
wrapper verifies the frozen Test262 pin and tracked patch set, records the
external QuickJS executable hash, runs the saved candidate without filters,
rebuilding, benchmark execution, baseline updates or profile output, and checks
source/corpus/binary/profile integrity afterwards, including interrupted runs.
Its stricter PGO acceptance gate requires all six correctness suites to pass
without failures or skips and a full pass candidate containing the frozen
baseline. Outputs remain in a unique `correctness-*` artifact subdirectory.
Interrupted or failed steps retain commands, logs, exit status and available
resource timings; only each step's owned process group is cancelled.

### Historical prototype

The initial external prototype and its tests live under
`$HOME/velum-fuzzing-artifacts/performance/campaign-20260922/`. It requires Linux
x86_64, `setsid`, Rust with matching bundled LLVM 22 tools, GNU time and Python with
PyYAML; it never installs dependencies. Every invocation owns a fresh directory
outside the checkout, containing immutable sources and binaries, raw and merged
profiles, source/tool hashes, commands, reports and completion status. This
prototype produced the historical results below; it is not the repository
launcher described above.

```bash
bash "$HOME/velum-fuzzing-artifacts/performance/campaign-20260922/pgo-plan-prototype.sh" \
  --execute --repo /absolute/path/to/clean/velum-checkout \
  --artifact-root "$HOME/velum-fuzzing-artifacts/performance/pgo" \
  --rounds 2 --cpu 0
```

Run the prototype's lightweight and fake-tool integration checks before real
compilation. Keep compilers, CI and fuzzers off the measured host during timed
execution; the existing runner owns the shared performance lock. Missing-function
diagnostics remain visible for review; they differ from retained zero-count
profile records. A missing previously trained
engine function, invalid profile or unclassified PGO warning blocks evaluation.
The final status `complete-needs-review` means collection finished, not adoption.

Review per-case gains and regressions, both rounds, quality flags, training and
build costs, profile coverage, size and memory before deciding on a follow-up.
Ordinary correctness CI does not validate the profile-use executable. Its
correctness must be checked separately before recommending deployment. Retaining
the ordinary build, even after an interesting experimental speedup, is valid.

The historical prototype passes 71 lightweight and fake-tool integration tests, including
full synthetic AB/BA execution and bounded SIGINT/SIGTERM cleanup of descendant
processes. The summary validator passes 17 tests; the saved-binary correctness
report validator passes six. A tiny real LLVM probe confirms profile grammar
and rejection of actual mismatch diagnostics. These are infrastructure checks,
not evidence of a faster engine.

## Initial attempt and discovered array accounting defect

The first actual attempt used source
`1b9fa119611b1c0c4623e7dc1762a2e24d2de035`, after PR #722. All three builds and
six training cases completed; profile diagnostics reported no missing-function
or mismatch warnings. Before any PGO holdout execution, however, the ordinary
build failed the string-processing holdout during automatic collection:
`ObjectProperty storage ledger mismatch: tracked 2816258, observed 2630789`.

The cause was an existing July array-front fast path: dense `shift` removed
properties without releasing the owner's logical property count; dense
`unshift` inserted properties without reserving them. Both bypassed enumerable
property accounting. Besides failing collection/accounting checks, inserting
into an empty array could leave `for...in` unaware of the new elements.

The repair routes these operations through the owning object's accounting
helpers. Shifting a hole releases no property; shifting a present element
releases one. Eligible dense insertion reserves its complete growth before
mutation. Pure eligibility checks preserve the existing generic fallback's
observable order. Regression tests cover packed/holey arrays, enumeration,
fallback descriptors/prototypes, automatic and explicit collection, rejected
reservations and independent VM budgets. Five of the first six regressions
failed before the repair; all eleven final cases pass afterwards with Velum's
`OptimizationMode::Enabled` and `OptimizationMode::Disabled`. These are engine
optimization modes, not ordinary/profile-use compiler variants. The full local
gate passes 1,844 test results without failures or ignored cases. Focused
Test262 passes all 84 variants from 42 files, plus 99 QuickJS differential cases.

The failed attempt remains at
`$HOME/velum-fuzzing-artifacts/performance/pgo/pgo-20260922T193128Z-Fz8y5zqe`.
Its status is `incomplete`; partial timings are not accepted PGO evidence.
The repaired engine was measured with a fresh snapshot, new training and new
profile under the unchanged sampling protocol. None of the failed attempt's
timings or profile records enter the comparison below.

## Reviewed experiment: 2026-09-22

The complete second attempt is preserved under
`$HOME/velum-fuzzing-artifacts/performance/pgo/pgo-20260922T200005Z-jeTteKWB`.
Its `reviewed-summary.md` and `.json` bind all reports to source
`be9dc9a2c206c10681a3a8c08c15e5f1e7ac5d72`, tree
`3560eb0d9efaddefb181b9ac931ee9aec0f58505`. Later PR commits change documentation
only. The compiler is Rust 1.96.0 / LLVM 22.1.2 on the same Linux / Ryzen 9
9950X3D host, with 30 build jobs and sequential measurements pinned to CPU 0.

All six training cases, 24 holdout timing rows and 144 memory workers passed.
There were no invalid measurements, failed rows, missing-function warnings or
profile mismatch diagnostics. Six fresh raw profiles contain observed execution
of 1,316 engine-owned functions; the complete IR profile has 25,966 functions
and 325,221 blocks. These counts are not language-feature or source-line coverage.

| Holdout | Ordinary ms | PGO ms | PGO / ordinary, round 1 | Round 2 | Time reduction |
| --- | ---: | ---: | ---: | ---: | ---: |
| Object transformation | 42.550 | 25.520 | 0.5996 | 0.5999 | 40.0% |
| Method dispatch | 48.225 | 30.940 | 0.6392 | 0.6440 | 35.8% |
| JSON ingestion | 13.345 | 9.450 | 0.7089 | 0.7073 | 29.2% |
| String processing | 58.520 | 39.650 | 0.6751 | 0.6800 | 32.2% |
| Collection indexing | 15.975 | 10.204 | 0.6454 | 0.6322 | 36.1% |
| Tree allocation | 35.275 | 24.710 | 0.6983 | 0.7027 | 29.9% |

The millisecond columns are geometric means of the two per-round medians.
The six-case time ratio is 0.6599: 34.0% lower time, or about 1.52 times the
throughput for these workloads. Round means are 0.6600 and 0.6599; every case
improved in both rounds. Typed useful-work checksums match exactly across all
four observations per holdout. Two rounds and the CV gate do not establish
statistical significance or predict performance on arbitrary applications.

### Build cost, size and memory

| Whole runner | File bytes | `.text` bytes | Build wall seconds | Build CPU seconds |
| --- | ---: | ---: | ---: | ---: |
| Ordinary | 16,957,224 | 10,530,471 | 45.63 | 181.36 |
| Instrumented | 33,536,096 | 16,058,434 | 48.33 | 182.66 |
| PGO | 16,297,584 | 9,197,911 | 52.22 | 165.22 |

The final runner is 3.9% smaller; its `.text` section is 12.7% smaller.
Training takes another 107.22 wall seconds / 106.71 CPU seconds; merging takes
0.20 seconds. A from-scratch instrument/train/merge/use sequence therefore costs
about 208 wall seconds versus 46 seconds for the ordinary build, excluding
launcher overhead and evaluation. Training includes the live QuickJS reference
and runner work; build CPU time includes compiler child processes. These are
whole-runner, warm-dependency-cache results, not standalone engine build costs.

All 162 paired logical phase/repetition comparisons are equal. A detailed
cross-round audit also finds exact equality across 3,912 per-VM snapshots and
117,360 category entries, including runtime steps and reclaimed records.
QuickJS's separate allocator-byte counter varies by at most 176 bytes across
15 groups; its other counters match. Median live
process RSS across six samples is 9.146 to 8.049 MiB for hello-world, 13.133 to
11.762 MiB for the retained graph, and 28.180 to 26.812 MiB for 50 independent
VMs. This is lower measured process residency, not fewer logical allocations or
a per-VM heap-size claim. Full RSS/PSS phases and availability remain in the
external summary. Raw Linux `VmHWM` readings sometimes decrease at teardown;
they are retained as observations, not asserted to be an exact monotonic peak.

### Correctness and decision

The saved profile-use executable has SHA-256
`f9df83620b9b0e696b684b9d6e231eb726e84021fd310c5884d02c2bb6086f10`;
the frozen merged profile is
`6d57ecb3d392069cced20c1d5ce7251c6ecc8bae138f808a7813e21c2e6c8f1e`.
Its independent complete correctness run, performed after all timed execution,
passes all 102,578 Test262 variants / 53,404 files, all 99 QuickJS differential
cases, 69 engine fixtures and 121 active-subset cases: zero failures or skips.
The expected-pass baseline also matches all 102,578 variants. This directly
executes the saved binary without rebuilding, with 30 workers, no filters,
no baseline updates and no profile output. Wall time is 362.01 seconds;
source, corpus, binary and profile integrity checks pass before/after execution.
Artifacts are in `correctness-20260922T201204Z-V79X6sQk` inside the experiment
directory. The required ordinary exact-head CI is a separate gate, linked with
its exact-tree artifact in [PR #723](https://github.com/surveria/velum/pull/723).

The experiment motivated the opt-in workflow above, not automatic enablement.
Ordinary release defaults remain unchanged. Before broader adoption,
add unrelated application and embedding/async/regexp holdouts, other target
hardware, and a maintainable profile-refresh/revalidation policy. This small
six-family training set cannot represent all embedders, and the result does
not extend the earlier JetStream or direct-library measurements to PGO.

## Reviewed repository launcher: 2026-09-22

The productized Rust-validated launcher completed a fresh `release` experiment
after the second runtime tranche, using source
`b7406f15e2ddbad1b3531d6a13e4b939f12cc2b4`, tree
`df6d31562693591060c64262440e4c711bf988a3`. The experiment is preserved under
`$HOME/velum-fuzzing-artifacts/performance/next-campaign-20260922/pgo/pgo-20260922T220707Z-keSmWwFg`.
It does not reuse the historical prototype's profile or measurements.

All six training programs, 24 holdout timing rows and 144 memory workers pass.
There are no missing-function or profile-mismatch diagnostics. Six raw profiles
merge into 27,418 IR function records and 341,443 blocks; the conservative
symbol recognizer observes execution of 1,333 Velum-related functions. These
are profile records, not language-feature, source-line or exact crate-ownership
coverage. The compiler, hardware, CPU affinity and frozen sampling protocol
match the documented experiment setup above.

| Holdout | Ordinary ms | PGO ms | PGO / ordinary, round 1 | Round 2 | Time reduction |
| --- | ---: | ---: | ---: | ---: | ---: |
| Object transformation | 43.290 | 27.830 | 0.6465 | 0.6393 | 35.7% |
| Method dispatch | 47.309 | 33.740 | 0.7096 | 0.7168 | 28.7% |
| JSON ingestion | 13.350 | 9.680 | 0.7263 | 0.7239 | 27.5% |
| String processing | 57.955 | 40.315 | 0.6960 | 0.6952 | 30.4% |
| Collection indexing | 15.920 | 11.355 | 0.7117 | 0.7148 | 28.7% |
| Tree allocation | 35.230 | 25.759 | 0.7407 | 0.7218 | 26.9% |

The geometric-mean time ratio is 0.7029, or 29.7% less time / 1.423x throughput
on this cohort. Round ratios are 0.7045 and 0.7013; every case improves in both
rounds. All typed useful-work checksums match. Maximum engine timing CV is 8.0%.
This is a new within-experiment comparison, not a causal explanation of the
difference from the historical prototype's 34.0% result: source, runner and
absolute build locations differ between those experiments.

| Whole runner | File bytes | `.text` bytes | Build wall seconds | Build CPU seconds |
| --- | ---: | ---: | ---: | ---: |
| Ordinary | 17,779,856 | 11,076,631 | 46.23 | 188.73 |
| Instrumented | 34,980,344 | 16,815,458 | 48.93 | 189.26 |
| PGO | 17,077,904 | 9,651,351 | 52.72 | 169.77 |

The final runner file is 3.9% smaller and its `.text` is 12.9% smaller. Training
takes 107.02 wall seconds / 106.65 CPU seconds, including reference execution;
merging takes 0.20 seconds. Instrument/train/merge/use costs 208.87 wall seconds,
excluding evaluation and launcher overhead, versus the 46.23-second ordinary
build. All variants start with empty Cargo output; dependency downloads are
already available. These are whole-runner costs on one host.

The four complete memory reports agree on all corresponding Velum checksums,
logical phases, runtime steps, reclaimed records and per-VM/category counters:
3,912 VM snapshots and 117,360 category entries are validated. RSS/PSS and
QuickJS allocator bytes remain separate observations in the raw reports;
logical equality does not establish equal physical memory usage.

The exact saved PGO executable, SHA256
`c6abd1cb371530703bcebea1b9b11012056348fb8021da2e4afc69406ddb4f76`,
passes all 102,578 Test262 variants / 53,404 files, 99 QuickJS differential
cases, 69 engine fixtures and 121 active-subset cases: no failures or skips.
The full pass candidate is byte-identical to the frozen baseline. The
353.78-second run performs no rebuild, benchmark, profile output or baseline
update; all source/corpus/binary/profile integrity checks pass. Receipts and
the validated report are in `correctness-20260922T221916Z-SfOvHMXR` inside the
experiment. Required ordinary exact-tree CI remains a separate integration gate.

Release defaults remain unchanged. The independent ThinLTO+CGU1 result
in [the runtime report](safe-runtime-performance.md#separate-thinlto-configuration-experiment)
must not be added to this result; the combined preset is available but has not
been measured in this campaign. Broader application/embedding holdouts and a
profile-refresh policy are still required before default adoption.

The build sequence follows the [Rust PGO guide](https://doc.rust-lang.org/rustc/profile-guided-optimization.html).
Profile merging and zero-count handling follow the
[LLVM 22 profile-tool documentation](https://releases.llvm.org/22.1.0/docs/CommandGuide/llvm-profdata.html).
