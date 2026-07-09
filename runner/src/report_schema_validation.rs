use anyhow::bail;

use crate::{
    report_schema::{
        BenchmarkSuite, CaseCounts, CaseStatus, DetailCompleteness, DetailLevel, FeatureSelection,
        ReportDocument, ReportMode, ReportSummary, SCHEMA_VERSION, SuiteReport, SuiteStatus,
        SuiteSummary,
    },
    report_schema_support::usize_to_u64,
};

impl ReportDocument {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_header(self.schema_version, self.detail_level, DetailLevel::Full)?;
        for suite in &self.suites {
            validate_suite(suite)?;
        }
        validate_benchmarks(&self.benchmarks)?;
        validate_benchmarks(&self.jetstream)?;
        validate_mode(
            self.configuration.report_mode,
            self.configuration.jetstream,
            &self.benchmarks,
            &self.jetstream,
        )
    }
}

impl ReportSummary {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_header(self.schema_version, self.detail_level, DetailLevel::Summary)?;
        for suite in &self.suites {
            validate_suite_summary(suite)?;
        }
        validate_benchmarks(&self.benchmarks)?;
        validate_benchmarks(&self.jetstream)?;
        validate_mode(
            self.configuration.report_mode,
            self.configuration.jetstream,
            &self.benchmarks,
            &self.jetstream,
        )
    }
}

fn validate_header(
    actual: u32,
    actual_level: DetailLevel,
    expected: DetailLevel,
) -> anyhow::Result<()> {
    if actual != SCHEMA_VERSION {
        bail!("unsupported report schema version {actual}; expected {SCHEMA_VERSION}");
    }
    if actual_level != expected {
        bail!("unexpected report detail level {actual_level:?}; expected {expected:?}");
    }
    Ok(())
}

fn validate_suite(suite: &SuiteReport) -> anyhow::Result<()> {
    validate_suite_summary(&suite.summary)?;
    if usize_to_u64(suite.cases.len()) != suite.summary.case_details.recorded_rows {
        bail!(
            "suite '{}' has {} detail rows but coverage records {}",
            suite.summary.name,
            suite.cases.len(),
            suite.summary.case_details.recorded_rows
        );
    }
    if suite.summary.case_details.completeness == DetailCompleteness::Partial {
        return Ok(());
    }
    let counts = CaseCounts {
        total: usize_to_u64(suite.cases.len()),
        executed: usize_to_u64(
            suite
                .cases
                .iter()
                .filter(|case| case.status != CaseStatus::Skipped)
                .count(),
        ),
        passed: count_status(suite, CaseStatus::Passed),
        failed: count_status(suite, CaseStatus::Failed),
        skipped: count_status(suite, CaseStatus::Skipped),
    };
    if counts != suite.summary.counts {
        bail!(
            "suite '{}' detail counts do not match summary",
            suite.summary.name
        );
    }
    Ok(())
}

fn validate_suite_summary(suite: &SuiteSummary) -> anyhow::Result<()> {
    let counts = suite.counts;
    if counts.executed != counts.passed.saturating_add(counts.failed) {
        bail!("suite '{}' has inconsistent executed count", suite.name);
    }
    if counts.total != counts.executed.saturating_add(counts.skipped) {
        bail!("suite '{}' has inconsistent total count", suite.name);
    }
    let Some(covered_total) = suite
        .case_details
        .recorded_rows
        .checked_add(suite.case_details.omitted_rows)
    else {
        bail!("suite '{}' detail coverage overflows", suite.name);
    };
    if covered_total != counts.total {
        bail!("suite '{}' has inconsistent detail coverage", suite.name);
    }
    let expected_completeness = if suite.case_details.omitted_rows == 0 {
        DetailCompleteness::Complete
    } else {
        DetailCompleteness::Partial
    };
    if suite.case_details.completeness != expected_completeness {
        bail!(
            "suite '{}' has inconsistent detail completeness",
            suite.name
        );
    }
    let expected_status = expected_suite_status(suite);
    if suite.status != expected_status {
        bail!("suite '{}' has inconsistent status", suite.name);
    }
    Ok(())
}

fn validate_benchmarks(suite: &BenchmarkSuite) -> anyhow::Result<()> {
    let row_count = usize_to_u64(suite.rows.len());
    if suite.counts.measured > row_count
        || suite.counts.failed > row_count
        || suite.counts.skipped_reference > row_count
    {
        bail!(
            "benchmark suite '{}' has counts above row count",
            suite.name
        );
    }
    if suite.counts.in_process_measured > suite.counts.measured {
        bail!(
            "benchmark suite '{}' has inconsistent measured counts",
            suite.name
        );
    }
    if suite.counts.invalid > suite.counts.failed {
        bail!(
            "benchmark suite '{}' has invalid count above failures",
            suite.name
        );
    }
    if suite.counts.over_latency_budget > suite.counts.measured
        || suite.counts.over_memory_budget > suite.counts.measured
    {
        bail!(
            "benchmark suite '{}' has budget count above measured",
            suite.name
        );
    }
    Ok(())
}

fn validate_mode(
    report_mode: ReportMode,
    jetstream_selection: FeatureSelection,
    benchmarks: &BenchmarkSuite,
    jetstream: &BenchmarkSuite,
) -> anyhow::Result<()> {
    if jetstream_selection == FeatureSelection::Disabled && !jetstream.rows.is_empty() {
        bail!("JetStream rows are present while JetStream is disabled");
    }
    if report_mode == ReportMode::Correctness {
        if !benchmarks.rows.is_empty() {
            bail!("benchmark rows are present in a correctness report");
        }
        if jetstream_selection == FeatureSelection::Enabled {
            bail!("JetStream cannot be enabled in a correctness report");
        }
    }
    Ok(())
}

fn count_status(suite: &SuiteReport, status: CaseStatus) -> u64 {
    usize_to_u64(
        suite
            .cases
            .iter()
            .filter(|case| case.status == status)
            .count(),
    )
}

const fn expected_suite_status(suite: &SuiteSummary) -> SuiteStatus {
    if suite.counts.failed > 0 {
        return SuiteStatus::Failed;
    }
    if suite.counts.executed == 0 && (suite.counts.skipped > 0 || !suite.skip_reasons.is_empty()) {
        return SuiteStatus::Skipped;
    }
    SuiteStatus::Passed
}
