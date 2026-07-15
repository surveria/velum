use anyhow::bail;
use serde::{Deserialize, Serialize};

use crate::{
    CaseRow, CorpusReport, FullReport, STATUS_FAILED, STATUS_PASSED, STATUS_SKIPPED, benchmarks,
    failure_classification,
    report_benchmark_methodology::BenchmarkMethodology,
    report_composition::ReportComponent,
    report_metadata::RunMetadata,
    report_schema_support::{
        duration_ns, labeled_count, optional_cv_permille, optional_duration, optional_ratio,
        parse_duration, usize_to_u64,
    },
};

#[path = "report_schema_compaction.rs"]
mod compaction;
use compaction::{case_counts, case_detail_coverage, limit_diagnostics, suite_status};
#[path = "report_schema_columns.rs"]
mod columns;
use columns::BenchmarkColumns;
#[path = "report_schema_jetstream.rs"]
mod jetstream_schema;
pub use jetstream_schema::{JetStreamDetailRecord, JetStreamRecord, JetStreamSuite};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_FAILURE_DIAGNOSTICS: usize = 30;
pub const TEST262_FULL_SUITE: &str = "Test262 full corpus";
pub const TEST262_FILE_SUITE: &str = "Test262 file conformance";

pub const NO_VALUE: &str = "-";

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportDocument {
    pub(crate) schema_version: u32,
    pub(crate) detail_level: DetailLevel,
    pub(crate) metadata: RunMetadata,
    pub(crate) environment: EnvironmentInfo,
    pub(crate) configuration: RunConfiguration,
    pub(crate) components: Vec<ReportComponent>,
    pub(crate) duration_ns: u64,
    pub(crate) suites: Vec<SuiteReport>,
    pub(crate) benchmarks: BenchmarkSuite,
    #[serde(default, skip_serializing_if = "JetStreamSuite::is_empty")]
    pub(crate) jetstream: JetStreamSuite,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportSummary {
    pub(crate) schema_version: u32,
    pub(crate) detail_level: DetailLevel,
    pub(crate) metadata: RunMetadata,
    pub(crate) environment: EnvironmentInfo,
    pub(crate) configuration: RunConfiguration,
    pub(crate) components: Vec<ReportComponent>,
    pub(crate) duration_ns: u64,
    pub(crate) suites: Vec<SuiteSummary>,
    pub(crate) benchmarks: BenchmarkSuite,
    #[serde(default, skip_serializing_if = "JetStreamSuite::is_empty")]
    pub(crate) jetstream: JetStreamSuite,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Full,
    Summary,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportMode {
    Full,
    Correctness,
    Performance,
    Jetstream,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureSelection {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputAvailability {
    Configured,
    NotConfigured,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Test262Mode {
    Full,
    Manifest,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSet {
    Full,
    Sentinel,
    Invalid,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickjsBaselineMode {
    Off,
    Read,
    Require,
    Refresh,
    Invalid,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentInfo {
    pub(crate) operating_system: String,
    pub(crate) architecture: String,
    pub(crate) available_parallelism: u64,
    pub(crate) build_profile: String,
    pub(crate) kernel_release: Option<String>,
    pub(crate) cpu_model: Option<String>,
    pub(crate) cpu_affinity: Option<String>,
    pub(crate) scaling_governor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct RunConfiguration {
    pub(crate) report_mode: ReportMode,
    pub(crate) jetstream: FeatureSelection,
    pub(crate) quickjs_differential: InputAvailability,
    pub(crate) test262: InputAvailability,
    pub(crate) test262_mode: Test262Mode,
    pub(crate) test262_path_filters: Vec<String>,
    pub(crate) test262_flag_filters: Vec<String>,
    pub(crate) benchmark_set: BenchmarkSet,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) benchmark_filter: Option<String>,
    pub(crate) quickjs_baseline: QuickjsBaselineMode,
    pub(crate) benchmark: BenchmarkConfiguration,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) suite_max_duration_ns: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkConfiguration {
    pub(crate) reference_quickjs_compiled: bool,
    pub(crate) warmup_duration_ns: u64,
    pub(crate) minimum_sample_duration_ns: u64,
    pub(crate) samples: u64,
    pub(crate) minimum_operation_duration_ns: u64,
    pub(crate) maximum_cv_permille: u32,
    pub(crate) attempts: u64,
    #[serde(default)]
    pub(crate) maximum_operation_duration_ns: u64,
    #[serde(default)]
    pub(crate) maximum_total_duration_ns: u64,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuiteReport {
    #[serde(flatten)]
    pub(crate) summary: SuiteSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) cases: Vec<CaseRecord>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuiteSummary {
    pub(crate) name: String,
    pub(crate) required: bool,
    pub(crate) status: SuiteStatus,
    pub(crate) counts: CaseCounts,
    pub(crate) case_details: CaseDetailCoverage,
    pub(crate) duration_ns: u64,
    pub(crate) skip_reasons: Vec<SkipReasonSummary>,
    pub(crate) feature_areas: Vec<FeatureAreaSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) failure_diagnostics: Option<FailureDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostics_derived_from: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaseDetailCoverage {
    pub(crate) completeness: DetailCompleteness,
    pub(crate) recorded_rows: u64,
    pub(crate) omitted_rows: u64,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailCompleteness {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuiteStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaseCounts {
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) total: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) executed: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) passed: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) failed: u64,
    #[serde(default, skip_serializing_if = "is_default")]
    pub(crate) skipped: u64,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaseRecord {
    pub(crate) id: String,
    pub(crate) status: CaseStatus,
    pub(crate) source: String,
    pub(crate) duration_ns: u64,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkipReasonSummary {
    pub(crate) skipped: u64,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct FeatureAreaSummary {
    pub(crate) name: String,
    #[serde(flatten)]
    pub(crate) counts: CaseCounts,
    pub(crate) manifest_enabled: u64,
    pub(crate) top_skip_reason: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticGroup {
    pub(crate) count: u64,
    pub(crate) feature_area: String,
    pub(crate) category: String,
    pub(crate) reason: String,
    pub(crate) representative_case: String,
    pub(crate) representative_source: String,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailureDiagnostics {
    pub(crate) total_failed: u64,
    pub(crate) represented_failed: u64,
    pub(crate) total_groups: u64,
    pub(crate) omitted_groups: u64,
    pub(crate) categories: Vec<FailureCategorySummary>,
    pub(crate) groups: Vec<DiagnosticGroup>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailureCategorySummary {
    pub(crate) category: String,
    pub(crate) failed: u64,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkSuite {
    pub(crate) name: String,
    pub(crate) duration_ns: u64,
    pub(crate) counts: BenchmarkCounts,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rows: Vec<BenchmarkRecord>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkCounts {
    pub(crate) measured: u64,
    pub(crate) in_process_measured: u64,
    pub(crate) failed: u64,
    pub(crate) invalid: u64,
    pub(crate) skipped_reference: u64,
    pub(crate) over_latency_budget: u64,
    pub(crate) over_memory_budget: u64,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkRecord {
    pub(crate) id: String,
    pub(crate) status: BenchmarkStatus,
    pub(crate) source: String,
    pub(crate) iterations: Option<u64>,
    pub(crate) case_duration_ns: Option<u64>,
    pub(crate) engine: Measurement,
    pub(crate) reference: Measurement,
    pub(crate) latency_ratio_centi_units: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) memory_ratio_centi_units: Option<u64>,
    pub(crate) latency_budget: BudgetStatus,
    pub(crate) quality: QualityStatus,
    pub(crate) methodology: Option<BenchmarkMethodology>,
    pub(crate) count_contribution: Option<BenchmarkCountContribution>,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkCountContribution {
    pub(crate) measured: BenchmarkContributionFlag,
    pub(crate) in_process_measured: BenchmarkContributionFlag,
    pub(crate) failed: BenchmarkContributionFlag,
    pub(crate) invalid: BenchmarkContributionFlag,
    pub(crate) skipped_reference: BenchmarkContributionFlag,
    pub(crate) over_latency_budget: BenchmarkContributionFlag,
    pub(crate) over_memory_budget: BenchmarkContributionFlag,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkContributionFlag {
    #[default]
    NotCounted,
    Counted,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct Measurement {
    pub(crate) availability: MeasurementAvailability,
    pub(crate) wall_duration_ns: Option<u64>,
    pub(crate) median_duration_ns: Option<u64>,
    pub(crate) coefficient_variation_permille: Option<u32>,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasurementAvailability {
    Measured,
    NotConfigured,
    NotAvailable,
    NotMeasured,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkStatus {
    Measured,
    WithinBudget,
    TrackedException,
    Failed,
    Invalid,
    Skipped,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetStatus {
    Within,
    Over,
    Unavailable,
    NotConfigured,
    Invalid,
    NotMeasured,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityStatus {
    Valid,
    Invalid,
    NotMeasured,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

impl ReportDocument {
    pub fn from_run(
        report: FullReport,
        environment: EnvironmentInfo,
        configuration: RunConfiguration,
    ) -> anyhow::Result<Self> {
        let jetstream = JetStreamSuite::from_jetstream(&report.jetstream)?;
        let component = ReportComponent::capture(
            configuration.report_mode,
            &report.metadata,
            &environment,
            &configuration,
            duration_ns(report.elapsed),
        );
        let mut suites = report
            .corpora
            .into_iter()
            .map(SuiteReport::from_run)
            .collect::<anyhow::Result<Vec<_>>>()?;
        limit_diagnostics(&mut suites)?;
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            detail_level: DetailLevel::Full,
            metadata: report.metadata,
            environment,
            configuration,
            components: vec![component],
            duration_ns: duration_ns(report.elapsed),
            suites,
            benchmarks: BenchmarkSuite::from_benchmarks(report.benchmarks)?,
            jetstream,
        })
    }

    pub fn summary(&self) -> ReportSummary {
        ReportSummary {
            schema_version: self.schema_version,
            detail_level: DetailLevel::Summary,
            metadata: self.metadata.clone(),
            environment: self.environment.clone(),
            configuration: self.configuration.clone(),
            components: self.components.clone(),
            duration_ns: self.duration_ns,
            suites: self
                .suites
                .iter()
                .map(|suite| suite.summary.clone())
                .collect(),
            benchmarks: self.benchmarks.clone(),
            jetstream: self.jetstream.clone(),
        }
    }

    pub fn bounded_component(&self) -> anyhow::Result<Self> {
        let mut bounded = self.clone();
        limit_diagnostics(&mut bounded.suites)?;
        let has_full_feature_map = bounded
            .suites
            .iter()
            .any(|suite| suite.summary.name == TEST262_FULL_SUITE);
        for suite in &mut bounded.suites {
            suite.cases.clear();
            suite.summary.case_details =
                case_detail_coverage(suite.summary.counts.total, usize_to_u64(suite.cases.len()))?;
            if has_full_feature_map && suite.summary.name == TEST262_FILE_SUITE {
                suite.summary.feature_areas.clear();
            }
        }
        bounded.validate()?;
        Ok(bounded)
    }

    pub fn failed_count(&self) -> u64 {
        let suite_failures = self
            .suites
            .iter()
            .filter(|suite| suite.summary.required)
            .map(|suite| suite.summary.counts.failed)
            .fold(0u64, u64::saturating_add);
        suite_failures.saturating_add(self.benchmarks.counts.failed)
    }
}

impl ReportSummary {
    pub fn suite(&self, name: &str) -> Option<&SuiteSummary> {
        self.suites.iter().find(|suite| suite.name == name)
    }
}

impl SuiteReport {
    fn from_run(corpus: CorpusReport) -> anyhow::Result<Self> {
        let counts = case_counts(
            corpus.total(),
            corpus.passed(),
            corpus.failed(),
            corpus.skipped(),
        )?;
        let cases = corpus
            .rows
            .into_iter()
            .map(CaseRecord::from_run)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let skip_reasons: Vec<SkipReasonSummary> = corpus
            .skip_reasons
            .into_iter()
            .map(|row| SkipReasonSummary {
                skipped: usize_to_u64(row.skipped),
                reason: row.reason,
            })
            .collect();
        let status = suite_status(counts, !skip_reasons.is_empty());
        let case_details = case_detail_coverage(counts.total, usize_to_u64(cases.len()))?;
        let diagnostics = failure_classification::diagnostic_groups(&cases)?;
        let feature_areas = corpus
            .feature_areas
            .into_iter()
            .map(FeatureAreaSummary::from_run)
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Self {
            summary: SuiteSummary {
                name: corpus.name.to_owned(),
                required: corpus.required,
                status,
                counts,
                case_details,
                duration_ns: duration_ns(corpus.elapsed),
                skip_reasons,
                feature_areas,
                failure_diagnostics: diagnostics,
                diagnostics_derived_from: None,
            },
            cases,
        })
    }
}

impl CaseRecord {
    fn from_run(row: CaseRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.case,
            status: CaseStatus::from_label(&row.status)?,
            source: row.source,
            duration_ns: duration_ns(row.elapsed),
            detail: row.detail,
        })
    }
}

impl FeatureAreaSummary {
    fn from_run(row: crate::FeatureAreaRow) -> anyhow::Result<Self> {
        let passed = labeled_count(&row.passed)?;
        let failed = labeled_count(&row.failed)?;
        let skipped = labeled_count(&row.skipped)?;
        Ok(Self {
            name: row.feature_area,
            counts: CaseCounts {
                total: usize_to_u64(row.total),
                executed: usize_to_u64(row.executed),
                passed,
                failed,
                skipped,
            },
            manifest_enabled: usize_to_u64(row.manifest_enabled),
            top_skip_reason: row.top_skip_reason,
        })
    }
}

impl BenchmarkSuite {
    fn from_benchmarks(report: benchmarks::BenchmarkReport) -> anyhow::Result<Self> {
        Ok(Self {
            name: "Benchmarks".to_owned(),
            duration_ns: duration_ns(report.elapsed),
            counts: BenchmarkCounts {
                measured: usize_to_u64(report.measured),
                in_process_measured: usize_to_u64(report.in_process_measured),
                failed: usize_to_u64(report.failed),
                invalid: usize_to_u64(report.invalid),
                skipped_reference: usize_to_u64(report.skipped),
                over_latency_budget: usize_to_u64(report.over_latency_budget),
                over_memory_budget: usize_to_u64(report.over_memory_budget),
            },
            rows: report
                .rows
                .into_iter()
                .map(BenchmarkRecord::from_benchmark)
                .collect::<anyhow::Result<Vec<_>>>()?,
        })
    }
}

impl BenchmarkRecord {
    fn from_benchmark(row: benchmarks::BenchmarkRow) -> anyhow::Result<Self> {
        let contribution = row.count_contribution;
        let mut record = Self::from_columns(BenchmarkColumns {
            id: row.benchmark,
            status: row.status,
            source: row.source,
            iterations: Some(usize_to_u64(row.iterations)),
            case_duration: row.case_elapsed,
            engine_measurement: row.velum_measure,
            engine_median: row.velum_eval,
            engine_cv: row.velum_cv,
            reference_measurement: row.quickjs_measure,
            reference_median: row.quickjs_eval,
            reference_cv: row.quickjs_cv,
            latency_ratio: row.latency_ratio,
            memory_ratio: row.memory_ratio,
            latency_budget: row.latency_budget,
            quality: row.quality,
            methodology: Some(row.methodology.into()),
            detail: row.detail,
        })?;
        record.count_contribution = Some(BenchmarkCountContribution {
            measured: contribution.measured.into(),
            in_process_measured: contribution.in_process_measured.into(),
            failed: contribution.failed.into(),
            invalid: contribution.invalid.into(),
            skipped_reference: contribution.skipped_reference.into(),
            over_latency_budget: contribution.over_latency_budget.into(),
            over_memory_budget: contribution.over_memory_budget.into(),
        });
        Ok(record)
    }

    fn from_columns(columns: BenchmarkColumns) -> anyhow::Result<Self> {
        Ok(Self {
            id: columns.id,
            status: BenchmarkStatus::from_label(&columns.status)?,
            source: columns.source,
            iterations: columns.iterations,
            case_duration_ns: optional_duration(&columns.case_duration)?,
            engine: Measurement::from_columns(
                &columns.engine_measurement,
                &columns.engine_median,
                &columns.engine_cv,
            )?,
            reference: Measurement::from_columns(
                &columns.reference_measurement,
                &columns.reference_median,
                &columns.reference_cv,
            )?,
            latency_ratio_centi_units: optional_ratio(&columns.latency_ratio)?,
            memory_ratio_centi_units: optional_ratio(&columns.memory_ratio)?,
            latency_budget: BudgetStatus::from_label(&columns.latency_budget)?,
            quality: QualityStatus::from_label(&columns.quality)?,
            methodology: columns.methodology,
            count_contribution: None,
            detail: columns.detail,
        })
    }
}

impl Measurement {
    fn from_columns(measurement: &str, median: &str, cv: &str) -> anyhow::Result<Self> {
        Ok(Self {
            availability: MeasurementAvailability::from_label(median)?,
            wall_duration_ns: optional_duration(measurement)?,
            median_duration_ns: optional_duration(median)?,
            coefficient_variation_permille: optional_cv_permille(cv)?,
        })
    }
}

impl CaseStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Passed => STATUS_PASSED,
            Self::Failed => STATUS_FAILED,
            Self::Skipped => STATUS_SKIPPED,
        }
    }

    fn from_label(label: &str) -> anyhow::Result<Self> {
        match label {
            STATUS_PASSED => Ok(Self::Passed),
            STATUS_FAILED => Ok(Self::Failed),
            STATUS_SKIPPED => Ok(Self::Skipped),
            _ => bail!("unknown case status '{label}'"),
        }
    }
}

impl BenchmarkStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Measured => "✅ measured",
            Self::WithinBudget => "✅ within budget",
            Self::TrackedException => "🟡 tracked exception",
            Self::Failed => "❌ failed",
            Self::Invalid => "❌ invalid benchmark",
            Self::Skipped => "🟡 skipped",
        }
    }

    fn from_label(label: &str) -> anyhow::Result<Self> {
        match label {
            "✅ measured" => Ok(Self::Measured),
            "✅ within budget" => Ok(Self::WithinBudget),
            "🟡 tracked exception" => Ok(Self::TrackedException),
            "❌ failed" => Ok(Self::Failed),
            "❌ invalid benchmark" => Ok(Self::Invalid),
            "🟡 skipped" => Ok(Self::Skipped),
            _ => bail!("unknown benchmark status '{label}'"),
        }
    }
}

impl MeasurementAvailability {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Measured | Self::NotMeasured => NO_VALUE,
            Self::NotConfigured => "🟡 not configured",
            Self::NotAvailable => "🟡 not available",
        }
    }

    fn from_label(label: &str) -> anyhow::Result<Self> {
        if parse_duration(label).is_some() {
            return Ok(Self::Measured);
        }
        match label {
            NO_VALUE => Ok(Self::NotMeasured),
            "🟡 not configured" => Ok(Self::NotConfigured),
            "🟡 not available" => Ok(Self::NotAvailable),
            _ => bail!("unknown measurement availability '{label}'"),
        }
    }
}

impl BudgetStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Within => "✅ <= 1.00x",
            Self::Over => "🟡 > 1.00x",
            Self::Unavailable => "🟡 unavailable",
            Self::NotConfigured => "🟡 no reference",
            Self::Invalid => "❌ invalid",
            Self::NotMeasured => NO_VALUE,
        }
    }

    fn from_label(label: &str) -> anyhow::Result<Self> {
        match label {
            "✅ <= 1.00x" => Ok(Self::Within),
            "🟡 > 1.00x" => Ok(Self::Over),
            "🟡 unavailable" => Ok(Self::Unavailable),
            "🟡 no reference" => Ok(Self::NotConfigured),
            "❌ invalid" => Ok(Self::Invalid),
            NO_VALUE => Ok(Self::NotMeasured),
            _ => bail!("unknown latency budget status '{label}'"),
        }
    }
}

impl QualityStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Valid => "✅ valid",
            Self::Invalid => "❌ invalid",
            Self::NotMeasured => NO_VALUE,
        }
    }

    fn from_label(label: &str) -> anyhow::Result<Self> {
        match label {
            "✅ valid" => Ok(Self::Valid),
            "❌ invalid" => Ok(Self::Invalid),
            NO_VALUE => Ok(Self::NotMeasured),
            _ => bail!("unknown measurement quality status '{label}'"),
        }
    }
}
