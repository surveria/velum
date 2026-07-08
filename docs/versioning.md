# Versioning And Build Identity

Ordinary task pull requests do not bump Cargo package versions. The repository
uses build identity, not synthetic patch versions, to recover the exact source
state for a binary, report, or benchmark run.

The engine build script embeds the engine package version and source commit.
The runner build script embeds the runner package version and source commit.
Generated reports also record the tested commit, tested tree, workflow run, and
pull request metadata. Those fields are the authoritative trace from an artifact
or binary back to source.

Cargo package versions are release metadata. Change them only in explicit
release or version pull requests, not as a routine part of feature, fix,
benchmark, documentation, or workflow tasks. A release/version pull request
should choose the semantic version for each crate that is actually being
released, update the matching `Cargo.toml` file, refresh the matching
`Cargo.lock` entry, describe the version decision, pass the full validation
gate, and create a release tag after the tested commit reaches `main` when a
tag is needed.

Keep both lockfiles tracked. They make local validation, CI, report generation,
and historical bisection use the same dependency graph that was tested. Removing
the lockfiles would reduce some merge conflicts, but it would also make report
reproduction and CI failures more dependent on external registry state.

The post-merge report publisher is intentionally report-only. It copies the
already-tested report artifact into `reports/test-runs/`, updates the benchmark
rollup and chart, and records the tested source commit and tree. It must not
change Cargo package versions after merge, because that would create a new
source tree that did not produce the report artifact.
