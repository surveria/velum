# Project Rules

These rules are mandatory for humans and agents working in any part of this repository.

## Language And Text

- Always answer users in Russian.
- Code comments must always be written in English.
- Runtime logs must always be written in English.
- Documentation text must always be written in English.

## Tasks, Branches, And Worktrees

- Create a separate git worktree for every task under `.claude/worktrees/<task>`.
- Create a separate branch for every task from a fresh `origin/main`.
- Immediately make the task visible on GitHub: create an empty start commit with `[skip ci]`, push the branch, and open a draft PR that describes the planned scope. If the branch already has a real first commit, push that instead of an empty commit.
- Keep the PR as draft while implementation is in progress. Draft PRs are for visibility and discussion; the full CI gate starts when the PR is marked ready for review or receives new ready-state commits.
- Do not delete task branches after completion, either locally or on GitHub. Branch history is part of the work record.
- Remove the task worktree after the task is complete with `git worktree remove <path>`. If a legacy worktree still contains a checked-out submodule and Git refuses to remove it, remove it with `rm -rf <path>` followed by `git worktree prune`.

## Pull Requests And Merge

- All changes reach `main` only through a pull request: push branch, open a draft PR early, mark it ready when implemented, wait for green CI, then merge.
- PR descriptions must be detailed and include what changed, why it changed, problems or noteworthy decisions during the work, and what remains for later.
- PR descriptions should include the validation summary and link or path for the CI report artifact. Do not commit generated full test reports in ordinary feature PRs unless the branch is explicitly a canonical report refresh.
- Split work into meaningful commits so the branch history shows the solution path.
- Every PR commit must have a GitHub-verified signature before the ready-PR CI gate can pass. Agents must use the repository signing key and a GitHub-verified author and committer email; do not use placeholder identities such as `codex@local`.
- Merge to `main` with one squash commit and a detailed commit message. The detailed history remains in the branch.
- The repository should use GitHub merge queue for `main`, with the CI `merge_group` check required before merge. Use a queue group size of one when report history must stay one-PR-per-report.
- The `main` branch ruleset must require the ready-PR/merge-queue CI check before merge and must allow the report publisher token to push report-only commits, or the post-merge canonical report commit will fail.
- After a PR is merged, GitHub Actions publishes the canonical report: it downloads the report artifact for the tested tree, stores the tested source commit on the single `ci-tested-sources` archive branch so report commit SHAs remain fetchable, copies exactly one report into `reports/test-runs/`, regenerates `reports/benchmark-rollup.md` and `reports/benchmark-summary.jpg`, and pushes one report-only commit to `main`.
- If the task is fully implemented and CI is green, push, PR creation, and merge do not need extra confirmation. Ask only when implementation is incomplete, there is doubt, or CI is red.
- After merge, update the main repository directory to fresh `main` with `git checkout main && git pull`.

## Repositories, Crates, And Versioning

- The engine and its test/benchmark runner live in this repository but remain separate crates:
  - `rs-quickjs` (the root crate): the embeddable engine library plus the `rsqjs` smoke CLI. It is a standalone, dependency-light crate — only what the engine itself needs. Do not add reporting, benchmarking, or reference-engine dependencies here. It builds and tests on its own, without the runner.
  - `runner/`: the `rsqjs-test-runner` nested workspace. It owns heavier dependencies such as `tabled`, `plotters`, `image`, `serde`, `rquickjs`, and report aggregation code. It depends on the engine only through `rs-quickjs = { path = ".." }` and the public API.
- To build or run the runner by hand, use `--manifest-path runner/Cargo.toml` and add `--features reference-quickjs` for the in-process QuickJS reference. No submodule checkout, Cargo `paths` override, or separate runner repository is required.
- Every pull request that changes the engine must bump `version` in the root `Cargo.toml`. Every pull request that changes the runner must bump `version` in `runner/Cargo.toml`. The two version independently; use semantic versioning and state the new version in the PR description. CI enforces the engine side on pull requests: `scripts/check-fast.sh` and `scripts/test-all.sh` run `scripts/check-version-bump.sh` whenever `RSQJS_BASE_REF` is set (CI sets it to the base branch on pull requests), so if `src/`, `Cargo.toml`, or `Cargo.lock` changed relative to the base branch, the engine version must be strictly higher than the version on that branch. Ready pull-request CI and merge-queue CI use `scripts/test-all.sh` and keep generated reports as artifacts instead of branch commits.

## Gitignore Whitelist Model

- The root `.gitignore` must deny everything with `*` and then explicitly allow only required files and directories.
- Subprojects may add their own `.gitignore` files to allow their specific tracked files.
- Use only this model. Every new source file or tracked directory must be explicitly allowed, otherwise it may be silently omitted from commits and CI can fail because files are missing.
- Whenever adding a new kind of file, add the matching whitelist rule in the same change.

## Product Architecture

- `rs-quickjs` is an embeddable Rust library first. The CLI, test runner, and scripts are support surfaces for smoke testing, differential checks, and benchmark orchestration.
- Public API decisions must optimize for Rust applications that run many isolated JavaScript virtual machines in one process.
- Do not introduce mutable global JavaScript state. Shared engine data must be immutable or guarded by explicit synchronization and resource accounting.
- Every VM-facing feature must define how it behaves across independent VM instances, including resource limits, teardown, errors, queued jobs, and host callbacks.
- Host extensions are part of the core product surface. New runtime work must preserve a path for typed Rust host functions, contextual `Result` errors, async callbacks, and embedder-owned executors.
- Do not make the CLI the only way to exercise a feature. If a feature affects embedders, add or plan direct library API tests and benchmarks in addition to CLI smoke coverage.

## Rust Development Rules

- Write idiomatic Rust.
- Use `Result` with `thiserror` or `anyhow`, and add context so errors are informative.
- Never use `unwrap()`, `expect()`, `panic!()`, or any other construct that intentionally crashes. Tests must also return `Result`.
- Never use indexing with `[]` when it can panic. Use `first`, `last`, `get`, iterators, or other checked mechanisms.
- `unsafe` is forbidden with `unsafe_code = "deny"` in `[lints.rust]`. If `unsafe` ever appears unavoidable, discuss it first and add a `// SAFETY:` block explaining the invariants.
- Use `parking_lot` for synchronization.
- Write async code by default.
- Use newtype wrappers by default.
- Prefer early returns such as `if let Some(..) = opt { ... } else { return ... }` to avoid unnecessary nesting.
- Do not use `todo!()`, `unimplemented!()`, or production-code placeholders.
- Do not ignore results. Do not use `let _ =` for `Result` or `Option`; handle the value or propagate it with `?`.
- Use `checked_*`, `saturating_*`, or `overflowing_*` arithmetic and handle overflow explicitly.
- Use structured logs, for example with `tracing`; never log secrets or PII, and log errors with context chains.
- Do not store secrets or PII in the repository. Use `secrecy` or `zeroize` when needed.
- Keep code files at or below 800 lines.
- Avoid overly long functions and methods. Split growing logic into smaller units.
- Avoid magic constants and string literals. Move them to `const`, `enum`, configuration, or named helpers.
- Do not use `dashmap`.
- Minimize mutability and global state. Prefer pure functions and explicit dependencies.
- Always run `cargo fmt`.
- Always run `cargo clippy` and fix every warning. Suppress lints only when there is a strong reason.

## Testing

- Keep tests and test fixtures under `tests/`. Do not hide test code inside production modules when an integration test or fixture is appropriate.
- Tests must follow the same rules as production code: no `unsafe`, no `unwrap()`, no `expect()`, no intentional panics, no unchecked indexing, no ignored `Result` or `Option`.
- Every test case must have one finished status: passed, failed, or skipped.
- Skipped tests must always include a concrete reason, such as the missing engine feature, missing reference runner, or unsupported external corpus step.
- Failed tests must report enough context to diagnose the problem: suite, case id, source path when applicable, expected behavior, and actual behavior or error.
- `scripts/check-fast.sh` is the local iteration gate. It runs formatting, clippy, tests, and docs for the engine without materializing external corpora or generating benchmark reports. Set `RSQJS_FAST_RUNNER=1` when the runner should also be formatted, linted, tested, and documented against the local engine checkout.
- `scripts/test-all.sh` is the full pull-request, merge-queue, report, and benchmark entrypoint. It runs the full validation sequence and writes reports under `target/rsqjs-reports/` by default. Agents should use the printed report path to evaluate benchmark progress locally, but must not commit that generated report from an ordinary feature branch.
- The Rust test runner is responsible for executing engine-level test cases and writing the final test report.
- Ordinary feature PRs must not commit generated full test reports. Full reports generated by CI live under `target/rsqjs-reports/` and are uploaded as artifacts named by the tested tree SHA.
- Canonical tracked test reports belong under `reports/test-runs/`. Each tracked report must be a separate Markdown file with a sortable UTC timestamp suffix. After merge, CI publishes the canonical report from the already-tested artifact and regenerates the tracked rollup and chart. Generate one manually only by setting `RSQJS_TRACKED_REPORT=1` or an explicit `RSQJS_TEST_REPORT_PATH`.
- Test reports must include a summary and a per-case table that records all passed, failed, and skipped cases.
- Official ECMAScript compatibility work should use Test262 as the external corpus and track pass or skip status by feature area.
- QuickJS should remain the reference implementation for differential behavior checks where the feature is implemented locally.
- Future benchmark reports should follow the same pattern: one command, Rust-owned execution, CI artifacts for ordinary PRs, post-merge tracked canonical reports, and clear comparison against QuickJS where possible.
- Benchmark cases must run sequentially, not in parallel, so measurements do not interfere with each other.
- Active benchmark cases must be large enough to pass the runner measurement quality gate. The default gate rejects rows with a median operation below 1 ms, sample variation above 10%, or calibration at the iteration cap.
- Use ordinary tests for tiny semantic checks. Do not keep microsecond-scale smoke cases in the active benchmark corpus.
- Benchmark reports must separate local engine measurements from QuickJS measurements and mark unavailable reference runs as skipped with a concrete reason.
- Embedding-facing benchmark cases must include direct library measurements where possible, not only CLI process measurements.

## Rust Lints

Use strict lints in `Cargo.toml`:

```toml
[lints.rust]
unsafe_code = "deny"

[lints.clippy]
nursery = { level = "deny", priority = -1 }
pedantic = { level = "deny", priority = -1 }
uninlined_format_args = "deny"
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
todo = "deny"
unimplemented = "deny"
dbg_macro = "deny"
mem_forget = "deny"
redundant_pub_crate = "deny"
too_many_lines = "deny"
cast_possible_truncation = "deny"
cast_sign_loss = "deny"
```

`unsafe_code` is a rustc lint, so it belongs under `[lints.rust]`. The other entries are clippy lints and should not use the `clippy::` prefix in `Cargo.toml`. The `nursery` and `pedantic` groups need `priority = -1` so precise overrides can be added without ambiguity. Do not enable the full `clippy::restriction` group because it is self-contradictory.
