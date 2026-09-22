use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, ensure};
use serde::Deserialize;
use serde_json::json;

use crate::{
    report_benchmark_methodology::{BenchmarkMode, ReferenceSource},
    report_metadata::RunMetadata,
    report_schema::{
        BenchmarkConfiguration, BenchmarkSet, BenchmarkStatus, EnvironmentInfo, FeatureSelection,
        Measurement, MeasurementAvailability, QualityStatus, ReportDocument, ReportMode,
        SuiteStatus,
    },
    report_schema_io::read_document,
};

use super::types::{
    BASELINE_PATH, Checksum, Experiment, PerformanceEvidence, Sampling, VerifiedReport, fnv_digest,
    hex_digest, label_path, read_json, sha256, write_json,
};

pub fn verify_report(
    run: &Path,
    variant: &str,
    label: &str,
    kind: &str,
    case: Option<&str>,
) -> Result<()> {
    let experiment = Experiment::load(run)?;
    let verified = collect_report(run, &experiment, variant, label, kind, case)?;
    write_json(&label_path(run, label, ".verified.json")?, &verified)
}

pub(super) fn collect_report(
    run: &Path,
    experiment: &Experiment,
    variant: &str,
    label: &str,
    kind: &str,
    case: Option<&str>,
) -> Result<VerifiedReport> {
    let binary = experiment.binary(variant)?;
    let (path, affinity, performance) = match kind {
        "performance" => {
            let id = case.context("performance verification requires an exact workload ID")?;
            ensure!(
                (variant == "instrumented") == id.starts_with("representative_"),
                "training and holdout variants must remain disjoint"
            );
            let path = label_path(run, label, "-component.yaml")?;
            let report = read_document(&path)?;
            let affinity = check_identity(experiment, &report.metadata, &report.environment, true)?;
            let performance = check_performance(experiment, &report, &path, id)?;
            (path, affinity, Some(performance))
        }
        "memory" => {
            ensure!(
                case.is_none() && variant != "instrumented",
                "memory evaluation cannot train profiles or select a case"
            );
            let path = label_path(run, label, ".json")?;
            let report: MemoryReport = read_json(&path)?;
            let affinity = check_identity(experiment, &report.metadata, &report.environment, true)?;
            check_memory(&report, &binary.path)?;
            (path, affinity, None)
        }
        _ => anyhow::bail!("unknown PGO report kind {kind}"),
    };
    let mut sources = experiment.cases.clone();
    sources.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(VerifiedReport {
        schema_version: 1,
        commit: experiment.commit.clone(),
        tree: experiment.tree.clone(),
        host: experiment.host.clone(),
        preset: experiment.preset.clone(),
        rounds: experiment.rounds,
        requested_cpu: experiment.cpu.clone(),
        cpu_affinity: affinity,
        source_root: experiment.source.clone(),
        sources,
        source_manifest_sha256: sha256(&run.join("source-files.sha256"))?,
        variant: variant.to_owned(),
        binary_path: binary.path.clone(),
        binary_sha256: binary.sha256.clone(),
        report_sha256: sha256(&path)?,
        report_path: path,
        kind: kind.to_owned(),
        expected_case: case.map(str::to_owned),
        performance,
    })
}

fn check_identity(
    experiment: &Experiment,
    metadata: &RunMetadata,
    environment: &EnvironmentInfo,
    timed: bool,
) -> Result<String> {
    ensure!(
        [
            &metadata.commit,
            &metadata.engine_commit,
            &metadata.runner_commit
        ]
        .into_iter()
        .all(|commit| *commit == experiment.commit)
            && metadata.tree == experiment.tree,
        "report source or build identity mismatch"
    );
    ensure!(
        environment.operating_system == "linux"
            && environment.architecture == "x86_64"
            && environment.build_profile == "release",
        "report host or build profile mismatch"
    );
    let affinity = environment
        .cpu_affinity
        .as_ref()
        .filter(|value| !value.is_empty())
        .context("report is missing CPU affinity")?;
    ensure!(
        !timed || experiment.cpu == "inherit" || *affinity == experiment.cpu,
        "report CPU affinity differs from requested CPU"
    );
    Ok(affinity.clone())
}

fn check_performance(
    experiment: &Experiment,
    report: &ReportDocument,
    path: &Path,
    id: &str,
) -> Result<PerformanceEvidence> {
    let source = experiment.case(id)?;
    check_components(experiment, report)?;
    let config = &report.configuration;
    ensure!(
        config.report_mode == ReportMode::Performance
            && config.benchmark_filter.as_deref() == Some(id)
            && config.benchmark_set == BenchmarkSet::Full
            && config.jetstream == FeatureSelection::Disabled,
        "unexpected performance mode, cohort, or filter"
    );
    let settings = sampling(&config.benchmark);
    let expected =
        Sampling::for_case(id.ends_with("_json_ingestion") || id.ends_with("_tree_allocation"));
    ensure!(
        settings == expected && config.benchmark.reference_quickjs_compiled,
        "frozen PGO measurement configuration changed"
    );
    let counts = report.benchmarks.counts;
    ensure!(
        counts.measured == 1
            && counts.in_process_measured == 1
            && counts.failed == 0
            && counts.invalid == 0
            && counts.skipped_reference == 0,
        "expected exactly one valid measured workload with a reference"
    );
    ensure!(
        report.benchmarks.rows.len() == 1,
        "workload report must contain exactly one row"
    );
    let row = report
        .benchmarks
        .rows
        .first()
        .context("workload report has no row")?;
    ensure!(
        row.id == id && row.source == source.source,
        "workload ID or source path mismatch"
    );
    ensure!(
        matches!(
            row.status,
            BenchmarkStatus::Measured
                | BenchmarkStatus::WithinBudget
                | BenchmarkStatus::TrackedException
        ) && row.quality == QualityStatus::Valid,
        "workload failed, skipped, or had invalid quality"
    );
    let methodology = row
        .methodology
        .as_ref()
        .context("workload methodology is missing")?;
    ensure!(
        methodology.mode == Some(BenchmarkMode::PreparedExecution)
            && methodology.reference_source == Some(ReferenceSource::QuickjsLive),
        "workload was not prepared execution with a live reference"
    );
    let checksum = strict_checksum(path)?;
    let (engine_ns, engine_cv_permille) = measurement(&row.engine)?;
    let (reference_ns, reference_cv_permille) = measurement(&row.reference)?;
    Ok(PerformanceEvidence {
        checksum,
        settings,
        engine_ns,
        reference_ns,
        engine_cv_permille,
        reference_cv_permille,
    })
}

const fn sampling(config: &BenchmarkConfiguration) -> Sampling {
    Sampling {
        warmup_duration_ns: config.warmup_duration_ns,
        minimum_sample_duration_ns: config.minimum_sample_duration_ns,
        samples: config.samples,
        minimum_operation_duration_ns: config.minimum_operation_duration_ns,
        maximum_cv_permille: config.maximum_cv_permille,
        attempts: config.attempts,
        maximum_operation_duration_ns: config.maximum_operation_duration_ns,
        maximum_total_duration_ns: config.maximum_total_duration_ns,
    }
}

fn strict_checksum(path: &Path) -> Result<Checksum> {
    // Read the raw object too: the general report decoder intentionally allows
    // future fields, while this frozen protocol requires exact typed checksums.
    let value: serde_json::Value = serde_yaml_ng::from_reader(BufReader::new(File::open(path)?))
        .context("failed to read raw workload checksum")?;
    let raw = value
        .pointer("/benchmarks/rows/0/methodology/checksum")
        .context("missing workload checksum")?;
    let checksum: Checksum =
        serde_json::from_value(raw.clone()).context("invalid typed workload checksum")?;
    checksum.validate()?;
    Ok(checksum)
}

fn measurement(value: &Measurement) -> Result<(u64, u32)> {
    ensure!(
        value.availability == MeasurementAvailability::Measured,
        "missing benchmark measurement"
    );
    let median = value
        .median_duration_ns
        .context("missing measurement median")?;
    let variation = value
        .coefficient_variation_permille
        .context("missing measurement variation")?;
    ensure!(
        median >= 1_000_000 && variation <= 100,
        "measurement failed the 1 ms or 10 percent quality gate"
    );
    Ok((median, variation))
}

#[derive(Deserialize)]
struct MemoryReport {
    schema_version: u32,
    artifact_kind: String,
    metadata: RunMetadata,
    environment: EnvironmentInfo,
    executable_path: PathBuf,
    executable_digest: String,
    configuration: MemoryConfiguration,
    expected_worker_count: usize,
    campaign_complete: bool,
    runs: Vec<MemoryWorker>,
}

#[derive(Deserialize)]
struct MemoryConfiguration {
    repetitions: u32,
    quickjs_compiled: bool,
    scenarios: Vec<MemoryScenario>,
}

#[derive(Deserialize)]
struct MemoryScenario {
    id: String,
    kind: String,
    vm_count: u32,
    nodes_per_vm: u32,
    bytes_per_node: u32,
    rounds: u32,
}

#[derive(Deserialize)]
struct MemoryWorker {
    scenario_id: String,
    engine: String,
    repetition: u32,
    outcome: String,
}

fn check_memory(report: &MemoryReport, executable: &Path) -> Result<()> {
    const SCENARIOS: [(&str, &str, u32, u32, u32, u32); 6] = [
        ("hello-world", "hello_world", 1, 0, 0, 1),
        ("retained-graph", "retained_graph", 1, 1_024, 256, 1),
        ("cyclic-churn", "cyclic_churn", 1, 1_024, 256, 3),
        ("independent-vms-1", "independent_vms", 1, 64, 256, 1),
        ("independent-vms-10", "independent_vms", 10, 64, 256, 1),
        ("independent-vms-50", "independent_vms", 50, 64, 256, 1),
    ];
    ensure!(
        report.schema_version == 1
            && report.artifact_kind == "local_process_isolated_memory_campaign",
        "unsupported memory report schema or kind"
    );
    ensure!(
        report.campaign_complete && report.expected_worker_count == 36 && report.runs.len() == 36,
        "incomplete memory campaign"
    );
    ensure!(
        report.executable_path == executable && report.executable_digest == fnv_digest(executable)?,
        "memory executable identity mismatch"
    );
    ensure!(
        report.configuration.repetitions == 3 && report.configuration.quickjs_compiled,
        "memory configuration mismatch"
    );
    let scenarios: BTreeSet<_> = report
        .configuration
        .scenarios
        .iter()
        .map(|case| {
            (
                case.id.as_str(),
                case.kind.as_str(),
                case.vm_count,
                case.nodes_per_vm,
                case.bytes_per_node,
                case.rounds,
            )
        })
        .collect();
    ensure!(
        report.configuration.scenarios.len() == SCENARIOS.len()
            && scenarios == BTreeSet::from(SCENARIOS),
        "memory scenario parameters changed"
    );
    let mut expected = BTreeSet::new();
    for (id, _, _, _, _, _) in SCENARIOS {
        for engine in ["velum", "quickjs"] {
            for repetition in 0..3 {
                ensure!(
                    expected.insert((id, engine, repetition)),
                    "duplicate expected memory worker"
                );
            }
        }
    }
    let actual: BTreeSet<_> = report
        .runs
        .iter()
        .map(|row| {
            (
                row.scenario_id.as_str(),
                row.engine.as_str(),
                row.repetition,
            )
        })
        .collect();
    ensure!(
        actual == expected && report.runs.iter().all(|row| row.outcome == "passed"),
        "memory worker failed, skipped, duplicated, or is missing"
    );
    Ok(())
}

pub fn verify_correctness(run: &Path, report_path: &Path, output: &Path) -> Result<()> {
    let experiment = Experiment::load(run)?;
    let binary = experiment.binary("pgo")?;
    let report = read_document(report_path)?;
    check_identity(&experiment, &report.metadata, &report.environment, false)?;
    check_components(&experiment, &report)?;
    crate::report_composition::validate_canonical_correctness(&report)?;
    ensure!(
        report.configuration.report_mode == ReportMode::Correctness
            && report.configuration.benchmark_filter.is_none()
            && report.configuration.benchmark.reference_quickjs_compiled,
        "PGO correctness must be unfiltered, benchmark-free, and include the compiled reference"
    );
    for suite in &report.suites {
        let summary = &suite.summary;
        ensure!(
            summary.status == SuiteStatus::Passed
                && summary.counts.passed == summary.counts.total
                && summary.counts.failed == 0
                && summary.counts.skipped == 0,
            "PGO adoption requires every case passed in {}",
            summary.name
        );
    }
    ensure!(
        crate::build_info::runner_build_info().commit_sha == experiment.commit,
        "correctness verifier must be the frozen runner build"
    );
    for (name, total) in [
        ("Engine fixtures", crate::cases::engine_cases().len()),
        ("Test262 active subset", crate::cases::test262_cases().len()),
        (
            "QuickJS differential",
            crate::cases::quickjs_differential_cases().len(),
        ),
    ] {
        ensure!(
            suite_passes(&report, name)? == u64::try_from(total)?,
            "correctness fixture coverage differs from frozen runner registry: {name}"
        );
    }
    let baseline_path = experiment.source.join(BASELINE_PATH);
    let candidate_path = output
        .parent()
        .context("correctness output needs a parent directory")?
        .join("pass-candidate.txt");
    let baseline = pass_list(&baseline_path)?;
    let candidate = pass_list(&candidate_path)?;
    ensure!(
        baseline.0 == candidate.0 && baseline.1.is_subset(&candidate.1),
        "Test262 candidate lost baseline passes or changed corpus pins"
    );
    let passes = u64::try_from(candidate.1.len())?;
    ensure!(
        passes == suite_passes(&report, "Test262 full corpus")?
            && passes == suite_passes(&report, "Test262 expected-pass baseline")?,
        "Test262 pass candidate disagrees with complete-corpus report"
    );
    let paths: BTreeSet<_> = candidate
        .1
        .iter()
        .map(|id| id.split_once('#').map_or(id.as_str(), |(path, _)| path))
        .collect();
    ensure!(
        suite_passes(&report, "Test262 file conformance")? == u64::try_from(paths.len())?,
        "Test262 file conformance count disagrees with pass candidate"
    );
    write_json(
        output,
        &json!({
            "schema_version": 1, "status": "passed", "commit": experiment.commit, "tree": experiment.tree,
            "preset": experiment.preset, "binary_path": binary.path, "binary_sha256": binary.sha256,
            "report_path": report_path, "report_sha256": sha256(report_path)?,
            "baseline_sha256": sha256(&baseline_path)?, "candidate_sha256": sha256(&candidate_path)?,
            "source_manifest_sha256": sha256(&run.join("source-files.sha256"))?,
            "test262_headers": baseline.0, "suites": report.suites,
            "provenance_boundary": "Execution identity and actual corpus pins additionally require the saved shell invocation and before/after hash receipts."
        }),
    )
}

fn check_components(experiment: &Experiment, report: &ReportDocument) -> Result<()> {
    ensure!(
        report
            .components
            .iter()
            .all(|component| component.commit == experiment.commit
                && component.tree == experiment.tree),
        "report component source identity mismatch"
    );
    Ok(())
}

fn suite_passes(report: &ReportDocument, name: &str) -> Result<u64> {
    report
        .suites
        .iter()
        .find(|suite| suite.summary.name == name)
        .map(|suite| suite.summary.counts.passed)
        .with_context(|| format!("missing correctness suite {name}"))
}

fn pass_list(path: &Path) -> Result<(Vec<String>, BTreeSet<String>)> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut lines = text.lines();
    ensure!(
        lines.next() == Some("# velum-test262-pass-baseline-v2"),
        "unsupported Test262 pass list"
    );
    let commit = lines.next().context("missing Test262 pin")?;
    let patches = lines.next().context("missing Test262 patch list")?;
    ensure!(
        commit
            .strip_prefix("# test262_commit=")
            .is_some_and(|value| hex_digest(value, 40)),
        "invalid Test262 commit pin"
    );
    ensure!(
        patches
            .strip_prefix("# test262_patches=")
            .is_some_and(|value| !value.is_empty()),
        "invalid Test262 patch pin"
    );
    let mut ids = BTreeSet::new();
    let mut previous = None;
    for line in lines.filter(|line| !line.is_empty() && !line.starts_with('#')) {
        ensure!(
            line == line.trim() && previous.is_none_or(|value| value < line),
            "Test262 pass list is not sorted and unique"
        );
        ensure!(ids.insert(line.to_owned()), "duplicate Test262 pass ID");
        previous = Some(line);
    }
    ensure!(!ids.is_empty(), "empty Test262 pass list");
    Ok((vec![commit.to_owned(), patches.to_owned()], ids))
}
