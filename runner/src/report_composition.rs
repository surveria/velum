use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use crate::{
    report_benchmark_methodology::ReferenceSource,
    report_metadata::RunMetadata,
    report_schema::{
        BenchmarkSet, DetailLevel, EnvironmentInfo, FeatureSelection, InputAvailability,
        QuickjsBaselineMode, ReportDocument, ReportMode, RunConfiguration, SCHEMA_VERSION,
        SuiteReport, SuiteStatus, Test262Mode,
    },
};

const CANONICAL_CORRECTNESS_SUITES: [(&str, bool); 6] = [
    ("Engine fixtures", true),
    ("Test262 active subset", true),
    ("Test262 file conformance", false),
    ("Test262 full corpus", false),
    ("Test262 expected-pass baseline", true),
    ("QuickJS differential", true),
];
const CANONICAL_SENTINELS: [&str; 5] = [
    "sentinel_arithmetic",
    "sentinel_array_index",
    "sentinel_property_read",
    "sentinel_function_call",
    "sentinel_string_scan",
];

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportComponent {
    pub(crate) mode: ReportMode,
    pub(crate) timestamp: String,
    pub(crate) commit: String,
    pub(crate) tree: String,
    pub(crate) run_id: String,
    pub(crate) duration_ns: u64,
}

impl ReportComponent {
    pub fn capture(
        mode: ReportMode,
        metadata: &RunMetadata,
        _environment: &EnvironmentInfo,
        _configuration: &RunConfiguration,
        duration_ns: u64,
    ) -> Self {
        Self {
            mode,
            timestamp: metadata.timestamp.clone(),
            commit: metadata.commit.clone(),
            tree: metadata.tree.clone(),
            run_id: metadata.run_id.clone(),
            duration_ns,
        }
    }
}

pub fn compose(
    correctness: ReportDocument,
    performance: ReportDocument,
    expected_tree: &str,
) -> anyhow::Result<ReportDocument> {
    correctness.validate()?;
    performance.validate()?;
    ensure_component(&correctness, ReportMode::Correctness, expected_tree)?;
    ensure_component(&performance, ReportMode::Performance, expected_tree)?;
    validate_canonical_correctness(&correctness)?;
    validate_canonical_performance(&performance)?;

    let configuration = RunConfiguration {
        report_mode: ReportMode::Full,
        jetstream: performance.configuration.jetstream,
        quickjs_differential: correctness.configuration.quickjs_differential,
        test262: correctness.configuration.test262,
        test262_mode: correctness.configuration.test262_mode,
        test262_path_filters: correctness.configuration.test262_path_filters.clone(),
        test262_flag_filters: correctness.configuration.test262_flag_filters.clone(),
        benchmark_set: performance.configuration.benchmark_set,
        benchmark_filter: performance.configuration.benchmark_filter.clone(),
        quickjs_baseline: performance.configuration.quickjs_baseline,
        benchmark: performance.configuration.benchmark.clone(),
        suite_max_duration_ns: performance.configuration.suite_max_duration_ns,
    };
    let mut components = correctness.components;
    components.extend(performance.components);
    let report = ReportDocument {
        schema_version: SCHEMA_VERSION,
        detail_level: DetailLevel::Full,
        metadata: performance.metadata,
        environment: performance.environment,
        configuration,
        components,
        duration_ns: correctness
            .duration_ns
            .saturating_add(performance.duration_ns),
        suites: correctness.suites,
        benchmarks: performance.benchmarks,
        jetstream: performance.jetstream,
    };
    report.validate()?;
    Ok(report)
}

pub fn validate_output_path(report: &ReportDocument, report_path: &Path) -> anyhow::Result<()> {
    if report.metadata.timestamp.is_empty() {
        bail!("composed report metadata timestamp must not be empty");
    }
    let expected = format!("rsqjs-test-report-{}.md", report.metadata.timestamp);
    let actual = report_path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .context("composed report output file name must be valid UTF-8")?;
    if actual != expected {
        bail!("composed report output '{actual}' does not match metadata timestamp '{expected}'");
    }
    Ok(())
}

pub fn validate_components(
    metadata: &RunMetadata,
    configuration: &RunConfiguration,
    components: &[ReportComponent],
) -> anyhow::Result<()> {
    if components.is_empty() {
        bail!("report has no component provenance");
    }
    for component in components {
        if !metadata.tree.is_empty() && component.tree != metadata.tree {
            bail!("report component tree does not match the report tree");
        }
    }
    let full_count = component_count(components, ReportMode::Full);
    let correctness_count = component_count(components, ReportMode::Correctness);
    let performance_count = component_count(components, ReportMode::Performance);
    let jetstream_count = component_count(components, ReportMode::Jetstream);
    let valid = match configuration.report_mode {
        ReportMode::Full => {
            (components.len() == 1 && full_count == 1)
                || (components.len() == 2 && correctness_count == 1 && performance_count == 1)
        }
        ReportMode::Correctness => components.len() == 1 && correctness_count == 1,
        ReportMode::Performance => components.len() == 1 && performance_count == 1,
        ReportMode::Jetstream => components.len() == 1 && jetstream_count == 1,
    };
    if !valid {
        bail!("report component provenance does not cover its mode");
    }
    Ok(())
}

fn component_count(components: &[ReportComponent], mode: ReportMode) -> usize {
    components
        .iter()
        .filter(|component| component.mode == mode)
        .count()
}

fn ensure_component(
    report: &ReportDocument,
    expected_mode: ReportMode,
    expected_tree: &str,
) -> anyhow::Result<()> {
    if report.configuration.report_mode != expected_mode {
        bail!(
            "expected {expected_mode:?} report, got {:?}",
            report.configuration.report_mode
        );
    }
    if expected_tree.is_empty() {
        bail!("expected tree must not be empty");
    }
    if report.metadata.tree != expected_tree {
        bail!(
            "{expected_mode:?} report tree mismatch: '{}' != '{expected_tree}'",
            report.metadata.tree
        );
    }
    for component in &report.components {
        if component.tree != expected_tree {
            bail!(
                "{expected_mode:?} component tree mismatch: '{}' != '{expected_tree}'",
                component.tree
            );
        }
    }
    report
        .components
        .iter()
        .find(|component| component.mode == expected_mode)
        .context("report is missing its matching component provenance")?;
    Ok(())
}

fn validate_canonical_correctness(report: &ReportDocument) -> anyhow::Result<()> {
    let configuration = &report.configuration;
    if configuration.test262 != InputAvailability::Configured
        || configuration.quickjs_differential != InputAvailability::Configured
        || configuration.test262_mode != Test262Mode::Full
        || !configuration.test262_path_filters.is_empty()
        || !configuration.test262_flag_filters.is_empty()
    {
        bail!("canonical correctness requires unfiltered full Test262 and QuickJS inputs");
    }
    for (name, required) in CANONICAL_CORRECTNESS_SUITES {
        let suite = canonical_suite(&report.suites, name)?;
        if suite.summary.required != required || suite.summary.counts.total == 0 {
            bail!("canonical correctness suite '{name}' has incomplete identity or coverage");
        }
        if required
            && (suite.summary.status != SuiteStatus::Passed
                || suite.summary.counts.passed != suite.summary.counts.total
                || suite.summary.counts.failed > 0
                || suite.summary.counts.skipped > 0)
        {
            bail!("canonical correctness required suite '{name}' did not pass every case");
        }
    }
    let full_corpus = canonical_suite(&report.suites, "Test262 full corpus")?;
    if full_corpus.summary.feature_areas.is_empty()
        || full_corpus.summary.counts.failed > 0
            && full_corpus.summary.failure_diagnostics.is_none()
    {
        bail!("canonical correctness full Test262 aggregates are incomplete");
    }
    let actual_names = report
        .suites
        .iter()
        .map(|suite| suite.summary.name.as_str())
        .collect::<BTreeSet<_>>();
    let expected_names = CANONICAL_CORRECTNESS_SUITES
        .into_iter()
        .map(|(name, _required)| name)
        .collect::<BTreeSet<_>>();
    if report.suites.len() != CANONICAL_CORRECTNESS_SUITES.len() || actual_names != expected_names {
        bail!("canonical correctness suite identities are duplicated or unexpected");
    }
    Ok(())
}

fn canonical_suite<'report>(
    suites: &'report [SuiteReport],
    name: &str,
) -> anyhow::Result<&'report SuiteReport> {
    suites
        .iter()
        .find(|suite| suite.summary.name == name)
        .with_context(|| format!("canonical correctness is missing suite '{name}'"))
}

fn validate_canonical_performance(report: &ReportDocument) -> anyhow::Result<()> {
    let configuration = &report.configuration;
    if configuration.benchmark_set != BenchmarkSet::Sentinel
        || configuration.benchmark_filter.is_some()
        || configuration.quickjs_baseline != QuickjsBaselineMode::Require
        || configuration.jetstream != FeatureSelection::Disabled
        || configuration.benchmark.reference_quickjs_compiled
    {
        bail!("canonical performance requires the unfiltered baseline-only sentinel set");
    }
    if report.benchmarks.counts.failed > 0
        || report.benchmarks.counts.invalid > 0
        || report.benchmarks.counts.measured != 5
        || report.benchmarks.counts.in_process_measured != 5
        || report.benchmarks.counts.skipped_reference > 0
        || report.benchmarks.rows.len() != CANONICAL_SENTINELS.len()
    {
        bail!("canonical performance has missing, failed, or invalid sentinel rows");
    }
    let actual = report
        .benchmarks
        .rows
        .iter()
        .map(|row| row.id.as_str())
        .collect::<BTreeSet<_>>();
    let expected = CANONICAL_SENTINELS.into_iter().collect::<BTreeSet<_>>();
    if actual != expected {
        bail!("canonical performance sentinel identities do not match the required set");
    }
    for row in &report.benchmarks.rows {
        let reference_source = row
            .methodology
            .as_ref()
            .and_then(|methodology| methodology.reference_source);
        if reference_source != Some(ReferenceSource::QuickjsBaseline) {
            bail!(
                "canonical performance benchmark '{}' did not use the committed baseline",
                row.id
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CANONICAL_CORRECTNESS_SUITES, CANONICAL_SENTINELS, compose, validate_output_path};
    use crate::{
        report_composition::ReportComponent,
        report_schema::{BenchmarkSet, FeatureSelection, QuickjsBaselineMode, ReportMode},
        report_schema_io::MAX_CANONICAL_YAML_LINES,
        report_schema_tests::{diagnostic_document, sample_document},
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn combines_exact_tree_correctness_and_performance_documents() -> TestResult {
        let (correctness, performance) = component_documents()?;
        let report = compose(correctness, performance, "tree-1")?;
        if report.configuration.report_mode != ReportMode::Full
            || report.suites.len() != CANONICAL_CORRECTNESS_SUITES.len()
            || report.benchmarks.rows.len() != CANONICAL_SENTINELS.len()
            || report.components.len() != 2
            || report
                .benchmarks
                .rows
                .first()
                .and_then(|row| row.methodology.as_ref())
                .and_then(|methodology| methodology.lifecycle.as_ref())
                .is_none()
        {
            return Err("composed report did not retain both components".into());
        }
        Ok(())
    }

    #[test]
    fn rejects_component_from_another_tree() -> TestResult {
        let (correctness, mut performance) = component_documents()?;
        performance.metadata.tree = "tree-2".to_owned();
        let Some(component) = performance.components.first_mut() else {
            return Err("expected performance component".into());
        };
        component.tree = "tree-2".to_owned();
        if compose(correctness, performance, "tree-1").is_err() {
            return Ok(());
        }
        Err("composition accepted mismatched trees".into())
    }

    #[test]
    fn rejects_missing_or_duplicate_component_provenance() -> TestResult {
        let (mut correctness, performance) = component_documents()?;
        correctness.components.clear();
        if compose(correctness, performance, "tree-1").is_ok() {
            return Err("composition accepted missing correctness provenance".into());
        }

        let (correctness, mut performance) = component_documents()?;
        let Some(component) = performance.components.first().cloned() else {
            return Err("expected performance component".into());
        };
        performance.components.push(component);
        if compose(correctness, performance, "tree-1").is_err() {
            return Ok(());
        }
        Err("composition accepted duplicate performance provenance".into())
    }

    #[test]
    fn output_file_name_must_match_performance_timestamp() -> TestResult {
        let (correctness, mut performance) = component_documents()?;
        performance.metadata.timestamp = "20260710T010100Z".to_owned();
        let Some(component) = performance.components.first_mut() else {
            return Err("expected performance component".into());
        };
        component.timestamp = "20260710T010100Z".to_owned();
        let report = compose(correctness, performance, "tree-1")?;
        validate_output_path(
            &report,
            std::path::Path::new("rsqjs-test-report-20260710T010100Z.md"),
        )?;
        if validate_output_path(
            &report,
            std::path::Path::new("rsqjs-test-report-20260710T010200Z.md"),
        )
        .is_err()
        {
            return Ok(());
        }
        Err("composition accepted an output timestamp mismatch".into())
    }

    #[test]
    fn rejects_empty_filtered_or_partial_canonical_correctness() -> TestResult {
        let (mut correctness, performance) = component_documents()?;
        correctness.suites.clear();
        ensure_rejected(correctness, performance, "empty correctness suites")?;

        let (mut correctness, performance) = component_documents()?;
        correctness.configuration.test262_path_filters = vec!["built-ins/Array".to_owned()];
        ensure_rejected(correctness, performance, "filtered Test262 correctness")?;

        let (mut correctness, performance) = component_documents()?;
        let Some(required) = correctness
            .suites
            .iter_mut()
            .find(|suite| suite.summary.required)
        else {
            return Err("expected required correctness suite".into());
        };
        required.cases.clear();
        required.summary.status = crate::report_schema::SuiteStatus::Skipped;
        required.summary.counts = crate::report_schema::CaseCounts {
            total: 1,
            executed: 0,
            passed: 0,
            failed: 0,
            skipped: 1,
        };
        required.summary.case_details = crate::report_schema::CaseDetailCoverage {
            completeness: crate::report_schema::DetailCompleteness::Partial,
            recorded_rows: 0,
            omitted_rows: 1,
        };
        ensure_rejected(
            correctness,
            performance,
            "skipped required correctness suite",
        )
    }

    #[test]
    fn rejects_noncanonical_performance_selection_and_reference() -> TestResult {
        let (correctness, mut performance) = component_documents()?;
        performance.configuration.benchmark_set = BenchmarkSet::Full;
        ensure_rejected(correctness, performance, "full benchmark set")?;

        let (correctness, mut performance) = component_documents()?;
        let Some(row) = performance.benchmarks.rows.first_mut() else {
            return Err("expected sentinel benchmark".into());
        };
        let Some(methodology) = row.methodology.as_mut() else {
            return Err("expected benchmark methodology".into());
        };
        methodology.reference_source =
            Some(crate::report_benchmark_methodology::ReferenceSource::QuickjsLive);
        ensure_rejected(correctness, performance, "live QuickJS reference")?;

        let (correctness, mut performance) = component_documents()?;
        performance.benchmarks.rows.clear();
        performance.benchmarks.counts = crate::report_schema::BenchmarkCounts {
            measured: 0,
            in_process_measured: 0,
            failed: 0,
            invalid: 0,
            skipped_reference: 0,
            over_latency_budget: 0,
            over_memory_budget: 0,
        };
        ensure_rejected(correctness, performance, "empty sentinel set")
    }

    #[test]
    fn composed_ordinary_yaml_stays_within_the_line_contract() -> TestResult {
        const REALISTIC_COMPOSE_LINE_TARGET: usize = 900;
        let (mut correctness, performance) = component_documents()?;
        let diagnostic_report = diagnostic_document()?;
        let Some(diagnostic_suite) = diagnostic_report.suites.first().cloned() else {
            return Err("expected diagnostic suite".into());
        };
        let Some(full_suite) = correctness
            .suites
            .iter_mut()
            .find(|suite| suite.summary.name == "Test262 full corpus")
        else {
            return Err("expected full Test262 suite".into());
        };
        let full_name = full_suite.summary.name.clone();
        let required = full_suite.summary.required;
        *full_suite = diagnostic_suite;
        full_suite.summary.name = full_name;
        full_suite.summary.required = required;
        let Some(file_suite) = correctness
            .suites
            .iter_mut()
            .find(|suite| suite.summary.name == "Test262 file conformance")
        else {
            return Err("expected file-level Test262 suite".into());
        };
        let file_name = file_suite.summary.name.clone();
        let file_required = file_suite.summary.required;
        *file_suite = diagnostic_report
            .suites
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("expected diagnostic file suite"))?;
        file_suite.summary.name = file_name;
        file_suite.summary.required = file_required;
        let correctness = correctness.bounded_component()?;
        let performance = performance.bounded_component()?;
        let report = compose(correctness, performance, "tree-1")?;
        let bounded = report.bounded_component()?;
        let component_yaml = serde_yaml_ng::to_string(&bounded)?;
        let summary_yaml = serde_yaml_ng::to_string(&bounded.summary())?;
        if component_yaml.lines().count() <= REALISTIC_COMPOSE_LINE_TARGET
            && summary_yaml.lines().count() <= REALISTIC_COMPOSE_LINE_TARGET
        {
            return Ok(());
        }
        Err(format!(
            "composed YAML exceeded {REALISTIC_COMPOSE_LINE_TARGET}-line headroom target (hard maximum {MAX_CANONICAL_YAML_LINES}): component={}, summary={}",
            component_yaml.lines().count(),
            summary_yaml.lines().count()
        )
        .into())
    }

    fn ensure_rejected(
        correctness: crate::report_schema::ReportDocument,
        performance: crate::report_schema::ReportDocument,
        label: &str,
    ) -> TestResult {
        if compose(correctness, performance, "tree-1").is_err() {
            return Ok(());
        }
        Err(format!("composition accepted {label}").into())
    }

    fn component_documents() -> Result<
        (
            crate::report_schema::ReportDocument,
            crate::report_schema::ReportDocument,
        ),
        anyhow::Error,
    > {
        let mut correctness = sample_document()?;
        correctness.metadata.tree = "tree-1".to_owned();
        correctness.configuration.report_mode = ReportMode::Correctness;
        correctness.suites = CANONICAL_CORRECTNESS_SUITES
            .into_iter()
            .map(|(name, required)| {
                let mut suite = correctness
                    .suites
                    .first()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("sample document has no suite"))?;
                suite.summary.name = name.to_owned();
                suite.summary.required = required;
                Ok(suite)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let Some(full_suite) = correctness
            .suites
            .iter_mut()
            .find(|suite| suite.summary.name == "Test262 full corpus")
        else {
            return Err(anyhow::anyhow!(
                "canonical fixture has no full Test262 suite"
            ));
        };
        full_suite.summary.feature_areas = vec![crate::report_schema::FeatureAreaSummary {
            name: "built-ins/Array".to_owned(),
            counts: full_suite.summary.counts,
            manifest_enabled: 1,
            top_skip_reason: "none".to_owned(),
        }];
        correctness.benchmarks = crate::report_schema::BenchmarkSuite {
            name: "Benchmarks".to_owned(),
            duration_ns: 0,
            counts: crate::report_schema::BenchmarkCounts {
                measured: 0,
                in_process_measured: 0,
                failed: 0,
                invalid: 0,
                skipped_reference: 0,
                over_latency_budget: 0,
                over_memory_budget: 0,
            },
            rows: Vec::new(),
        };
        correctness.components = vec![ReportComponent::capture(
            ReportMode::Correctness,
            &correctness.metadata,
            &correctness.environment,
            &correctness.configuration,
            correctness.duration_ns,
        )];

        let mut performance = sample_document()?;
        performance.metadata.tree = "tree-1".to_owned();
        performance.configuration.report_mode = ReportMode::Performance;
        performance.configuration.jetstream = FeatureSelection::Disabled;
        performance.configuration.benchmark_set = BenchmarkSet::Sentinel;
        performance.configuration.benchmark_filter = None;
        performance.configuration.quickjs_baseline = QuickjsBaselineMode::Require;
        performance
            .configuration
            .benchmark
            .reference_quickjs_compiled = false;
        performance.suites.clear();
        let Some(sample_benchmark) = performance.benchmarks.rows.first().cloned() else {
            return Err(anyhow::anyhow!("sample document has no benchmark"));
        };
        performance.benchmarks.rows = CANONICAL_SENTINELS
            .into_iter()
            .map(|id| {
                let mut row = sample_benchmark.clone();
                row.id = id.to_owned();
                row
            })
            .collect();
        performance.benchmarks.counts.measured = 5;
        performance.benchmarks.counts.in_process_measured = 5;
        performance.benchmarks.counts.over_latency_budget = 5;
        performance.components = vec![ReportComponent::capture(
            ReportMode::Performance,
            &performance.metadata,
            &performance.environment,
            &performance.configuration,
            performance.duration_ns,
        )];
        Ok((correctness, performance))
    }
}
