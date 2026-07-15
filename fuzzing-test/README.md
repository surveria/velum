# Velum Fuzzilli Testing

This directory contains an opt-in local Fuzzilli target for security and
robustness testing of the Velum engine. It is intentionally excluded from the
ordinary CI workflows.

The upstream Fuzzilli repository does not ship a populated, engine-independent
corpus. A normal run starts from Fuzzilli's generative bootstrap, executes
programs against the instrumented Velum target, and retains only FuzzIL samples
that add stable Velum coverage. Later runs can resume from those local samples.

## What the integration tests

The target builds Velum with LLVM sanitizer coverage, AddressSanitizer, release
optimizations, debug assertions, checked arithmetic, and aborting Rust panics.
Fuzzilli then generates syntactically valid JavaScript, executes each program in
a fresh Velum runtime through its persistent REPRL process, and records new
coverage or failures. Useful findings include process crashes, sanitizer
failures, aborting assertions, hangs, and reproducible resource failures.

This initial lane does not compare Velum output with V8 or another JavaScript
engine. JavaScript syntax and runtime exceptions are ordinary rejected samples,
not engine crashes.

## Layout

- `FUZZILLI_REVISION` pins the upstream Fuzzilli commit.
- `patches/` adds the local Velum profile to the pinned checkout.
- `driver/` contains the small Rust utility used to start and summarize runs.
- `velum-reprl/` is a standalone Rust workspace containing the persistent
  target and sanitizer-coverage bridge.
- `scripts/` bootstraps and builds the local campaign.
- `fuzzilli/`, `runs/`, and build outputs are generated locally and ignored.

## Prerequisites

- Git and a C compiler;
- Swift for building Fuzzilli;
- the Rust nightly toolchain for sanitizer coverage.

On the current Ubuntu host, Swift can be installed with:

```bash
sudo apt install swiftlang
```

The launcher checks Git, Cargo, rustc, Swift, the nightly toolchain, and the C
compiler before cloning or building anything. A missing prerequisite stops the
run before a session directory is created and prints a concrete installation
command. The launcher never installs system packages itself.

## Bootstrap and build

```bash
./fuzzing-test/scripts/bootstrap-fuzzilli.sh
./fuzzing-test/scripts/build.sh
```

The bootstrap script clones the exact pinned Fuzzilli revision into the ignored
`fuzzing-test/fuzzilli/` directory and applies the tracked Velum profile patch.
The build script compiles Fuzzilli and an AddressSanitizer-instrumented Velum
REPRL target.

Set `VELUM_FUZZ_SANITIZER=none` only for instrumentation diagnostics where
AddressSanitizer itself prevents the target from starting.

## Run a local campaign

```bash
./fuzzing-test/run.sh
```

By default the launcher incrementally rebuilds the Rust driver, pinned Fuzzilli,
and the sanitizer-instrumented target against the current Velum checkout. Cargo
recompiles every changed engine dependency, so a run never intentionally reuses
a target from older source. Use `--skip-build` only as an explicit local
optimization when the existing binaries are known to match the checkout.

After the build, the utility starts one Fuzzilli worker and runs until Ctrl-C.
Fuzzilli receives the terminal interrupt, finishes its current operation, and
saves the corpus before exiting. Every session receives a new directory below
`fuzzing-test/runs/`.

Use a human-readable duration when the campaign should stop automatically. The
driver sends the same graceful interrupt when the time limit expires:

```bash
./fuzzing-test/run.sh --duration 30s
./fuzzing-test/run.sh --duration 2m
./fuzzing-test/run.sh --duration 1h
```

`--duration` and `--iterations` may be combined; the first reached limit stops
the campaign. Without either option, only Ctrl-C stops it.

For a bounded smoke run or a chosen output path:

```bash
./fuzzing-test/run.sh \
    --iterations 1000 --jobs 1 --output /tmp/velum-fuzz-smoke
```

Resume the coverage corpus from an earlier session without discarding its crash
history:

```bash
./fuzzing-test/run.sh --resume fuzzing-test/runs/session-123
```

Unique crash reproducers are saved as `crashes/*.js` together with their FuzzIL
form. Duplicate crashes are kept separately below `crashes/duplicates/`. Pass
`--diagnostics` only when needed: it also retains timeouts and ordinary rejected
programs and can use substantial disk space.

Fuzzilli stdout and stderr are captured in one detailed log. The launcher prints
the temporary live path before execution so a second terminal or an agent can
follow it with `tail -f`. On shutdown that file moves into the session directory
as a timestamped `fuzzilli-*.log`, and the final path is printed again. The
driver appends the same summary it prints to the terminal: a `tabled` table with
generated and valid cases, engine executions, corpus additions, crash and
timeout events, and new saved finding counts. At most the 10 latest new crash or
timeout JavaScript paths are printed below the table; every reproducer remains
available in the session directories even when the displayed list is truncated.
The detailed log additionally records the complete untruncated list of new
problem-file paths for later agent analysis.

Re-run a saved JavaScript case against the same instrumented target with:

```bash
./fuzzing-test/run.sh \
    --reproduce fuzzing-test/runs/session-123/crashes/program_0.js
```

Use `--skip-build` on either command when the Fuzzilli and target binaries are
already current. Session data and build outputs are deliberately untracked.

## CI policy

No GitHub Actions job invokes these scripts, downloads Fuzzilli, installs Swift,
or uploads fuzzing artifacts. The lane remains an explicit local or separately
scheduled security-testing activity until its runtime, storage, and triage
behavior are understood well enough to justify dedicated automation.
