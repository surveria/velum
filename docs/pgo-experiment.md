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
that actual engine-owned functions executed. Freeze the merged profile before
evaluating any `holdout_*` workload or memory scenario. Never incorporate
holdout measurements or correctness runs into training.

Evaluate six structurally different holdouts in ordinary/PGO order, then
PGO/ordinary order, sequentially on CPU 0. JSON and tree-allocation cases use
15,000 ms minimum sampling, three samples and a 60,000 ms total budget. Other
cases use 5,000 ms, five samples and a 30,000 ms budget. All retain a 150 ms
warmup, the existing 1 ms minimum operation, 10% maximum CV and three attempts.
These settings are fixed before training. A quality failure remains
inconclusive; do not tune the protocol after inspecting holdout results.

For every variant and round, run the separate six-scenario memory lane with
both engines and three repetitions: 144 isolated workers in total. Keep physical
RSS/PSS, logical records/payload and collection timing separate. Paired timings
compare absolute Velum execution time against its own ordinary build, not ratios
to independently sampled QuickJS timings. Prepared useful-work checksums must
match across variants and rounds.

## Execution and acceptance boundaries

The external prototype and its tests live under
`$HOME/velum-fuzzing-artifacts/performance/campaign-20260922/`. It requires Linux
x86_64, Rust with matching bundled LLVM 22 tools, GNU time and Python with
PyYAML; it never installs dependencies. Every invocation owns a fresh directory
outside the checkout, containing immutable sources and binaries, raw and merged
profiles, source/tool hashes, commands, reports and completion status. This
prototype is not a shipped portable repository entrypoint.

```bash
bash "$HOME/velum-fuzzing-artifacts/performance/campaign-20260922/pgo-plan-prototype.sh" \
  --execute --repo /absolute/path/to/clean/velum-checkout \
  --artifact-root "$HOME/velum-fuzzing-artifacts/performance/pgo" \
  --rounds 2 --cpu 0
```

Run the prototype's lightweight and fake-tool integration checks before real
compilation. Keep compilers, CI and fuzzers off the measured host during timed
execution; the existing runner owns the shared performance lock. Missing cold
function profiles remain visible for review. A missing previously trained
engine function, invalid profile or unclassified PGO warning blocks evaluation.
The final status `complete-needs-review` means collection finished, not adoption.

Review per-case gains and regressions, both rounds, quality flags, training and
build costs, profile coverage, size and memory before deciding on a follow-up.
Ordinary correctness CI does not validate the profile-use executable. Its
correctness must be checked separately before recommending deployment. Retaining
the ordinary build, even after an interesting experimental speedup, is valid.

The launcher passes 71 lightweight and fake-tool integration tests, including
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
failed before the repair; all pass afterwards in both optimization modes.

The failed attempt remains at
`$HOME/velum-fuzzing-artifacts/performance/pgo/pgo-20260922T193128Z-Fz8y5zqe`.
Its status is `incomplete`; partial timings are not accepted PGO evidence.
The repaired engine requires a fresh snapshot, new training and new profile,
under the unchanged sampling protocol. The reviewed PGO comparison remains
pending at this checkpoint.

The build sequence follows the [Rust PGO guide](https://doc.rust-lang.org/rustc/profile-guided-optimization.html).
Profile merging and zero-count handling follow the
[LLVM 22 profile-tool documentation](https://releases.llvm.org/22.1.0/docs/CommandGuide/llvm-profdata.html).
