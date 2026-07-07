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
