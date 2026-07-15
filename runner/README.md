# rsqjs-test-runner

Test and benchmark runner for the [`rs-quickjs`](https://github.com/surveria/rs-quickjs)
JavaScript engine.

This crate lives in `runner/` as a nested workspace inside the engine repository.
The engine crate stays dependency-light because the runner's reporting,
benchmarking, and reference-engine dependencies belong only to this workspace.
The runner depends on the engine through the public API with `rs-quickjs = {
path = ".." }`.

## Layout

- `src/main.rs` — the `rsqjs-test-runner` binary (report and benchmark modes).
- `src/*.rs` — engine/Test262/QuickJS-differential corpora drivers, the
  in-process benchmark sampler, and report/rollup rendering.

## Building

Built from the engine repository:

```sh
cargo build --manifest-path runner/Cargo.toml --features reference-quickjs
```

The `reference-quickjs` feature adds an in-process QuickJS reference (via the
`rquickjs` binding) for benchmark comparison.

## Standalone JetStream

Run the shell corpus without repeating correctness, Test262, differential, or
project benchmark work:

```sh
./scripts/run-jetstream.sh
```

The script owns the exclusive host lock and uses the committed content-addressed
QuickJS baseline in read mode. `RSQJS_JETSTREAM_FILTER` accepts exact ids and
explicit trailing-star prefixes. Live QuickJS execution occurs only when
`RSQJS_JETSTREAM_QUICKJS_BASELINE=refresh` is set explicitly; refresh output is
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
