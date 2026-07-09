use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use serde::{Deserialize, Serialize};

use crate::{
    CaseRow, CorpusReport, FullReport, STATUS_FAILED, STATUS_PASSED, STATUS_SKIPPED, benchmarks,
    jetstream,
    report_metadata::RunMetadata,
    report_schema_support::{
        duration_ns, labeled_count, optional_cv_permille, optional_duration, optional_ratio,
        parse_duration, usize_to_u64,
    },
};

pub const SCHEMA_VERSION: u32 = 1;
pub const TEST262_FULL_SUITE: &str = "Test262 full corpus";

pub const NO_VALUE: &str = "-";

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportDocument {
    pub(crate) schema_version: u32,
    pub(crate) detail_level: DetailLevel,
    pub(crate) metadata: RunMetadata,
    pub(crate) environment: EnvironmentInfo,
    pub(crate) configuration: RunConfiguration,
    pub(crate) duration_ns: u64,
    pub(crate) suites: Vec<SuiteReport>,
    pub(crate) benchmarks: BenchmarkSuite,
    pub(crate) jetstream: BenchmarkSuite,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReportSummary {
    pub(crate) schema_version: u32,
    pub(crate) detail_level: DetailLevel,
    pub(crate) metadata: RunMetadata,
    pub(crate) environment: EnvironmentInfo,
    pub(crate) configuration: RunConfiguration,
    pub(crate) duration_ns: u64,
    pub(crate) suites: Vec<SuiteSummary>,
    pub(crate) benchmarks: BenchmarkSuite,
    pub(crate) jetstream: BenchmarkSuite,
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailLevel {
    Full,
    Summary,
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
    pub(crate) quickjs_differential_configured: bool,
    pub(crate) test262_configured: bool,
    pub(crate) test262_run_all: bool,
    pub(crate) test262_path_filters: Vec<String>,
    pub(crate) test262_flag_filters: Vec<String>,
    pub(crate) benchmark_filter: Option<String>,
    pub(crate) benchmark: BenchmarkConfiguration,
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
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuiteReport {
    pub(crate) summary: SuiteSummary,
    pub(crate) cases: Vec<CaseRecord>,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuiteSummary {
    pub(crate) name: String,
    pub(crate) required: bool,
    pub(crate) status: SuiteStatus,
    pub(crate) counts: CaseCounts,
    pub(crate) duration_ns: u64,
    pub(crate) skip_reasons: Vec<SkipReasonSummary>,
    pub(crate) feature_areas: Vec<FeatureAreaSummary>,
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
    pub(crate) total: u64,
    pub(crate) executed: u64,
    pub(crate) passed: u64,
    pub(crate) failed: u64,
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
    pub(crate) counts: CaseCounts,
    pub(crate) manifest_enabled: u64,
    pub(crate) top_skip_reason: String,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct BenchmarkSuite {
    pub(crate) name: String,
    pub(crate) duration_ns: u64,
    pub(crate) counts: BenchmarkCounts,
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
    pub(crate) memory_ratio_centi_units: Option<u64>,
    pub(crate) latency_budget: BudgetStatus,
    pub(crate) quality: QualityStatus,
    pub(crate) detail: String,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct YamlArtifactPaths {
    pub(crate) summary: PathBuf,
    pub(crate) details: PathBuf,
}

struct BenchmarkColumns {
    id: String,
    status: String,
    source: String,
    iterations: Option<u64>,
    case_duration: String,
    engine_measurement: String,
    engine_median: String,
    engine_cv: String,
    reference_measurement: String,
    reference_median: String,
    reference_cv: String,
    latency_ratio: String,
    memory_ratio: String,
    latency_budget: String,
    quality: String,
    detail: String,
}

impl ReportDocument {
    pub fn from_run(
        report: FullReport,
        environment: EnvironmentInfo,
        configuration: RunConfiguration,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            schema_version: SCHEMA_VERSION,
            detail_level: DetailLevel::Full,
            metadata: report.metadata,
            environment,
            configuration,
            duration_ns: duration_ns(report.elapsed),
            suites: report
                .corpora
                .into_iter()
                .map(SuiteReport::from_run)
                .collect::<anyhow::Result<Vec<_>>>()?,
            benchmarks: BenchmarkSuite::from_benchmarks(report.benchmarks)?,
            jetstream: BenchmarkSuite::from_jetstream(report.jetstream)?,
        })
    }

    pub fn summary(&self) -> ReportSummary {
        ReportSummary {
            schema_version: self.schema_version,
            detail_level: DetailLevel::Summary,
            metadata: self.metadata.clone(),
            environment: self.environment.clone(),
            configuration: self.configuration.clone(),
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

    pub fn failed_count(&self) -> u64 {
        let suite_failures = self
            .suites
            .iter()
            .filter(|suite| suite.summary.required)
            .map(|suite| suite.summary.counts.failed)
            .fold(0u64, u64::saturating_add);
        suite_failures.saturating_add(self.benchmarks.counts.failed)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        validate_schema(self.schema_version, self.detail_level, DetailLevel::Full)
    }
}

impl ReportSummary {
    pub fn validate(&self) -> anyhow::Result<()> {
        validate_schema(self.schema_version, self.detail_level, DetailLevel::Summary)
    }

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
        );
        let status = suite_status(counts);
        let cases = corpus
            .rows
            .into_iter()
            .map(CaseRecord::from_run)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let skip_reasons = corpus
            .skip_reasons
            .into_iter()
            .map(|row| SkipReasonSummary {
                skipped: usize_to_u64(row.skipped),
                reason: row.reason,
            })
            .collect();
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
                duration_ns: duration_ns(corpus.elapsed),
                skip_reasons,
                feature_areas,
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

    fn from_jetstream(report: jetstream::JetStreamReport) -> anyhow::Result<Self> {
        Ok(Self {
            name: "JetStream Shell Benchmarks".to_owned(),
            duration_ns: duration_ns(report.elapsed),
            counts: BenchmarkCounts {
                measured: usize_to_u64(report.measured),
                in_process_measured: usize_to_u64(report.measured),
                failed: usize_to_u64(report.failed),
                invalid: usize_to_u64(report.invalid),
                skipped_reference: usize_to_u64(report.skipped),
                over_latency_budget: usize_to_u64(report.over_latency_budget),
                over_memory_budget: 0,
            },
            rows: report
                .rows
                .into_iter()
                .map(BenchmarkRecord::from_jetstream)
                .collect::<anyhow::Result<Vec<_>>>()?,
        })
    }
}

impl BenchmarkRecord {
    fn from_benchmark(row: benchmarks::BenchmarkRow) -> anyhow::Result<Self> {
        Self::from_columns(BenchmarkColumns {
            id: row.benchmark,
            status: row.status,
            source: row.source,
            iterations: Some(usize_to_u64(row.iterations)),
            case_duration: row.case_elapsed,
            engine_measurement: row.rsqjs_measure,
            engine_median: row.rsqjs_eval,
            engine_cv: row.rsqjs_cv,
            reference_measurement: row.quickjs_measure,
            reference_median: row.quickjs_eval,
            reference_cv: row.quickjs_cv,
            latency_ratio: row.latency_ratio,
            memory_ratio: row.memory_ratio,
            latency_budget: row.latency_budget,
            quality: row.quality,
            detail: row.detail,
        })
    }

    fn from_jetstream(row: jetstream::JetStreamRow) -> anyhow::Result<Self> {
        Self::from_columns(BenchmarkColumns {
            id: row.benchmark,
            status: row.status,
            source: row.source,
            iterations: None,
            case_duration: row.case_elapsed,
            engine_measurement: row.rsqjs_measure,
            engine_median: row.rsqjs_time,
            engine_cv: row.rsqjs_cv,
            reference_measurement: row.quickjs_measure,
            reference_median: row.quickjs_time,
            reference_cv: row.quickjs_cv,
            latency_ratio: row.latency_ratio,
            memory_ratio: NO_VALUE.to_owned(),
            latency_budget: row.latency_budget,
            quality: row.quality,
            detail: row.detail,
        })
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

pub fn write_yaml_artifacts(
    report_path: &Path,
    report: &ReportDocument,
) -> anyhow::Result<YamlArtifactPaths> {
    report.validate()?;
    let paths = yaml_artifact_paths(report_path);
    write_yaml(&paths.summary, &report.summary())?;
    write_yaml(&paths.details, report)?;
    Ok(paths)
}

pub fn read_summary(path: &Path) -> anyhow::Result<ReportSummary> {
    let file = File::open(path)
        .with_context(|| format!("failed to open YAML report '{}'", path.display()))?;
    let report: ReportSummary = serde_yaml_ng::from_reader(BufReader::new(file))
        .with_context(|| format!("failed to parse YAML report '{}'", path.display()))?;
    report.validate()?;
    Ok(report)
}

pub fn yaml_artifact_paths(report_path: &Path) -> YamlArtifactPaths {
    let stem = report_path
        .file_stem()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("rsqjs-test-report");
    YamlArtifactPaths {
        summary: report_path.with_extension("yaml"),
        details: report_path.with_file_name(format!("{stem}-details.yaml")),
    }
}

fn write_yaml<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create YAML report directory '{}'",
                parent.display()
            )
        })?;
    }
    let file = File::create(path)
        .with_context(|| format!("failed to create YAML report '{}'", path.display()))?;
    serde_yaml_ng::to_writer(BufWriter::new(file), value)
        .with_context(|| format!("failed to write YAML report '{}'", path.display()))
}

fn validate_schema(
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

const fn suite_status(counts: CaseCounts) -> SuiteStatus {
    if counts.failed > 0 {
        return SuiteStatus::Failed;
    }
    if counts.executed == 0 && counts.skipped > 0 {
        return SuiteStatus::Skipped;
    }
    SuiteStatus::Passed
}

fn case_counts(total: usize, passed: usize, failed: usize, skipped: usize) -> CaseCounts {
    CaseCounts {
        total: usize_to_u64(total),
        executed: usize_to_u64(passed.saturating_add(failed)),
        passed: usize_to_u64(passed),
        failed: usize_to_u64(failed),
        skipped: usize_to_u64(skipped),
    }
}
