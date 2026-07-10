use std::collections::BTreeSet;

use anyhow::bail;

use crate::{
    report_benchmark_methodology::ReferenceSource,
    report_composition::validate_components,
    report_schema::{
        BenchmarkContributionFlag, BenchmarkCountContribution, BenchmarkStatus, BenchmarkSuite,
        BudgetStatus, CaseCounts, CaseStatus, DetailCompleteness, DetailLevel, FeatureSelection,
        JetStreamSuite, MAX_FAILURE_DIAGNOSTICS, Measurement, MeasurementAvailability,
        ReportDocument, ReportMode, ReportSummary, SCHEMA_VERSION, SuiteReport, SuiteStatus,
        SuiteSummary, TEST262_FILE_SUITE, TEST262_FULL_SUITE,
    },
    report_schema_support::usize_to_u64,
};

impl ReportDocument {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_header(self.schema_version, self.detail_level, DetailLevel::Full)?;
        validate_components(&self.metadata, &self.configuration, &self.components)?;
        validate_unique_suite_names(self.suites.iter().map(|suite| suite.summary.name.as_str()))?;
        validate_diagnostic_limit(self.suites.iter().map(|suite| &suite.summary))?;
        for suite in &self.suites {
            validate_suite(suite)?;
        }
        validate_diagnostic_sources(self.suites.iter().map(|suite| &suite.summary))?;
        validate_benchmarks(&self.benchmarks)?;
        validate_jetstream(&self.jetstream)?;
        validate_mode(
            &self.configuration,
            self.suites.len(),
            &self.benchmarks,
            &self.jetstream,
        )
    }
}

impl ReportSummary {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_header(self.schema_version, self.detail_level, DetailLevel::Summary)?;
        validate_components(&self.metadata, &self.configuration, &self.components)?;
        validate_unique_suite_names(self.suites.iter().map(|suite| suite.name.as_str()))?;
        validate_diagnostic_limit(self.suites.iter())?;
        for suite in &self.suites {
            validate_suite_summary(suite)?;
        }
        validate_diagnostic_sources(self.suites.iter())?;
        validate_benchmarks(&self.benchmarks)?;
        validate_jetstream(&self.jetstream)?;
        validate_mode(
            &self.configuration,
            self.suites.len(),
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
    let mut case_ids = BTreeSet::new();
    for case in &suite.cases {
        if !case_ids.insert(case.id.as_str()) {
            bail!(
                "suite '{}' has duplicate case id '{}'",
                suite.summary.name,
                case.id
            );
        }
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
    if counts.total > suite.summary.counts.total
        || counts.executed > suite.summary.counts.executed
        || counts.passed > suite.summary.counts.passed
        || counts.failed > suite.summary.counts.failed
        || counts.skipped > suite.summary.counts.skipped
    {
        bail!(
            "suite '{}' detail status counts exceed its summary",
            suite.summary.name
        );
    }
    if suite.summary.case_details.completeness == DetailCompleteness::Complete
        && counts != suite.summary.counts
    {
        bail!(
            "suite '{}' detail counts do not match summary",
            suite.summary.name
        );
    }
    if !suite.cases.is_empty() {
        for diagnostic in suite
            .summary
            .failure_diagnostics
            .iter()
            .flat_map(|diagnostics| diagnostics.groups.iter())
        {
            let representative = suite.cases.iter().find(|case| {
                case.id == diagnostic.representative_case
                    && case.source == diagnostic.representative_source
            });
            if !representative.is_some_and(|case| case.status == CaseStatus::Failed) {
                bail!(
                    "suite '{}' diagnostic representative is absent or not failed",
                    suite.summary.name
                );
            }
        }
    }
    Ok(())
}

fn validate_suite_summary(suite: &SuiteSummary) -> anyhow::Result<()> {
    let counts = suite.counts;
    let Some(executed) = counts.passed.checked_add(counts.failed) else {
        bail!("suite '{}' executed count overflows", suite.name);
    };
    if counts.executed != executed {
        bail!("suite '{}' has inconsistent executed count", suite.name);
    }
    let Some(total) = counts.executed.checked_add(counts.skipped) else {
        bail!("suite '{}' total count overflows", suite.name);
    };
    if counts.total != total {
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
    validate_feature_areas(suite)?;
    validate_failure_diagnostics(suite)?;
    Ok(())
}

fn validate_failure_diagnostics(suite: &SuiteSummary) -> anyhow::Result<()> {
    let diagnostics = match (&suite.failure_diagnostics, &suite.diagnostics_derived_from) {
        (Some(diagnostics), None) => diagnostics,
        (None, Some(source)) if suite.counts.failed > 0 && !source.is_empty() => return Ok(()),
        (None, None) if suite.counts.failed == 0 => return Ok(()),
        _ => bail!(
            "suite '{}' has ambiguous or missing failure diagnostics",
            suite.name
        ),
    };
    if diagnostics.total_failed != suite.counts.failed {
        bail!(
            "suite '{}' diagnostic failure total does not match summary",
            suite.name
        );
    }
    let category_total = checked_sum(
        diagnostics
            .categories
            .iter()
            .map(|category| category.failed),
        "failure category counts",
    )?;
    if category_total != diagnostics.total_failed {
        bail!(
            "suite '{}' failure category counts are incomplete",
            suite.name
        );
    }
    let mut category_names = BTreeSet::new();
    for category in &diagnostics.categories {
        if category.category.is_empty()
            || category.failed == 0
            || !category_names.insert(category.category.as_str())
        {
            bail!(
                "suite '{}' has empty or duplicate failure categories",
                suite.name
            );
        }
    }
    let represented = checked_sum(
        diagnostics.groups.iter().map(|group| group.count),
        "represented failure counts",
    )?;
    if represented != diagnostics.represented_failed
        || represented > diagnostics.total_failed
        || diagnostics.groups.iter().any(|group| {
            group.count == 0
                || group.category.is_empty()
                || group.feature_area.is_empty()
                || group.reason.is_empty()
                || group.representative_case.is_empty()
                || group.representative_source.is_empty()
                || !category_names.contains(group.category.as_str())
        })
    {
        bail!(
            "suite '{}' has inconsistent represented failure diagnostics",
            suite.name
        );
    }
    let Some(group_total) =
        usize_to_u64(diagnostics.groups.len()).checked_add(diagnostics.omitted_groups)
    else {
        bail!("suite '{}' diagnostic group count overflows", suite.name);
    };
    if group_total != diagnostics.total_groups {
        bail!(
            "suite '{}' diagnostic group coverage is inconsistent",
            suite.name
        );
    }
    let mut keys = BTreeSet::new();
    for group in &diagnostics.groups {
        if !keys.insert((&group.feature_area, &group.category, &group.reason)) {
            bail!("suite '{}' has duplicate diagnostic groups", suite.name);
        }
    }
    Ok(())
}

fn validate_diagnostic_sources<'suite>(
    suites: impl Iterator<Item = &'suite SuiteSummary>,
) -> anyhow::Result<()> {
    let suites = suites.collect::<Vec<_>>();
    for suite in &suites {
        let Some(source) = suite.diagnostics_derived_from.as_deref() else {
            continue;
        };
        if suite.name != TEST262_FILE_SUITE || source != TEST262_FULL_SUITE {
            bail!(
                "suite '{}' has unsupported derived diagnostic provenance",
                suite.name
            );
        }
        let source_exists = suites
            .iter()
            .any(|candidate| candidate.name == source && candidate.failure_diagnostics.is_some());
        if !source_exists {
            bail!(
                "suite '{}' references missing diagnostics in '{source}'",
                suite.name
            );
        }
    }
    Ok(())
}

fn validate_feature_areas(suite: &SuiteSummary) -> anyhow::Result<()> {
    if suite.feature_areas.is_empty() {
        return Ok(());
    }
    let mut names = BTreeSet::new();
    let mut totals = CaseCounts {
        total: 0,
        executed: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
    };
    for area in &suite.feature_areas {
        if area.name.is_empty() || !names.insert(area.name.as_str()) {
            bail!(
                "suite '{}' has empty or duplicate feature areas",
                suite.name
            );
        }
        let Some(executed) = area.counts.passed.checked_add(area.counts.failed) else {
            bail!(
                "suite '{}' feature-area executed count overflows",
                suite.name
            );
        };
        let Some(total) = area.counts.executed.checked_add(area.counts.skipped) else {
            bail!("suite '{}' feature-area total count overflows", suite.name);
        };
        if executed != area.counts.executed || total != area.counts.total {
            bail!(
                "suite '{}' has inconsistent feature-area counts",
                suite.name
            );
        }
        totals.total = checked_count_add(totals.total, area.counts.total)?;
        totals.executed = checked_count_add(totals.executed, area.counts.executed)?;
        totals.passed = checked_count_add(totals.passed, area.counts.passed)?;
        totals.failed = checked_count_add(totals.failed, area.counts.failed)?;
        totals.skipped = checked_count_add(totals.skipped, area.counts.skipped)?;
    }
    if totals != suite.counts {
        bail!(
            "suite '{}' feature-area totals do not match suite counts",
            suite.name
        );
    }
    Ok(())
}

fn checked_count_add(left: u64, right: u64) -> anyhow::Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| anyhow::anyhow!("feature-area aggregate count overflows"))
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
    let mut row_ids = BTreeSet::new();
    for row in &suite.rows {
        if !row_ids.insert(row.id.as_str()) {
            bail!(
                "benchmark suite '{}' has duplicate row id '{}'",
                suite.name,
                row.id
            );
        }
        validate_measurement(&row.engine, &row.id, "engine")?;
        validate_measurement(&row.reference, &row.id, "reference")?;
        if (row.latency_ratio_centi_units.is_some() || row.memory_ratio_centi_units.is_some())
            && (row.engine.availability != MeasurementAvailability::Measured
                || row.reference.availability != MeasurementAvailability::Measured)
        {
            bail!(
                "benchmark '{}' has a ratio without both measurements",
                row.id
            );
        }
        if suite.name == "Benchmarks" {
            let Some(methodology) = &row.methodology else {
                bail!("project benchmark '{}' is missing methodology", row.id);
            };
            methodology.validate()?;
            validate_reference_source(row)?;
            let Some(contribution) = row.count_contribution else {
                bail!(
                    "project benchmark '{}' is missing its count contribution",
                    row.id
                );
            };
            validate_count_contribution(row, contribution)?;
        } else if row.methodology.is_some() || row.count_contribution.is_some() {
            bail!(
                "non-project benchmark '{}' has project-only metadata",
                row.id
            );
        }
    }
    if suite.name == "Benchmarks" {
        validate_project_benchmark_counts(suite)?;
    }
    Ok(())
}

fn validate_jetstream(suite: &JetStreamSuite) -> anyhow::Result<()> {
    let recorded = usize_to_u64(suite.rows.len());
    if recorded != suite.row_details.recorded_rows {
        bail!(
            "JetStream suite has {} rows but coverage records {}",
            suite.rows.len(),
            suite.row_details.recorded_rows
        );
    }
    let Some(total) = suite
        .row_details
        .recorded_rows
        .checked_add(suite.row_details.omitted_rows)
    else {
        bail!("JetStream row coverage overflows");
    };
    if total != suite.counts.total || suite.row_details.omitted_rows != 0 {
        bail!("JetStream report must retain every official selected row");
    }
    if suite.row_details.completeness != DetailCompleteness::Complete {
        bail!("JetStream report has incomplete row coverage");
    }
    for count in [
        suite.counts.measured,
        suite.counts.failed,
        suite.counts.invalid,
        suite.counts.skipped,
        suite.counts.unavailable_reference,
        suite.counts.missing_reference,
        suite.counts.over_latency_budget,
    ] {
        if count > total {
            bail!("JetStream count exceeds selected row count");
        }
    }
    if suite.counts.invalid > suite.counts.failed
        || suite.counts.over_latency_budget > suite.counts.measured
    {
        bail!("JetStream aggregate counts are inconsistent");
    }
    let mut row_ids = BTreeSet::new();
    for row in &suite.rows {
        if !row_ids.insert(row.id.as_str()) {
            bail!("JetStream suite has duplicate row id '{}'", row.id);
        }
        if row.engine_cv_permille.is_some() && row.engine_median_duration_ns.is_none() {
            bail!("JetStream row '{}' has engine CV without a median", row.id);
        }
        if row.reference_cv_permille.is_some() && row.reference_median_duration_ns.is_none() {
            bail!(
                "JetStream row '{}' has reference CV without a median",
                row.id
            );
        }
        if row.latency_ratio_centi_units.is_some()
            && (row.engine_median_duration_ns.is_none()
                || row.reference_median_duration_ns.is_none())
        {
            bail!(
                "JetStream row '{}' has a ratio without both medians",
                row.id
            );
        }
        validate_jetstream_reference(row)?;
    }
    if !suite.details.is_empty() {
        if suite.details.len() != suite.rows.len() {
            bail!("JetStream diagnostic details do not cover every row");
        }
        let detail_ids = suite
            .details
            .iter()
            .map(|detail| detail.id.as_str())
            .collect::<BTreeSet<_>>();
        if detail_ids != row_ids {
            bail!("JetStream diagnostic detail ids do not match compact rows");
        }
    }
    if suite.derived_counts()? != suite.counts {
        bail!("JetStream aggregate counts do not match their rows");
    }
    Ok(())
}

fn validate_jetstream_reference(row: &crate::report_schema::JetStreamRecord) -> anyhow::Result<()> {
    let measured = row.reference_median_duration_ns.is_some();
    let valid = match row.reference_source {
        Some(ReferenceSource::QuickjsBaseline) => true,
        Some(ReferenceSource::QuickjsLive) => measured,
        Some(
            ReferenceSource::QuickjsLiveFailed
            | ReferenceSource::QuickjsBaselineMissing
            | ReferenceSource::NotConfigured,
        )
        | None => !measured,
    };
    if !valid {
        bail!(
            "JetStream row '{}' has inconsistent reference provenance",
            row.id
        );
    }
    Ok(())
}

fn validate_project_benchmark_counts(suite: &BenchmarkSuite) -> anyhow::Result<()> {
    let mut actual = crate::report_schema::BenchmarkCounts {
        measured: 0,
        in_process_measured: 0,
        failed: 0,
        invalid: 0,
        skipped_reference: 0,
        over_latency_budget: 0,
        over_memory_budget: 0,
    };
    for row in &suite.rows {
        let Some(contribution) = row.count_contribution else {
            bail!(
                "project benchmark '{}' is missing its count contribution",
                row.id
            );
        };
        actual.measured = add_flag(actual.measured, contribution.measured)?;
        actual.in_process_measured =
            add_flag(actual.in_process_measured, contribution.in_process_measured)?;
        actual.failed = add_flag(actual.failed, contribution.failed)?;
        actual.invalid = add_flag(actual.invalid, contribution.invalid)?;
        actual.skipped_reference =
            add_flag(actual.skipped_reference, contribution.skipped_reference)?;
        actual.over_latency_budget =
            add_flag(actual.over_latency_budget, contribution.over_latency_budget)?;
        actual.over_memory_budget =
            add_flag(actual.over_memory_budget, contribution.over_memory_budget)?;
    }
    if actual != suite.counts {
        bail!("project benchmark row counts do not match summary counts");
    }
    Ok(())
}

fn add_flag(value: u64, flag: BenchmarkContributionFlag) -> anyhow::Result<u64> {
    value
        .checked_add(u64::from(flag == BenchmarkContributionFlag::Counted))
        .ok_or_else(|| anyhow::anyhow!("benchmark contribution count overflows"))
}

fn validate_measurement(
    measurement: &Measurement,
    benchmark_id: &str,
    label: &str,
) -> anyhow::Result<()> {
    let consistent = match measurement.availability {
        MeasurementAvailability::Measured => {
            measurement.wall_duration_ns.is_some()
                && measurement.median_duration_ns.is_some()
                && measurement.coefficient_variation_permille.is_some()
        }
        MeasurementAvailability::NotConfigured => {
            measurement.wall_duration_ns.is_none()
                && measurement.median_duration_ns.is_none()
                && measurement.coefficient_variation_permille.is_none()
        }
        MeasurementAvailability::NotAvailable => {
            measurement.wall_duration_ns.is_some()
                && measurement.median_duration_ns.is_none()
                && measurement.coefficient_variation_permille.is_none()
        }
        MeasurementAvailability::NotMeasured => {
            measurement.median_duration_ns.is_none()
                && measurement.coefficient_variation_permille.is_none()
        }
    };
    if !consistent {
        bail!("benchmark '{benchmark_id}' has inconsistent {label} measurement availability");
    }
    Ok(())
}

fn validate_reference_source(row: &crate::report_schema::BenchmarkRecord) -> anyhow::Result<()> {
    let source = row
        .methodology
        .as_ref()
        .and_then(|methodology| methodology.reference_source);
    let consistent = match source {
        Some(ReferenceSource::QuickjsBaseline | ReferenceSource::QuickjsLive) => {
            row.reference.availability == MeasurementAvailability::Measured
        }
        Some(ReferenceSource::QuickjsLiveFailed) => {
            row.reference.availability == MeasurementAvailability::NotAvailable
        }
        Some(ReferenceSource::NotConfigured) => {
            row.reference.availability == MeasurementAvailability::NotConfigured
        }
        Some(ReferenceSource::QuickjsBaselineMissing) => false,
        None => row.reference.availability == MeasurementAvailability::NotMeasured,
    };
    if !consistent {
        bail!(
            "benchmark '{}' has inconsistent reference provenance",
            row.id
        );
    }
    Ok(())
}

fn validate_count_contribution(
    row: &crate::report_schema::BenchmarkRecord,
    contribution: BenchmarkCountContribution,
) -> anyhow::Result<()> {
    let failed = matches!(
        row.status,
        BenchmarkStatus::Failed | BenchmarkStatus::Invalid
    );
    let missing_reference = matches!(
        row.reference.availability,
        MeasurementAvailability::NotConfigured | MeasurementAvailability::NotAvailable
    );
    let counted = BenchmarkContributionFlag::Counted;
    let not_counted = BenchmarkContributionFlag::NotCounted;
    if (contribution.measured == counted)
        != (row.engine.availability == MeasurementAvailability::Measured)
        || contribution.in_process_measured != contribution.measured
        || (contribution.failed == counted) != failed
        || contribution.invalid == counted && contribution.failed == not_counted
        || contribution.skipped_reference == counted && !missing_reference
        || (contribution.over_latency_budget == counted)
            != (row.latency_budget == BudgetStatus::Over)
        || contribution.over_memory_budget == counted && row.memory_ratio_centi_units.is_none()
    {
        bail!("benchmark '{}' has inconsistent count contribution", row.id);
    }
    Ok(())
}

fn validate_unique_suite_names<'name>(
    names: impl Iterator<Item = &'name str>,
) -> anyhow::Result<()> {
    let mut unique = BTreeSet::new();
    for name in names {
        if !unique.insert(name) {
            bail!("report has duplicate suite name '{name}'");
        }
    }
    Ok(())
}

fn validate_diagnostic_limit<'suite>(
    suites: impl Iterator<Item = &'suite SuiteSummary>,
) -> anyhow::Result<()> {
    let count = suites
        .map(|suite| {
            suite
                .failure_diagnostics
                .as_ref()
                .map_or(0, |diagnostics| diagnostics.groups.len())
        })
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(|| anyhow::anyhow!("failure diagnostic count overflows"))?;
    if count > MAX_FAILURE_DIAGNOSTICS {
        bail!("report has {count} failure diagnostics; maximum is {MAX_FAILURE_DIAGNOSTICS}");
    }
    Ok(())
}

fn checked_sum(mut values: impl Iterator<Item = u64>, label: &str) -> anyhow::Result<u64> {
    values.try_fold(0u64, |total, value| {
        total
            .checked_add(value)
            .ok_or_else(|| anyhow::anyhow!("{label} overflow"))
    })
}

fn validate_mode(
    configuration: &crate::report_schema::RunConfiguration,
    suite_count: usize,
    benchmarks: &BenchmarkSuite,
    jetstream: &JetStreamSuite,
) -> anyhow::Result<()> {
    let report_mode = configuration.report_mode;
    let jetstream_selection = configuration.jetstream;
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
    if report_mode == ReportMode::Performance {
        if suite_count != 0 {
            bail!("correctness suites are present in a performance report");
        }
        if jetstream_selection == FeatureSelection::Enabled || !jetstream.rows.is_empty() {
            bail!("JetStream cannot be enabled in a performance report");
        }
    }
    if report_mode == ReportMode::Jetstream {
        if suite_count != 0 || !benchmarks.rows.is_empty() {
            bail!("non-JetStream suites are present in a JetStream report");
        }
        if jetstream_selection != FeatureSelection::Enabled {
            bail!("JetStream must be enabled in a JetStream report");
        }
        if configuration.suite_max_duration_ns.is_none() {
            bail!("JetStream report is missing its suite wall budget");
        }
        if configuration.quickjs_baseline == crate::report_schema::QuickjsBaselineMode::Read
            && configuration.benchmark.reference_quickjs_compiled
        {
            bail!("strict JetStream baseline reads must not compile QuickJS");
        }
        if configuration.quickjs_baseline == crate::report_schema::QuickjsBaselineMode::Refresh
            && !configuration.benchmark.reference_quickjs_compiled
        {
            bail!("JetStream baseline refresh requires compiled QuickJS");
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
