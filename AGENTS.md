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
- Do not delete task branches after completion, either locally or on GitHub. Branch history is part of the work record.
- Remove the task worktree after the task is complete with `git worktree remove <path>`.

## Pull Requests And Merge

- All changes reach `main` only through a pull request: push branch, open PR, wait for green CI, then merge.
- PR descriptions must be detailed and include what changed, why it changed, problems or noteworthy decisions during the work, and what remains for later.
- Split work into meaningful commits so the branch history shows the solution path.
- Merge to `main` with one squash commit and a detailed commit message. The detailed history remains in the branch.
- If the task is fully implemented and CI is green, push, PR creation, and merge do not need extra confirmation. Ask only when implementation is incomplete, there is doubt, or CI is red.
- After merge, update the main repository directory to fresh `main` with `git checkout main && git pull`.

## Gitignore Whitelist Model

- The root `.gitignore` must deny everything with `*` and then explicitly allow only required files and directories.
- Subprojects may add their own `.gitignore` files to allow their specific tracked files.
- Use only this model. Every new source file or tracked directory must be explicitly allowed, otherwise it may be silently omitted from commits and CI can fail because files are missing.
- Whenever adding a new kind of file, add the matching whitelist rule in the same change.

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
- `scripts/test-all.sh` is the default test entrypoint for humans, agents, and CI. Use it before pushing unless a task explicitly narrows validation.
- The Rust test runner is responsible for executing engine-level test cases and writing the final test report.
- Test reports belong under `reports/test-runs/`. Each report must be a separate tracked Markdown file with a sortable UTC timestamp suffix.
- Test reports must include a summary and a per-case table that records all passed, failed, and skipped cases.
- Official ECMAScript compatibility work should use Test262 as the external corpus and track pass or skip status by feature area.
- QuickJS should remain the reference implementation for differential behavior checks where the feature is implemented locally.
- Future benchmark reports should follow the same pattern: one command, Rust-owned execution, tracked report files, and clear comparison against QuickJS where possible.
- Benchmark cases must run sequentially, not in parallel, so measurements do not interfere with each other.
- Benchmark reports must separate local engine measurements from QuickJS measurements and mark unavailable reference runs as skipped with a concrete reason.

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
