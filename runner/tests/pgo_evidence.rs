#![cfg(target_os = "linux")]

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};

#[path = "support/pgo_memory_fixture.rs"]
mod memory_fixture;

const SUFFIXES: [&str; 6] = [
    "object_transform",
    "method_dispatch",
    "json_ingestion",
    "string_processing",
    "collection_index",
    "tree_allocation",
];
const CASE: &str = "holdout_object_transform";
const COMMIT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const TREE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HEADER: &str = "step\texit_code\twall_seconds\tcommand\tstdout\tstderr\ttime\n";

struct Fixture {
    run: PathBuf,
    manifest: Value,
}

impl Fixture {
    fn new() -> Result<Self> {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let run =
            std::env::temp_dir().join(format!("velum-pgo-evidence-{}-{nonce}", std::process::id()));
        fs::create_dir(&run)?;
        for name in ["source/tests/corpora/benchmarks/prepared", "bin", "reports"] {
            fs::create_dir_all(run.join(name))?;
        }
        let run = run.canonicalize()?;
        let mut cases = Vec::new();
        for cohort in ["representative", "holdout"] {
            for suffix in SUFFIXES {
                let id = format!("{cohort}_{suffix}");
                let source = format!("tests/corpora/benchmarks/prepared/{id}.js");
                let path = run.join("source").join(&source);
                fs::write(&path, "function run() { return 42; }\n")?;
                cases.push(json!({"id": id, "source": source, "sha256": sha(&path)?}));
            }
        }
        let mut binaries = serde_json::Map::new();
        for variant in ["ordinary", "instrumented", "pgo"] {
            let path = run.join("bin").join(variant);
            fs::write(&path, variant)?;
            ensure!(binaries.insert(variant.to_owned(), json!({"path": path, "sha256": sha(&path)?, "bytes": fs::metadata(&path)?.len()})).is_none());
        }
        fs::write(
            run.join("source-files.sha256"),
            "synthetic source inventory\n",
        )?;
        let profile = run.join("merged.profdata");
        fs::write(&profile, "synthetic merged profile")?;
        fs::write(
            run.join("profile.sha256"),
            format!("{}  {}\n", sha(&profile)?, profile.display()),
        )?;
        let manifest = json!({
            "schema_version": 1, "protocol_version": "2-frozen-before-pgo-training",
            "commit": COMMIT, "tree": TREE, "host": "x86_64-unknown-linux-gnu",
            "cpu": "0", "rounds": 2, "preset": "release", "source": run.join("source"),
            "cases": cases, "binaries": binaries,
        });
        write(&run.join("experiment.json"), &manifest)?;
        Ok(Self { run, manifest })
    }

    fn report(&self, label: &str, id: &str, pgo: bool) -> Result<Value> {
        let long = id.ends_with("_json_ingestion") || id.ends_with("_tree_allocation");
        let median = if pgo { 3_000_000 } else { 6_000_000 };
        let value = json!({
            "schema_version": 1, "detail_level": "full", "metadata": metadata(),
            "environment": environment(), "duration_ns": 1,
            "configuration": {
                "report_mode": "performance", "jetstream": "disabled",
                "quickjs_differential": "not_configured", "test262": "not_configured",
                "test262_mode": "manifest", "test262_path_filters": [], "test262_flag_filters": [],
                "benchmark_set": "full", "benchmark_filter": id, "quickjs_baseline": "refresh",
                "benchmark": {
                    "reference_quickjs_compiled": true, "warmup_duration_ns": 150_000_000,
                    "minimum_sample_duration_ns": if long { 15_000_000_000_u64 } else { 5_000_000_000_u64 },
                    "samples": if long { 3 } else { 5 }, "minimum_operation_duration_ns": 1_000_000,
                    "maximum_cv_permille": 100, "attempts": 3, "maximum_operation_duration_ns": 2_000_000_000,
                    "maximum_total_duration_ns": if long { 60_000_000_000_u64 } else { 30_000_000_000_u64 },
                }
            },
            "components": [{"mode": "performance", "timestamp": "fixture", "commit": COMMIT,
                "tree": TREE, "run_id": "", "duration_ns": 1}], "suites": [],
            "benchmarks": {
                "name": "Benchmarks", "duration_ns": 1,
                "counts": {"measured": 1, "in_process_measured": 1, "failed": 0, "invalid": 0,
                    "skipped_reference": 0, "over_latency_budget": 0, "over_memory_budget": 0},
                "rows": [{"id": id, "status": "measured", "source": format!("tests/corpora/benchmarks/prepared/{id}.js"),
                    "iterations": 1, "case_duration_ns": 1,
                    "engine": measurement(median), "reference": measurement(2_000_000),
                    "latency_ratio_centi_units": 100, "latency_budget": "within", "quality": "valid",
                    "methodology": {"mode": "prepared_execution", "lifecycle": null,
                        "checksum": {"kind": "number", "bits": 42}, "reference_source": "quickjs_live"},
                    "count_contribution": {"measured": "counted", "in_process_measured": "counted",
                        "failed": "not_counted", "invalid": "not_counted", "skipped_reference": "not_counted",
                        "over_latency_budget": "not_counted", "over_memory_budget": "not_counted"},
                    "detail": "synthetic verification fixture"}]
            }
        });
        write(&self.performance_path(label), &value)?;
        Ok(value)
    }

    fn performance_path(&self, label: &str) -> PathBuf {
        self.run
            .join("reports")
            .join(format!("{label}-component.yaml"))
    }

    fn verify(&self, variant: &str, label: &str, kind: &str, case: Option<&str>) -> Result<Output> {
        let mut arguments = vec![
            "--pgo-report-check".to_owned(),
            self.run.display().to_string(),
            variant.to_owned(),
            label.to_owned(),
            kind.to_owned(),
        ];
        if let Some(id) = case {
            arguments.push(id.to_owned());
        }
        invoke(&arguments)
    }

    fn memory(&self, label: &str, variant: &str) -> Result<Value> {
        let value = memory_fixture::report(
            &self.run.join("bin").join(variant),
            &metadata(),
            &environment(),
        )?;
        write(
            &self.run.join("reports").join(format!("{label}.json")),
            &value,
        )?;
        Ok(value)
    }

    fn campaign(&self) -> Result<()> {
        let mut steps = HEADER.to_owned();
        for name in ["source-export", "source-extract"] {
            step(&mut steps, name)?;
        }
        for variant in ["ordinary", "instrumented"] {
            build_steps(&mut steps, variant)?;
        }
        for suffix in SUFFIXES {
            let id = format!("representative_{suffix}");
            let label = format!("training-{id}");
            self.report(&label, &id, false)?;
            success(&self.verify("instrumented", &label, "performance", Some(&id))?)?;
            lane_steps(&mut steps, &label)?;
        }
        for name in ["profile-merge", "profile-show", "profile-verify"] {
            step(&mut steps, name)?;
        }
        build_steps(&mut steps, "pgo")?;
        for name in ["pgo-profile-integrity", "pgo-diagnostics"] {
            step(&mut steps, name)?;
        }
        for round in 1..=2 {
            let variants = if round == 1 {
                ["ordinary", "pgo"]
            } else {
                ["pgo", "ordinary"]
            };
            for variant in variants {
                for suffix in SUFFIXES {
                    let id = format!("holdout_{suffix}");
                    let label = format!("round-{round}-{variant}-{id}");
                    self.report(&label, &id, variant == "pgo")?;
                    success(&self.verify(variant, &label, "performance", Some(&id))?)?;
                    lane_steps(&mut steps, &label)?;
                }
                let label = format!("round-{round}-{variant}-memory");
                self.memory(&label, variant)?;
                success(&self.verify(variant, &label, "memory", None)?)?;
                lane_steps(&mut steps, &label)?;
            }
        }
        for name in [
            "final-source",
            "final-layout",
            "final-tools",
            "final-profile-integrity",
        ] {
            step(&mut steps, name)?;
        }
        fs::write(self.run.join("steps.tsv"), steps)?;
        Ok(())
    }

    fn summarize(&self) -> Result<Output> {
        invoke(&["--pgo-summary".to_owned(), self.run.display().to_string()])
    }
}

fn metadata() -> Value {
    json!({"timestamp": "fixture", "commit": COMMIT, "tree": TREE, "event": "local",
        "run_id": "", "run_attempt": "", "repository": "fixture", "workflow": "", "pull_request": "",
        "task": "fixture", "engine_version": "fixture", "engine_commit": COMMIT,
        "runner_version": "fixture", "runner_commit": COMMIT})
}

fn environment() -> Value {
    json!({"operating_system": "linux", "architecture": "x86_64", "available_parallelism": 1,
        "build_profile": "release", "kernel_release": null, "cpu_model": null,
        "cpu_affinity": "0", "scaling_governor": null})
}

fn measurement(ns: u64) -> Value {
    json!({"availability": "measured", "wall_duration_ns": ns, "median_duration_ns": ns,
        "coefficient_variation_permille": 20})
}

fn sha(path: &Path) -> Result<String> {
    let output = Command::new("sha256sum").arg("--").arg(path).output()?;
    ensure!(output.status.success(), "fixture SHA-256 failed");
    Ok(String::from_utf8(output.stdout)?
        .split_whitespace()
        .next()
        .context("fixture SHA-256 missing")?
        .to_owned())
}

fn write(path: &Path, value: &Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)
        .context("failed to write synthetic PGO evidence")
}

fn set(value: &mut Value, pointer: &str, replacement: Value) -> Result<()> {
    *value
        .pointer_mut(pointer)
        .with_context(|| format!("fixture field missing: {pointer}"))? = replacement;
    Ok(())
}

fn invoke(arguments: &[String]) -> Result<Output> {
    Command::new("timeout")
        .args([
            "--kill-after=1s",
            "20s",
            env!("CARGO_BIN_EXE_velum-test-runner"),
        ])
        .args(arguments)
        .output()
        .context("failed to run bounded PGO verifier")
}

fn success(output: &Output) -> Result<()> {
    ensure!(
        output.status.success(),
        "PGO fixture rejected: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn failure(output: &Output) -> Result<()> {
    ensure!(
        !output.status.success()
            && output.status.code() != Some(124)
            && output.status.code() != Some(137),
        "PGO verifier accepted invalid evidence or exceeded its deadline"
    );
    Ok(())
}

fn step(text: &mut String, name: &str) -> Result<()> {
    writeln!(
        text,
        "{name}\t0\t1\tsteps/{name}.command\tsteps/{name}.log\tsteps/{name}.stderr\tsteps/{name}.time"
    )?;
    Ok(())
}

fn build_steps(text: &mut String, variant: &str) -> Result<()> {
    for suffix in [
        "source", "layout", "tools", "clean", "build", "sections", "manifest",
    ] {
        step(text, &format!("{variant}-{suffix}"))?;
    }
    Ok(())
}

fn lane_steps(text: &mut String, label: &str) -> Result<()> {
    for name in [
        format!("{label}-binary"),
        label.to_owned(),
        format!("{label}-verify"),
    ] {
        step(text, &name)?;
    }
    Ok(())
}

fn with_fixture(test: impl FnOnce(&mut Fixture) -> Result<()>) -> Result<()> {
    let mut fixture = Fixture::new()?;
    let result = test(&mut fixture);
    fs::remove_dir_all(&fixture.run).context("failed to clean synthetic PGO fixture")?;
    result
}

#[test]
fn typed_checksums_preserve_all_supported_representations() -> Result<()> {
    with_fixture(|fixture| {
        let original = fixture.report("one", CASE, false)?;
        for checksum in [
            json!({"kind": "number", "bits": u64::MAX}),
            json!({"kind": "boolean", "value": false}),
            json!({"kind": "string", "value": "typed-result"}),
        ] {
            let mut report = original.clone();
            set(
                &mut report,
                "/benchmarks/rows/0/methodology/checksum",
                checksum.clone(),
            )?;
            write(&fixture.performance_path("one"), &report)?;
            success(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
            let sidecar: Value =
                serde_json::from_slice(&fs::read(fixture.run.join("reports/one.verified.json"))?)?;
            ensure!(sidecar.pointer("/performance/checksum") == Some(&checksum));
        }
        Ok(())
    })
}

#[test]
fn malformed_or_untyped_checksums_fail_closed() -> Result<()> {
    with_fixture(|fixture| {
        let original = fixture.report("one", CASE, false)?;
        for checksum in [
            json!({"kind": "number", "bits": -1}),
            json!({"kind": "number", "bits": 1.0}),
            json!({"kind": "boolean", "value": 1}),
            json!({"kind": "string", "value": ""}),
            json!({"kind": "number", "bits": 1, "extra": 2}),
            json!({"kind": "null"}),
            Value::Null,
        ] {
            let mut report = original.clone();
            set(
                &mut report,
                "/benchmarks/rows/0/methodology/checksum",
                checksum,
            )?;
            write(&fixture.performance_path("one"), &report)?;
            failure(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
        }
        Ok(())
    })
}

#[test]
fn identity_sampling_membership_and_quality_are_not_optional() -> Result<()> {
    with_fixture(|fixture| {
        let original = fixture.report("one", CASE, false)?;
        success(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
        for (pointer, replacement) in [
            ("/metadata/engine_commit", json!(TREE)),
            ("/environment/cpu_affinity", json!("99")),
            ("/configuration/benchmark/samples", json!(1)),
            ("/benchmarks/rows/0/id", json!("other")),
            ("/benchmarks/rows/0/source", json!("other.js")),
            (
                "/benchmarks/rows/0/engine/median_duration_ns",
                json!(999_999),
            ),
            (
                "/benchmarks/rows/0/reference/coefficient_variation_permille",
                json!(101),
            ),
            (
                "/benchmarks/rows/0/methodology/reference_source",
                json!("quickjs_baseline"),
            ),
        ] {
            let mut report = original.clone();
            set(&mut report, pointer, replacement)?;
            write(&fixture.performance_path("one"), &report)?;
            failure(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
        }
        Ok(())
    })
}

#[test]
fn hashes_manifest_labels_and_training_boundaries_are_enforced() -> Result<()> {
    with_fixture(|fixture| {
        fixture.report("one", CASE, false)?;
        for (variant, label, id) in [
            ("instrumented", "one", CASE),
            ("ordinary", "../escape", CASE),
            ("other", "one", CASE),
        ] {
            failure(&fixture.verify(variant, label, "performance", Some(id))?)?;
        }
        fs::write(fixture.run.join("bin/ordinary"), "changed")?;
        failure(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
        fs::write(fixture.run.join("bin/ordinary"), "ordinary")?;
        fs::write(
            fixture
                .run
                .join("source/tests/corpora/benchmarks/prepared/holdout_object_transform.js"),
            "changed",
        )?;
        failure(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
        Ok(())
    })
}

#[test]
fn frozen_protocol_and_preset_are_explicit() -> Result<()> {
    with_fixture(|fixture| {
        fixture.report("one", CASE, false)?;
        for (pointer, value) in [
            ("/protocol_version", json!("changed")),
            ("/preset", json!("unknown")),
            ("/rounds", json!(0)),
            ("/binaries/ordinary/bytes", json!(999)),
        ] {
            let mut manifest = fixture.manifest.clone();
            set(&mut manifest, pointer, value)?;
            write(&fixture.run.join("experiment.json"), &manifest)?;
            failure(&fixture.verify("ordinary", "one", "performance", Some(CASE))?)?;
        }
        Ok(())
    })
}

#[test]
fn memory_requires_exact_workers_configuration_and_executable() -> Result<()> {
    with_fixture(|fixture| {
        let original = fixture.memory("memory", "ordinary")?;
        success(&fixture.verify("ordinary", "memory", "memory", None)?)?;
        for (pointer, value) in [
            ("/runs/0/repetition", json!(1)),
            ("/runs/0/outcome", json!("skipped")),
            ("/configuration/scenarios/0/vm_count", json!(2)),
            ("/executable_digest", json!("changed")),
            ("/campaign_complete", json!(false)),
        ] {
            let mut report = original.clone();
            set(&mut report, pointer, value)?;
            write(&fixture.run.join("reports/memory.json"), &report)?;
            failure(&fixture.verify("ordinary", "memory", "memory", None)?)?;
        }
        Ok(())
    })
}

#[test]
fn summary_revalidates_artifacts_and_cross_round_checksums() -> Result<()> {
    with_fixture(|fixture| {
        fixture.campaign()?;
        success(&fixture.summarize()?)?;
        let summary: Value =
            serde_json::from_slice(&fs::read(fixture.run.join("holdout-comparison.json"))?)?;
        ensure!(summary.get("evidence_validated") == Some(&json!(true)));
        ensure!(
            summary
                .get("rows")
                .and_then(Value::as_array)
                .context("missing comparison rows")?
                .len()
                == 12
        );
        ensure!(
            summary
                .pointer("/rows/0/ordinary_over_pgo_speedup")
                .and_then(Value::as_f64)
                == Some(2.0)
        );
        let memory_label = "round-2-pgo-memory";
        let mut memory = fixture.memory(memory_label, "pgo")?;
        set(
            &mut memory,
            "/runs/0/phases/1/diagnostics/vms/0/runtime_steps",
            json!(99),
        )?;
        write(
            &fixture.run.join("reports/round-2-pgo-memory.json"),
            &memory,
        )?;
        success(&fixture.verify("pgo", memory_label, "memory", None)?)?;
        failure(&fixture.summarize()?)?;
        let invalidated: Value =
            serde_json::from_slice(&fs::read(fixture.run.join("holdout-comparison.json"))?)?;
        ensure!(invalidated.get("evidence_validated") == Some(&json!(false)));
        ensure!(invalidated.get("rows").is_none());
        ensure!(
            fs::read_to_string(fixture.run.join("holdout-comparison.tsv"))?
                .lines()
                .count()
                == 1
        );
        let drift: Value =
            serde_json::from_slice(&fs::read(fixture.run.join("memory-comparison.json"))?)?;
        ensure!(
            drift.get("equal") == Some(&json!(false))
                && drift.get("status") == Some(&json!("needs-review"))
        );
        fixture.memory(memory_label, "pgo")?;
        success(&fixture.verify("pgo", memory_label, "memory", None)?)?;
        let label = "round-2-pgo-holdout_object_transform";
        let mut report = fixture.report(label, CASE, true)?;
        set(
            &mut report,
            "/benchmarks/rows/0/methodology/checksum",
            json!({"kind": "number", "bits": 99}),
        )?;
        write(&fixture.performance_path(label), &report)?;
        failure(&fixture.summarize()?)?;
        success(&fixture.verify("pgo", label, "performance", Some(CASE))?)?;
        failure(&fixture.summarize()?)?;
        fixture.report(label, CASE, true)?;
        success(&fixture.verify("pgo", label, "performance", Some(CASE))?)?;
        fs::write(fixture.run.join("merged.profdata"), "changed")?;
        failure(&fixture.summarize()?)?;
        let unverified: Value =
            serde_json::from_slice(&fs::read(fixture.run.join("memory-comparison.json"))?)?;
        ensure!(unverified.get("status") == Some(&json!("unverified")));
        ensure!(unverified.get("equal").is_none());
        Ok(())
    })
}

#[test]
fn summary_rejects_incomplete_or_reordered_protocol() -> Result<()> {
    with_fixture(|fixture| {
        fixture.campaign()?;
        let path = fixture.run.join("steps.tsv");
        let original = fs::read_to_string(&path)?;
        let result_path = fixture.run.join("result.txt");
        fs::write(&result_path, "status=incomplete\nexit_code=143\n")?;
        failure(&fixture.summarize()?)?;
        fs::write(&result_path, "status=complete-needs-review\nexit_code=0\n")?;
        failure(&fixture.summarize()?)?;
        let mut completed = original.clone();
        step(&mut completed, "compare-holdouts")?;
        fs::write(&path, &completed)?;
        success(&fixture.summarize()?)?;
        fs::write(
            &result_path,
            "status=complete-needs-review\nstatus=incomplete\nexit_code=0\n",
        )?;
        failure(&fixture.summarize()?)?;
        fs::remove_file(result_path)?;
        failure(&fixture.summarize()?)?;
        for added in ["training-foo", "round-99-pgo-holdout_object_transform"] {
            let mut extra = original.clone();
            step(&mut extra, added)?;
            fs::write(&path, extra)?;
            failure(&fixture.summarize()?)?;
        }
        let missing_build = original
            .lines()
            .filter(|line| !line.starts_with("pgo-build\t"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, missing_build)?;
        failure(&fixture.summarize()?)?;
        let mut lines: Vec<_> = original.lines().collect();
        lines.reverse();
        fs::write(&path, lines.join("\n"))?;
        failure(&fixture.summarize()?)?;
        fs::write(path, original)?;
        fs::remove_file(fixture.run.join("reports/round-2-pgo-memory.verified.json"))?;
        failure(&fixture.summarize()?)?;
        Ok(())
    })
}

fn correctness_fixture(fixture: &Fixture) -> Result<Value> {
    let mut report = fixture.report("correctness-seed", CASE, false)?;
    for (pointer, value) in [
        ("/configuration/report_mode", json!("correctness")),
        ("/configuration/benchmark_filter", Value::Null),
        ("/configuration/quickjs_differential", json!("configured")),
        ("/configuration/test262", json!("configured")),
        ("/configuration/test262_mode", json!("full")),
        ("/components/0/mode", json!("correctness")),
        ("/benchmarks/rows", json!([])),
        ("/benchmarks/counts/measured", json!(0)),
        ("/benchmarks/counts/in_process_measured", json!(0)),
    ] {
        set(&mut report, pointer, value)?;
    }
    let mut suites = Vec::new();
    for (name, required) in [
        ("Engine fixtures", true),
        ("Test262 active subset", true),
        ("QuickJS differential", true),
        ("Test262 expected-pass baseline", true),
        ("Test262 file conformance", false),
        ("Test262 full corpus", false),
    ] {
        suites.push(json!({
            "name": name, "required": required, "status": "passed",
            "counts": {"total": 1, "executed": 1, "passed": 1, "failed": 0, "skipped": 0},
            "case_details": {"completeness": "partial", "recorded_rows": 0, "omitted_rows": 1},
            "duration_ns": 1, "skip_reasons": [], "feature_areas": [{"name": "fixture",
                "total": 1, "executed": 1, "passed": 1, "failed": 0, "skipped": 0,
                "manifest_enabled": 1, "top_skip_reason": ""}],
        }));
    }
    set(&mut report, "/suites", json!(suites))?;
    Ok(report)
}

fn correctness_output(fixture: &Fixture, report: &Value) -> Result<Output> {
    let path = fixture.run.join("reports/correctness-component.yaml");
    write(&path, report)?;
    invoke(&[
        "--pgo-correctness-check".to_owned(),
        fixture.run.display().to_string(),
        path.display().to_string(),
        fixture
            .run
            .join("reports/correctness-verified.json")
            .display()
            .to_string(),
    ])
}

#[test]
fn correctness_rejects_filters_missing_reference_and_benchmark_leakage() -> Result<()> {
    with_fixture(|fixture| {
        let original = correctness_fixture(fixture)?;
        for (pointer, value, diagnostic) in [
            (
                "/configuration/benchmark/reference_quickjs_compiled",
                json!(false),
                "include the compiled reference",
            ),
            (
                "/configuration/benchmark_filter",
                json!(CASE),
                "unfiltered, benchmark-free",
            ),
            (
                "/configuration/test262_path_filters",
                json!(["selected-only"]),
                "unfiltered full Test262",
            ),
            (
                "/configuration/jetstream",
                json!("enabled"),
                "JetStream cannot be enabled",
            ),
        ] {
            let mut report = original.clone();
            set(&mut report, pointer, value)?;
            let output = correctness_output(fixture, &report)?;
            failure(&output)?;
            ensure!(
                String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                "unexpected correctness rejection: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let seed = fixture.report("seed", CASE, false)?;
        let mut report = original;
        set(
            &mut report,
            "/benchmarks",
            seed.get("benchmarks")
                .context("fixture benchmarks missing")?
                .clone(),
        )?;
        let output = correctness_output(fixture, &report)?;
        failure(&output)?;
        ensure!(
            String::from_utf8_lossy(&output.stderr)
                .contains("benchmark rows are present in a correctness report")
        );
        Ok(())
    })
}
