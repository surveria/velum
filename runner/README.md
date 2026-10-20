# velum-test-runner

Test and benchmark runner for the [`velum`](https://github.com/surveria/velum)
JavaScript engine.

This crate lives in `runner/` as a nested workspace inside the engine repository.
The engine crate stays dependency-light because the runner's reporting,
benchmarking, and reference-engine dependencies belong only to this workspace.
The runner depends on the engine through the public API with `velum = {
path = ".." }`.

## Layout

- `src/main.rs` — the `velum-test-runner` binary (report and benchmark modes).
- `src/*.rs` — engine/Test262/QuickJS-differential corpora drivers, the
  in-process benchmark sampler, and report/rollup rendering.

## Building

Built from the engine repository:

```sh
cargo build --manifest-path runner/Cargo.toml --features reference-quickjs
```

The `reference-quickjs` feature adds an in-process QuickJS reference (via the
`rquickjs` binding) for benchmark comparison.

### Private registry access

The runner takes `human-yaml` from the private Surveria Cargo registry
(`crates.surveria.org`). The engine and the interactive CLI do not depend on
it and build without registry access. The repository `.cargo/config.toml`
defines the `surveria` registry, so Cargo only needs a read token with its
`Bearer` prefix, either in the environment:

```sh
export CARGO_REGISTRIES_SURVERIA_TOKEN="Bearer <token>"
```

or once in `$CARGO_HOME/credentials.toml`:

```toml
[registries.surveria]
token = "Bearer <token>"
```

Use Rust 1.96 or newer with the token: Cargo from Rust 1.68 to 1.95 is
affected by CVE-2026-5222 and can send a registry token to another registry
on the same host.

CI passes the token from the `SURVERIA_REGISTRY_TOKEN` repository secret to
every job that runs Cargo. GitHub does not expose secrets to pull requests from
forks, so the required CI job runs only for branches of this repository;
without registry access, fork contributors can still build and test the engine
and the CLI locally.

### Registry freshness

A crate updated in the registry must also reach this repository.
`scripts/check-registry-freshness.sh` runs `cargo surveria check --strict` from
the `cargo-surveria` CLI (sources in `surveria/healthcheck`) and reports in the
style of clippy:

- `error`: the registry has a newer compatible release of a locked crate, or a
  crates.io package shadows the name of a registry crate;
- `warning`: the registry has a newer incompatible release.

A newer compatible release is fixed by updating the lock file of the affected
workspace, for example
`cargo update --manifest-path runner/Cargo.toml -p human-yaml`. A newer
incompatible release needs a raised requirement in `Cargo.toml`.

`scripts/check-fast.sh` runs the check next to clippy for every workspace it
lints that takes registry crates; with `VELUM_FAST_RUNNER=1`, that includes the
runner. Install the CLI once:

```sh
cargo install --registry surveria cargo-surveria --locked
```

It needs Rust 1.96 or newer and the registry read token. `cargo install` ignores
the repository `.cargo/config.toml`, so the command needs the same
`[registries.surveria]` table in `$CARGO_HOME/config.toml`. Alternatively, run
it from the repository root with `--config .cargo/config.toml` added. Without
the CLI, the check prints a warning with the install command and repeats it
when `check-fast.sh` exits.

CI checks every workspace with a tracked `Cargo.lock`, including the crates.io
name-shadowing check for workspaces that do not take registry crates yet. The
`rust` job installs the pinned CLI with `scripts/prepare-cargo-surveria.sh` into
the persistent runner cache; only a cache miss builds it. The check runs before
the correctness gate, so a stale lock file fails CI early. When the check warns
that a newer cargo-surveria is out, bump the pin in that script.

## Standalone JetStream

Run the shell corpus without repeating correctness, Test262, differential, or
project benchmark work:

```sh
./scripts/run-jetstream.sh
```

The script owns the exclusive host lock and uses the committed content-addressed
QuickJS baseline in read mode. `VELUM_JETSTREAM_FILTER` accepts exact ids and
explicit trailing-star prefixes. Live QuickJS execution occurs only when
`VELUM_JETSTREAM_QUICKJS_BASELINE=refresh` is set explicitly; refresh output is
reviewed and committed as a separate baseline change.

Each run writes derived Markdown, compact schema-v1 YAML, bounded component
YAML, and a bounded TSV view. The compact YAML keeps every selected official
row and exact aggregate counts within the repository's 1,000-line canonical
limit. CI rejects local-only exhaustive output. Normal read mode does not
compile or run the QuickJS binding; canonical publication stores only Markdown
plus compact YAML, while component/TSV files remain workflow artifacts.

## Test262 Accounting

The full Test262 report keeps two progress views:

- `Test262 file conformance` is the comparison-oriented view. One upstream
  source file counts once, and it passes only when every required Test262
  execution variant for that file passes.
- `Test262 full corpus` is the diagnostic view. It counts each metadata-driven
  execution variant separately, such as `default`, `strict`, `module`, and
  `raw`, so default/strict failures remain visible.

This follows Test262 strict-mode accounting: ordinary script tests run in both
non-strict and strict mode, while `onlyStrict`, `noStrict`, `module`, and `raw`
tests each produce one required execution. The runner still uses the engine's
current script evaluation surface for module-tagged files and a minimal host
harness for `$262`-adjacent behavior, so the file-level row is the closest
dashboard-style compatibility metric, not a claim that every official Test262
host capability is implemented.

Full-corpus execution uses a dedicated conformance limit profile sized for
official large-vector cases, including 32 MiB buffers and arrays with more than
two million elements. These ceilings do not change the embeddable VM defaults;
applications continue to select their own `RuntimeLimits` policy.
