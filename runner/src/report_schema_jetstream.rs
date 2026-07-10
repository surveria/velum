use anyhow::bail;
use serde::{Deserialize, Serialize};

use super::{
    BenchmarkCounts, BenchmarkRecord, BenchmarkStatus, BenchmarkSuite, BudgetStatus,
    CaseDetailCoverage, DetailCompleteness, DetailLevel, MeasurementAvailability, NO_VALUE,
    QualityStatus, ReportDocument, SCHEMA_VERSION, is_default,
};

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct JetStreamSuite {
    pub(crate) name: String,
    pub(crate) duration_ns: u64,
    pub(crate) counts: JetStreamCounts,
    pub(crate) row_details: CaseDetailCoverage,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rows: Vec<JetStreamRecord>,
    #[serde(skip)]
    pub(crate) details: Vec<JetStreamDetailRecord>,
}

impl Default for JetStreamSuite {
    fn default() -> Self {
        Self {
            name: "JetStream Shell Benchmarks".to_owned(),
            duration_ns: 0,
            counts: JetStreamCounts::default(),
            row_details: CaseDetailCoverage {
                completeness: DetailCompleteness::Complete,
                recorded_rows: 0,
                omitted_rows: 0,
            },
            rows: Vec::new(),
            details: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct JetStreamCounts {
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) total: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) measured: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) failed: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) invalid: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) skipped: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) unavailable_reference: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) missing_reference: u64,
    #[serde(skip_serializing_if = "is_default")]
    pub(crate) over_latency_budget: u64,
}

#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct JetStreamRecord {
    pub(crate) id: String,
    pub(crate) status: BenchmarkStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) engine_median_duration_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) engine_cv_permille: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reference_median_duration_ns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reference_cv_permille: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) latency_ratio_centi_units: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) reference_source: Option<ReferenceSource>,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JetStreamDetailRecord {
    pub(crate) id: String,
    pub(crate) case_duration_ns: Option<u64>,
    pub(crate) engine_wall_duration_ns: Option<u64>,
    pub(crate) reference_wall_duration_ns: Option<u64>,
    pub(crate) latency_budget: BudgetStatus,
    pub(crate) quality: QualityStatus,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum JetStreamSuiteWire {
    Compact(CompactJetStreamSuite),
    Legacy(LegacyJetStreamSuite),
}

#[derive(Deserialize)]
struct CompactJetStreamSuite {
    name: String,
    duration_ns: u64,
    counts: JetStreamCounts,
    row_details: CaseDetailCoverage,
    #[serde(default)]
    rows: Vec<JetStreamRecord>,
}

#[derive(Deserialize)]
struct LegacyJetStreamSuite {
    name: String,
    duration_ns: u64,
    counts: BenchmarkCounts,
    #[serde(default)]
    rows: Vec<BenchmarkRecord>,
}

impl<'de> Deserialize<'de> for JetStreamSuite {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match JetStreamSuiteWire::deserialize(deserializer)? {
            JetStreamSuiteWire::Compact(suite) => Ok(Self {
                name: suite.name,
                duration_ns: suite.duration_ns,
                counts: suite.counts,
                row_details: suite.row_details,
                rows: suite.rows,
                details: Vec::new(),
            }),
            JetStreamSuiteWire::Legacy(suite) => {
                Self::from_legacy(suite).map_err(serde::de::Error::custom)
            }
        }
    }
}
use crate::{
    jetstream,
    report_benchmark_methodology::ReferenceSource,
    report_composition::ReportComponent,
    report_metadata::RunMetadata,
    report_schema::{EnvironmentInfo, RunConfiguration},
    report_schema_support::{duration_ns, optional_cv_permille, optional_ratio, parse_duration},
};

impl ReportDocument {
    pub fn from_jetstream_run(
        report: &jetstream::JetStreamReport,
        metadata: RunMetadata,
        environment: EnvironmentInfo,
        configuration: RunConfiguration,
    ) -> anyhow::Result<Self> {
        let duration_ns = duration_ns(report.elapsed);
        let component = ReportComponent::capture(
            configuration.report_mode,
            &metadata,
            &environment,
            &configuration,
            duration_ns,
        );
        let document = Self {
            schema_version: SCHEMA_VERSION,
            detail_level: DetailLevel::Full,
            metadata,
            environment,
            configuration,
            components: vec![component],
            duration_ns,
            suites: Vec::new(),
            benchmarks: BenchmarkSuite::empty(),
            jetstream: JetStreamSuite::from_run(report)?,
        };
        document.validate()?;
        Ok(document)
    }
}

impl BenchmarkSuite {
    pub(super) const fn empty() -> Self {
        Self {
            name: String::new(),
            duration_ns: 0,
            counts: BenchmarkCounts {
                measured: 0,
                in_process_measured: 0,
                failed: 0,
                invalid: 0,
                skipped_reference: 0,
                over_latency_budget: 0,
                over_memory_budget: 0,
            },
            rows: Vec::new(),
        }
    }
}

impl JetStreamSuite {
    pub const fn is_empty(&self) -> bool {
        self.counts.total == 0 && self.rows.is_empty()
    }

    pub(super) fn from_jetstream(report: &jetstream::JetStreamReport) -> anyhow::Result<Self> {
        Self::from_run(report)
    }

    fn from_run(report: &jetstream::JetStreamReport) -> anyhow::Result<Self> {
        let total = u64::try_from(report.rows.len())?;
        let rows = report
            .rows
            .iter()
            .map(JetStreamRecord::from_run)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let details = report
            .rows
            .iter()
            .map(JetStreamDetailRecord::from_run)
            .collect::<anyhow::Result<Vec<_>>>()?;
        let counts = derived_counts(&rows)?;
        if counts.measured != u64::try_from(report.measured)?
            || counts.failed != u64::try_from(report.failed)?
            || counts.invalid != u64::try_from(report.invalid)?
            || counts.unavailable_reference != u64::try_from(report.skipped)?
            || counts.missing_reference != u64::try_from(report.reference_missing)?
            || counts.over_latency_budget != u64::try_from(report.over_latency_budget)?
        {
            bail!("JetStream row outcomes do not match their source aggregates");
        }
        Ok(Self {
            name: "JetStream Shell Benchmarks".to_owned(),
            duration_ns: duration_ns(report.elapsed),
            counts,
            row_details: CaseDetailCoverage {
                completeness: DetailCompleteness::Complete,
                recorded_rows: total,
                omitted_rows: 0,
            },
            rows,
            details,
        })
    }

    pub(crate) fn derived_counts(&self) -> anyhow::Result<JetStreamCounts> {
        derived_counts(&self.rows)
    }

    fn from_legacy(suite: LegacyJetStreamSuite) -> anyhow::Result<Self> {
        let LegacyJetStreamSuite {
            name,
            duration_ns,
            counts: legacy_counts,
            rows,
        } = suite;
        let rows = rows
            .into_iter()
            .map(JetStreamRecord::from_legacy)
            .collect::<Vec<_>>();
        let counts = derived_counts(&rows)?;
        let legacy_skipped_reference = legacy_counts.skipped_reference;
        if counts.measured != legacy_counts.measured
            || counts.failed != legacy_counts.failed
            || counts.invalid != legacy_counts.invalid
            || counts.unavailable_reference != legacy_skipped_reference
            || counts.over_latency_budget != legacy_counts.over_latency_budget
        {
            bail!("legacy JetStream rows do not match their aggregate counts");
        }
        Ok(Self {
            name,
            duration_ns,
            counts,
            row_details: CaseDetailCoverage {
                completeness: DetailCompleteness::Complete,
                recorded_rows: u64::try_from(rows.len())?,
                omitted_rows: 0,
            },
            rows,
            details: Vec::new(),
        })
    }
}

fn derived_counts(rows: &[JetStreamRecord]) -> anyhow::Result<JetStreamCounts> {
    let mut counts = JetStreamCounts {
        total: u64::try_from(rows.len())?,
        measured: 0,
        failed: 0,
        invalid: 0,
        skipped: 0,
        unavailable_reference: 0,
        missing_reference: 0,
        over_latency_budget: 0,
    };
    for row in rows {
        counts.measured =
            checked_increment(counts.measured, row.engine_median_duration_ns.is_some())?;
        counts.failed = checked_increment(
            counts.failed,
            matches!(
                row.status,
                BenchmarkStatus::Failed | BenchmarkStatus::Invalid
            ),
        )?;
        counts.invalid = checked_increment(counts.invalid, row.status == BenchmarkStatus::Invalid)?;
        counts.skipped = checked_increment(counts.skipped, row.status == BenchmarkStatus::Skipped)?;
        counts.unavailable_reference = checked_increment(
            counts.unavailable_reference,
            row.status == BenchmarkStatus::Skipped
                || row.engine_median_duration_ns.is_some()
                    && row.reference_median_duration_ns.is_none(),
        )?;
        counts.missing_reference = checked_increment(
            counts.missing_reference,
            row.reference_source == Some(ReferenceSource::QuickjsBaselineMissing),
        )?;
        counts.over_latency_budget = checked_increment(
            counts.over_latency_budget,
            row.status == BenchmarkStatus::TrackedException,
        )?;
    }
    Ok(counts)
}

fn checked_increment(value: u64, condition: bool) -> anyhow::Result<u64> {
    if !condition {
        return Ok(value);
    }
    value
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("JetStream aggregate count overflows"))
}

impl JetStreamRecord {
    fn from_run(row: &jetstream::JetStreamRow) -> anyhow::Result<Self> {
        let reference_source = reference_source(&row.reference_source, &row.quickjs_time)?;
        Ok(Self {
            id: row.benchmark.clone(),
            status: BenchmarkStatus::from_label(&row.status)?,
            source: (row.source != NO_VALUE).then(|| row.source.clone()),
            engine_median_duration_ns: measurement_duration(&row.rsqjs_time)?,
            engine_cv_permille: optional_cv_permille(&row.rsqjs_cv)?,
            reference_median_duration_ns: measurement_duration(&row.quickjs_time)?,
            reference_cv_permille: optional_cv_permille(&row.quickjs_cv)?,
            latency_ratio_centi_units: optional_ratio(&row.latency_ratio)?,
            reference_source,
            detail: row.detail.clone(),
        })
    }

    fn from_legacy(row: BenchmarkRecord) -> Self {
        let reference_source = match row.reference.availability {
            MeasurementAvailability::Measured => Some(ReferenceSource::QuickjsLive),
            MeasurementAvailability::NotConfigured => Some(ReferenceSource::NotConfigured),
            MeasurementAvailability::NotAvailable => Some(ReferenceSource::QuickjsLiveFailed),
            MeasurementAvailability::NotMeasured => None,
        };
        Self {
            id: row.id,
            status: row.status,
            source: (row.source != NO_VALUE).then_some(row.source),
            engine_median_duration_ns: row.engine.median_duration_ns,
            engine_cv_permille: row.engine.coefficient_variation_permille,
            reference_median_duration_ns: row.reference.median_duration_ns,
            reference_cv_permille: row.reference.coefficient_variation_permille,
            latency_ratio_centi_units: row.latency_ratio_centi_units,
            reference_source,
            detail: row.detail,
        }
    }
}

impl JetStreamDetailRecord {
    fn from_run(row: &jetstream::JetStreamRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: row.benchmark.clone(),
            case_duration_ns: wall_duration(&row.case_elapsed)?,
            engine_wall_duration_ns: wall_duration(&row.rsqjs_measure)?,
            reference_wall_duration_ns: wall_duration(&row.quickjs_measure)?,
            latency_budget: BudgetStatus::from_label(&row.latency_budget)?,
            quality: QualityStatus::from_label(&row.quality)?,
        })
    }
}

fn reference_source(
    value: &str,
    reference_median: &str,
) -> anyhow::Result<Option<ReferenceSource>> {
    match value {
        jetstream::REFERENCE_SOURCE_BASELINE => Ok(Some(ReferenceSource::QuickjsBaseline)),
        jetstream::REFERENCE_SOURCE_LIVE => {
            if parse_duration(reference_median).is_some() {
                Ok(Some(ReferenceSource::QuickjsLive))
            } else {
                Ok(Some(ReferenceSource::QuickjsLiveFailed))
            }
        }
        jetstream::REFERENCE_SOURCE_MISSING => Ok(Some(ReferenceSource::QuickjsBaselineMissing)),
        jetstream::REFERENCE_SOURCE_DISABLED => Ok(Some(ReferenceSource::NotConfigured)),
        NO_VALUE => Ok(None),
        _ => bail!("unknown JetStream reference source '{value}'"),
    }
}

fn measurement_duration(value: &str) -> anyhow::Result<Option<u64>> {
    if let Some(duration) = parse_duration(value) {
        return Ok(Some(duration));
    }
    match value {
        NO_VALUE
        | jetstream::REFERENCE_NOT_AVAILABLE
        | jetstream::REFERENCE_BASELINE_MISSING
        | jetstream::REFERENCE_NOT_CONFIGURED => Ok(None),
        _ => bail!("unknown JetStream measurement value '{value}'"),
    }
}

fn wall_duration(value: &str) -> anyhow::Result<Option<u64>> {
    if value == jetstream::REFERENCE_MEASURE_CACHED {
        return Ok(None);
    }
    measurement_duration(value)
}

#[cfg(test)]
mod tests {
    use super::{measurement_duration, wall_duration};

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn accepts_all_non_numeric_jetstream_reference_outcomes() -> TestResult {
        for value in [
            "-",
            "🟡 not available",
            "🟡 baseline missing",
            "🟡 not configured",
        ] {
            if measurement_duration(value)?.is_some() {
                return Err(format!("'{value}' unexpectedly produced a duration").into());
            }
        }
        if wall_duration("cached; not run")?.is_none() {
            return Ok(());
        }
        Err("cached reference wall time unexpectedly produced a duration".into())
    }
}
