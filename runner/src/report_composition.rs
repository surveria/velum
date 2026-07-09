use std::path::Path;

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use crate::{
    report_metadata::RunMetadata,
    report_schema::{
        DetailLevel, EnvironmentInfo, ReportDocument, ReportMode, RunConfiguration, SCHEMA_VERSION,
    },
};

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportComponent {
    pub(crate) mode: ReportMode,
    pub(crate) metadata: RunMetadata,
    pub(crate) environment: EnvironmentInfo,
    pub(crate) configuration: RunConfiguration,
    pub(crate) duration_ns: u64,
}

impl ReportComponent {
    pub fn capture(
        mode: ReportMode,
        metadata: &RunMetadata,
        environment: &EnvironmentInfo,
        configuration: &RunConfiguration,
        duration_ns: u64,
    ) -> Self {
        Self {
            mode,
            metadata: metadata.clone(),
            environment: environment.clone(),
            configuration: configuration.clone(),
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

    let configuration = RunConfiguration {
        report_mode: ReportMode::Full,
        jetstream: performance.configuration.jetstream,
        quickjs_differential: correctness.configuration.quickjs_differential,
        test262: correctness.configuration.test262,
        test262_mode: correctness.configuration.test262_mode,
        test262_path_filters: correctness.configuration.test262_path_filters.clone(),
        test262_flag_filters: correctness.configuration.test262_flag_filters.clone(),
        benchmark_filter: performance.configuration.benchmark_filter.clone(),
        benchmark: performance.configuration.benchmark.clone(),
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
        if component.configuration.report_mode != component.mode {
            bail!("report component mode does not match its configuration");
        }
        if !metadata.tree.is_empty() && component.metadata.tree != metadata.tree {
            bail!("report component tree does not match the report tree");
        }
    }
    let full_count = component_count(components, ReportMode::Full);
    let correctness_count = component_count(components, ReportMode::Correctness);
    let performance_count = component_count(components, ReportMode::Performance);
    let valid = match configuration.report_mode {
        ReportMode::Full => {
            (components.len() == 1 && full_count == 1)
                || (components.len() == 2 && correctness_count == 1 && performance_count == 1)
        }
        ReportMode::Correctness => components.len() == 1 && correctness_count == 1,
        ReportMode::Performance => components.len() == 1 && performance_count == 1,
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
        if component.metadata.tree != expected_tree {
            bail!(
                "{expected_mode:?} component tree mismatch: '{}' != '{expected_tree}'",
                component.metadata.tree
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

#[cfg(test)]
mod tests {
    use super::{compose, validate_output_path};
    use crate::{
        report_composition::ReportComponent,
        report_schema::{FeatureSelection, ReportMode},
        report_schema_tests::sample_document,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn combines_exact_tree_correctness_and_performance_documents() -> TestResult {
        let (correctness, performance) = component_documents()?;
        let report = compose(correctness, performance, "tree-1")?;
        if report.configuration.report_mode != ReportMode::Full
            || report.suites.len() != 1
            || report.benchmarks.rows.len() != 1
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
        component.metadata.tree = "tree-2".to_owned();
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
        component.metadata.timestamp = "20260710T010100Z".to_owned();
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
        performance.suites.clear();
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
