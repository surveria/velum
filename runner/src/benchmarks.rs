//! In-process benchmark harness.
//!
//! Both the engine under test and the optional `QuickJS` reference are timed by
//! the same sampler (`bench_measure::measure`) through the `BenchEngine`
//! interface, so the latency ratio reflects execution with no process-startup
//! component on either side and the runner owns every measurement. The CLI /
//! startup-subtraction machinery it replaces has been removed.
//!
//! Peak-memory comparison used to piggyback on a CLI process; a byte-level
//! in-process memory parity needs per-VM heap accounting that rs-quickjs does
//! not expose yet, so the `memory_ratio` column is reserved (`-`) for now.

use std::fs;

use tabled::Tabled;

use super::bench_engines::{BenchEngine, RsqjsEngine, make_reference};
use super::bench_measure::{self, MeasureConfig, MeasureStats, format_duration, ratio_values};
use super::benchmark_protocol::{
    BenchmarkContributionFlag, BenchmarkCountContribution, BenchmarkInput, BenchmarkMethodology,
    BenchmarkReferenceSource, ReportedLifecycle,
};
use super::benchmark_selection::BenchmarkSelection;
use super::cases::{self, BenchmarkCase};
use super::quickjs_baseline::{QuickjsBaseline, detect_host_profile};
use super::{prepared_benchmarks, report_text, timing};

#[path = "benchmark_configuration_report.rs"]
mod configuration_report;
#[path = "prepared_benchmark_report.rs"]
mod prepared_report;

use configuration_report::{configuration_failure_outcome, configuration_failure_report};

pub const BUDGET_LABEL: &str = "1.00x";

const BUDGET_NUMERATOR: u128 = 100;
const BUDGET_DENOMINATOR: u128 = 100;
const STATUS_MEASURED: &str = "✅ measured";
const STATUS_FAILED: &str = "❌ failed";
const STATUS_INVALID_BENCHMARK: &str = "❌ invalid benchmark";
const STATUS_TRACKED_EXCEPTION: &str = "🟡 tracked exception";
const STATUS_WITHIN_BUDGET: &str = "✅ within budget";
const BUDGET_WITHIN: &str = "✅ <= 1.00x";
const BUDGET_OVER: &str = "🟡 > 1.00x";
const BUDGET_INVALID: &str = "❌ invalid";
const BUDGET_NOT_AVAILABLE: &str = "🟡 unavailable";
const BUDGET_NOT_CONFIGURED: &str = "🟡 no reference";
const QUALITY_VALID: &str = "✅ valid";
const QUALITY_INVALID: &str = "❌ invalid";
const REFERENCE_NOT_CONFIGURED: &str = "🟡 not configured";
const REFERENCE_NOT_AVAILABLE: &str = "🟡 not available";
const NOT_MEASURED: &str = "-";
const DETAIL_COMPLETED: &str = "sequential benchmark completed";
const DETAIL_LATENCY_EXCEPTION: &str = "latency budget exception tracked";
const DETAIL_QUALITY_GATE: &str = "measurement quality gate failed";
const DETAIL_REFERENCE_COMPLETED: &str = "QuickJS reference completed";
const MAX_BENCHMARK_DETAIL_CHARS: usize = 240;

#[derive(Debug)]
pub struct BenchmarkReport {
    pub rows: Vec<BenchmarkRow>,
    pub measured: usize,
    pub in_process_measured: usize,
    pub failed: usize,
    pub invalid: usize,
    pub skipped: usize,
    pub over_latency_budget: usize,
    pub over_memory_budget: usize,
    pub elapsed: std::time::Duration,
}

impl BenchmarkReport {
    #[must_use]
    pub const fn not_run() -> Self {
        Self {
            rows: Vec::new(),
            measured: 0,
            in_process_measured: 0,
            failed: 0,
            invalid: 0,
            skipped: 0,
            over_latency_budget: 0,
            over_memory_budget: 0,
            elapsed: std::time::Duration::ZERO,
        }
    }
}

#[derive(Debug, Tabled)]
pub struct BenchmarkRow {
    pub(crate) benchmark: String,
    pub(crate) status: String,
    pub(crate) source: String,
    pub(crate) iterations: usize,
    pub(crate) case_elapsed: String,
    pub(crate) rsqjs_measure: String,
    pub(crate) quickjs_measure: String,
    pub(crate) rsqjs_eval: String,
    pub(crate) quickjs_eval: String,
    pub(crate) latency_ratio: String,
    pub(crate) latency_budget: String,
    pub(crate) memory_ratio: String,
    pub(crate) rsqjs_cv: String,
    pub(crate) quickjs_cv: String,
    pub(crate) quality: String,
    pub(crate) detail: String,
    pub(crate) mode: String,
    pub(crate) lifecycle: String,
    pub(crate) checksum: String,
    pub(crate) reference_source: String,
    #[tabled(skip)]
    pub(crate) methodology: BenchmarkMethodology,
    #[tabled(skip)]
    pub(crate) count_contribution: BenchmarkCountContribution,
}

#[derive(Debug, Clone, Copy, Default)]
struct BenchmarkCounts {
    measured: usize,
    in_process_measured: usize,
    failed: usize,
    invalid: usize,
    skipped: usize,
    over_latency_budget: usize,
    over_memory_budget: usize,
}

#[derive(Debug)]
struct BenchmarkOutcome {
    row: BenchmarkRow,
    counts: BenchmarkCounts,
}

#[derive(Debug)]
enum ReferenceMeasurement {
    NotConfigured,
    Measured(timing::Timed<MeasureStats>),
    Failed(timing::Timed<String>),
}

#[derive(Debug, Clone, Copy)]
struct BudgetCheck {
    label: &'static str,
    over_budget: bool,
}

#[must_use]
pub fn run() -> BenchmarkReport {
    let timer = timing::RunTimer::start();
    let config = MeasureConfig::in_process_from_env();
    let selection = match BenchmarkSelection::from_env() {
        Ok(selection) => selection,
        Err(error) => return configuration_failure_report(timer.elapsed(), &error.to_string()),
    };
    let selected_cases = match selection.select(cases::benchmark_cases()) {
        Ok(cases) => cases,
        Err(error) => return configuration_failure_report(timer.elapsed(), &error.to_string()),
    };
    let mut baseline = match QuickjsBaseline::from_env() {
        Ok(baseline) => baseline,
        Err(error) => return configuration_failure_report(timer.elapsed(), &error.to_string()),
    };
    let host_profile = detect_host_profile();
    let reference = make_reference();
    let mut report = BenchmarkReport::not_run();
    for case in selected_cases {
        let outcome = run_benchmark_case(
            &case,
            config,
            reference.as_deref(),
            &mut baseline,
            &host_profile,
        );
        push_outcome(&mut report, outcome);
    }
    if let Err(error) = baseline.finish() {
        push_outcome(
            &mut report,
            configuration_failure_outcome(&error.to_string()),
        );
    }
    report.elapsed = timer.elapsed();
    report
}

fn push_outcome(report: &mut BenchmarkReport, mut outcome: BenchmarkOutcome) {
    report.measured = report.measured.saturating_add(outcome.counts.measured);
    report.in_process_measured = report
        .in_process_measured
        .saturating_add(outcome.counts.in_process_measured);
    report.failed = report.failed.saturating_add(outcome.counts.failed);
    report.invalid = report.invalid.saturating_add(outcome.counts.invalid);
    report.skipped = report.skipped.saturating_add(outcome.counts.skipped);
    report.over_latency_budget = report
        .over_latency_budget
        .saturating_add(outcome.counts.over_latency_budget);
    report.over_memory_budget = report
        .over_memory_budget
        .saturating_add(outcome.counts.over_memory_budget);
    outcome.row.count_contribution = BenchmarkCountContribution {
        measured: BenchmarkContributionFlag::from_bool(outcome.counts.measured > 0),
        in_process_measured: BenchmarkContributionFlag::from_bool(
            outcome.counts.in_process_measured > 0,
        ),
        failed: BenchmarkContributionFlag::from_bool(outcome.counts.failed > 0),
        invalid: BenchmarkContributionFlag::from_bool(outcome.counts.invalid > 0),
        skipped_reference: BenchmarkContributionFlag::from_bool(outcome.counts.skipped > 0),
        over_latency_budget: BenchmarkContributionFlag::from_bool(
            outcome.counts.over_latency_budget > 0,
        ),
        over_memory_budget: BenchmarkContributionFlag::from_bool(
            outcome.counts.over_memory_budget > 0,
        ),
    };
    report.rows.push(outcome.row);
}

fn cold_lifecycle(load: std::time::Duration) -> String {
    format!(
        "load={};compile=per_operation;setup=per_operation;warmup=measured;run=measured;verify=-;teardown=per_operation",
        timing::format_duration(load)
    )
}

fn run_benchmark_case(
    case: &BenchmarkCase,
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
    baseline: &mut QuickjsBaseline,
    host_profile: &str,
) -> BenchmarkOutcome {
    let case_timer = timing::RunTimer::start();
    let loaded = timing::timed(|| fs::read_to_string(case.path));
    let source = match loaded.value {
        Ok(source) => source,
        Err(error) => {
            return failed_outcome(
                case,
                timing::format_duration(case_timer.elapsed()),
                &format!("failed to read '{}': {error}", case.path),
            );
        }
    };
    if case.mode.uses_prepared_protocol() {
        return match prepared_benchmarks::run(
            case,
            &source,
            loaded.elapsed,
            config,
            reference,
            baseline,
            host_profile,
        ) {
            Ok(run) => prepared_report::outcome(case, &run),
            Err(error) => failed_outcome(
                case,
                timing::format_duration(case_timer.elapsed()),
                &error.to_string(),
            ),
        };
    }
    let ours = timing::timed(|| {
        bench_measure::measure(config, || eval_benchmark(&RsqjsEngine, case, &source))
    });
    let reference = measure_reference(config, reference, case, &source);
    let reference_source = match &reference {
        ReferenceMeasurement::Measured(_) => BenchmarkReferenceSource::QuickjsLive,
        ReferenceMeasurement::Failed(_) => BenchmarkReferenceSource::QuickjsLiveFailed,
        ReferenceMeasurement::NotConfigured => BenchmarkReferenceSource::NotConfigured,
    };
    let case_elapsed = timing::format_duration(case_timer.elapsed());
    let mut outcome = match ours.value {
        Ok(stats) => measured_with_reference_result(
            case,
            timing::Timed {
                value: stats,
                elapsed: ours.elapsed,
            },
            reference,
            case_elapsed,
        ),
        Err(error) => failed_with_reference(
            case,
            &error.to_string(),
            timing::format_duration(ours.elapsed),
            reference,
            case_elapsed,
        ),
    };
    outcome.row.lifecycle = cold_lifecycle(loaded.elapsed);
    let reference_source_label = match reference_source {
        BenchmarkReferenceSource::QuickjsLive => "quickjs_live",
        BenchmarkReferenceSource::QuickjsLiveFailed => "quickjs_live_failed",
        BenchmarkReferenceSource::NotConfigured => REFERENCE_NOT_CONFIGURED,
        BenchmarkReferenceSource::QuickjsBaseline => "quickjs_baseline",
    };
    reference_source_label.clone_into(&mut outcome.row.reference_source);
    outcome.row.methodology.lifecycle = Some(ReportedLifecycle::cold_eval(loaded.elapsed));
    outcome.row.methodology.reference_source = Some(reference_source);
    outcome
}

fn measure_reference(
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
    case: &BenchmarkCase,
    source: &str,
) -> ReferenceMeasurement {
    let Some(reference) = reference else {
        return ReferenceMeasurement::NotConfigured;
    };
    let measured = timing::timed(|| {
        bench_measure::measure(config, || eval_benchmark(reference, case, source))
    });
    match measured.value {
        Ok(stats) => ReferenceMeasurement::Measured(timing::Timed {
            value: stats,
            elapsed: measured.elapsed,
        }),
        Err(error) => ReferenceMeasurement::Failed(timing::Timed {
            value: format!("{}: {error}", reference.label()),
            elapsed: measured.elapsed,
        }),
    }
}

fn eval_benchmark(
    engine: &dyn BenchEngine,
    case: &BenchmarkCase,
    source: &str,
) -> anyhow::Result<()> {
    if let BenchmarkInput::HostImage { byte_len } = case.input {
        return engine.eval_with_host_image(source, byte_len);
    }
    engine.eval(source)
}

fn measured_with_reference_result(
    case: &BenchmarkCase,
    ours: timing::Timed<MeasureStats>,
    reference: ReferenceMeasurement,
    case_elapsed: String,
) -> BenchmarkOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => {
            measured_with_reference(case, ours, reference, case_elapsed)
        }
        ReferenceMeasurement::Failed(note) => {
            reference_unavailable(case, ours, &note, case_elapsed)
        }
        ReferenceMeasurement::NotConfigured => measured_without_reference(case, ours, case_elapsed),
    }
}

fn failed_with_reference(
    case: &BenchmarkCase,
    detail: &str,
    rsqjs_measure: String,
    reference: ReferenceMeasurement,
    case_elapsed: String,
) -> BenchmarkOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns::failed_with_reference(
                case_elapsed,
                rsqjs_measure,
                reference.elapsed,
            ),
            timing::ReferenceColumns::measured(
                format_duration(reference.value.median()),
                reference.value.cv_percent_text(),
            ),
            reference_quality(reference.value),
            &detail_with_reference_quality(detail, reference.value),
        ),
        ReferenceMeasurement::Failed(note) => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns::failed_with_reference(
                case_elapsed,
                rsqjs_measure,
                note.elapsed,
            ),
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_AVAILABLE),
            NOT_MEASURED.to_owned(),
            &format!("{detail}; reference error: {}", note.value),
        ),
        ReferenceMeasurement::NotConfigured => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns {
                case_elapsed,
                rsqjs_measure,
                quickjs_measure: NOT_MEASURED.to_owned(),
            },
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_CONFIGURED),
            NOT_MEASURED.to_owned(),
            detail,
        ),
    }
}

fn failed_with_reference_measurement(
    case: &BenchmarkCase,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    quality: String,
    row_detail: &str,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: failed_row(
            case,
            measurements,
            NOT_MEASURED.to_owned(),
            quickjs,
            quality,
            row_detail,
        ),
        counts: BenchmarkCounts {
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn reference_quality(reference: MeasureStats) -> String {
    if reference.quality().is_valid() {
        return QUALITY_VALID.to_owned();
    }
    QUALITY_INVALID.to_owned()
}

fn detail_with_reference_quality(detail: &str, reference: MeasureStats) -> String {
    let Some(quality) = reference_quality_failure_detail(reference) else {
        return format!("{detail}; {DETAIL_REFERENCE_COMPLETED}");
    };
    format!("{detail}; {quality}")
}

fn reference_quality_failure_detail(reference: MeasureStats) -> Option<String> {
    let mut reasons = Vec::new();
    collect_quality_reasons(&mut reasons, "quickjs", reference);
    if reasons.is_empty() {
        return None;
    }
    Some(format!("{DETAIL_QUALITY_GATE}: {}", reasons.join("; ")))
}

fn measured_with_reference(
    case: &BenchmarkCase,
    ours: timing::Timed<MeasureStats>,
    reference: timing::Timed<MeasureStats>,
    case_elapsed: String,
) -> BenchmarkOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, Some(reference.value)) {
        let measurements =
            timing::MeasurementColumns::measured(case_elapsed, ours.elapsed, reference.elapsed);
        let quickjs = timing::ReferenceColumns::measured(
            format_duration(reference.value.median()),
            reference.value.cv_percent_text(),
        );
        return invalid_measurement_outcome(case, ours, measurements, quickjs, &detail, false);
    }
    let budget = budget_check(
        ours.value.median().as_nanos(),
        reference.value.median().as_nanos(),
    );
    let over = budget.over_budget;
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: benchmark_status(over).to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours.value),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: timing::format_duration(reference.elapsed),
            rsqjs_eval: format_duration(ours.value.median()),
            quickjs_eval: format_duration(reference.value.median()),
            latency_ratio: ratio_values(
                ours.value.median().as_nanos(),
                reference.value.median().as_nanos(),
            ),
            latency_budget: budget.label.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: reference.value.cv_percent_text(),
            quality: QUALITY_VALID.to_owned(),
            detail: benchmark_detail(&detail_text(over)),
            mode: case.mode.to_string(),
            lifecycle: NOT_MEASURED.to_owned(),
            checksum: NOT_MEASURED.to_owned(),
            reference_source: NOT_MEASURED.to_owned(),
            methodology: BenchmarkMethodology::for_mode(case.mode),
            count_contribution: BenchmarkCountContribution::default(),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            over_latency_budget: count_if(over),
            ..BenchmarkCounts::default()
        },
    }
}

fn measured_without_reference(
    case: &BenchmarkCase,
    ours: timing::Timed<MeasureStats>,
    case_elapsed: String,
) -> BenchmarkOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, None) {
        let measurements =
            timing::MeasurementColumns::without_reference(case_elapsed, ours.elapsed);
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_CONFIGURED),
            &detail,
            true,
        );
    }
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_MEASURED.to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours.value),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: NOT_MEASURED.to_owned(),
            rsqjs_eval: format_duration(ours.value.median()),
            quickjs_eval: REFERENCE_NOT_CONFIGURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: BUDGET_NOT_CONFIGURED.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: benchmark_detail(DETAIL_COMPLETED),
            mode: case.mode.to_string(),
            lifecycle: NOT_MEASURED.to_owned(),
            checksum: NOT_MEASURED.to_owned(),
            reference_source: NOT_MEASURED.to_owned(),
            methodology: BenchmarkMethodology::for_mode(case.mode),
            count_contribution: BenchmarkCountContribution::default(),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            skipped: 1,
            ..BenchmarkCounts::default()
        },
    }
}

/// The engine under test was measured, but the reference could not run this
/// script (e.g. an unsupported construct): report our number without a ratio
/// and note the reason, rather than failing the benchmark.
fn reference_unavailable(
    case: &BenchmarkCase,
    ours: timing::Timed<MeasureStats>,
    note: &timing::Timed<String>,
    case_elapsed: String,
) -> BenchmarkOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, None) {
        let measurements =
            timing::MeasurementColumns::measured(case_elapsed, ours.elapsed, note.elapsed);
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_AVAILABLE),
            &format!("{detail}; reference error: {}", note.value),
            true,
        );
    }
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_MEASURED.to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours.value),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: timing::format_duration(note.elapsed),
            rsqjs_eval: format_duration(ours.value.median()),
            quickjs_eval: REFERENCE_NOT_AVAILABLE.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: BUDGET_NOT_AVAILABLE.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: benchmark_detail(&format!(
                "{DETAIL_COMPLETED}; reference error: {}",
                note.value
            )),
            mode: case.mode.to_string(),
            lifecycle: NOT_MEASURED.to_owned(),
            checksum: NOT_MEASURED.to_owned(),
            reference_source: NOT_MEASURED.to_owned(),
            methodology: BenchmarkMethodology::for_mode(case.mode),
            count_contribution: BenchmarkCountContribution::default(),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            skipped: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn invalid_measurement_outcome(
    case: &BenchmarkCase,
    ours: timing::Timed<MeasureStats>,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    detail: &str,
    skipped_reference: bool,
) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: case.id.to_owned(),
            status: STATUS_INVALID_BENCHMARK.to_owned(),
            source: case.path.to_owned(),
            iterations: report_iterations(ours.value),
            case_elapsed: measurements.case_elapsed,
            rsqjs_measure: measurements.rsqjs_measure,
            quickjs_measure: measurements.quickjs_measure,
            rsqjs_eval: format_duration(ours.value.median()),
            quickjs_eval: quickjs.eval,
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: BUDGET_INVALID.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: quickjs.cv,
            quality: QUALITY_INVALID.to_owned(),
            detail: benchmark_detail(detail),
            mode: case.mode.to_string(),
            lifecycle: NOT_MEASURED.to_owned(),
            checksum: NOT_MEASURED.to_owned(),
            reference_source: NOT_MEASURED.to_owned(),
            methodology: BenchmarkMethodology::for_mode(case.mode),
            count_contribution: BenchmarkCountContribution::default(),
        },
        counts: BenchmarkCounts {
            measured: 1,
            in_process_measured: 1,
            failed: 1,
            invalid: 1,
            skipped: count_if(skipped_reference),
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_outcome(case: &BenchmarkCase, case_elapsed: String, detail: &str) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: failed_row(
            case,
            timing::MeasurementColumns::not_measured(case_elapsed),
            NOT_MEASURED.to_owned(),
            timing::ReferenceColumns::not_measured(NOT_MEASURED),
            NOT_MEASURED.to_owned(),
            detail,
        ),
        counts: BenchmarkCounts {
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}

fn failed_row(
    case: &BenchmarkCase,
    measurements: timing::MeasurementColumns,
    rsqjs_eval: String,
    quickjs: timing::ReferenceColumns,
    quality: String,
    detail: &str,
) -> BenchmarkRow {
    BenchmarkRow {
        benchmark: case.id.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: case.path.to_owned(),
        iterations: 0,
        case_elapsed: measurements.case_elapsed,
        rsqjs_measure: measurements.rsqjs_measure,
        quickjs_measure: measurements.quickjs_measure,
        rsqjs_eval,
        quickjs_eval: quickjs.eval,
        latency_ratio: NOT_MEASURED.to_owned(),
        latency_budget: NOT_MEASURED.to_owned(),
        memory_ratio: NOT_MEASURED.to_owned(),
        rsqjs_cv: NOT_MEASURED.to_owned(),
        quickjs_cv: quickjs.cv,
        quality,
        detail: benchmark_detail(detail),
        mode: case.mode.to_string(),
        lifecycle: NOT_MEASURED.to_owned(),
        checksum: NOT_MEASURED.to_owned(),
        reference_source: NOT_MEASURED.to_owned(),
        methodology: BenchmarkMethodology::for_mode(case.mode),
        count_contribution: BenchmarkCountContribution::default(),
    }
}

fn benchmark_detail(detail: &str) -> String {
    report_text::table_detail_with_limit(detail, MAX_BENCHMARK_DETAIL_CHARS)
}

fn quality_failure_detail(ours: MeasureStats, reference: Option<MeasureStats>) -> Option<String> {
    if ours.quality().is_valid() && reference.is_none_or(|reference| reference.quality().is_valid())
    {
        return None;
    }
    let mut reasons = Vec::new();
    collect_quality_reasons(&mut reasons, "rsqjs", ours);
    if let Some(reference) = reference {
        collect_quality_reasons(&mut reasons, "quickjs", reference);
    }
    if reasons.is_empty() {
        return None;
    }
    Some(format!("{DETAIL_QUALITY_GATE}: {}", reasons.join("; ")))
}

fn collect_quality_reasons(reasons: &mut Vec<String>, label: &str, stats: MeasureStats) {
    let quality = stats.quality();
    if quality.low_signal() {
        reasons.push(format!(
            "{label} median {} below minimum {}",
            format_duration(stats.median()),
            format_duration(quality.min_op_time())
        ));
    }
    if quality.high_variance() {
        reasons.push(format!(
            "{label} CV {} exceeds maximum {}",
            stats.cv_percent_text(),
            quality.max_cv_percent_text()
        ));
    }
    if quality.iteration_cap_reached() {
        reasons.push(format!(
            "{label} calibration reached iteration cap; median sample {}",
            format_duration(stats.median_sample())
        ));
    }
}

fn detail_text(over_latency_budget: bool) -> String {
    if over_latency_budget {
        return format!("{DETAIL_COMPLETED}; {DETAIL_LATENCY_EXCEPTION}");
    }
    DETAIL_COMPLETED.to_owned()
}

fn report_iterations(stats: MeasureStats) -> usize {
    usize::try_from(stats.total_iters()).unwrap_or(usize::MAX)
}

const fn budget_check(ours: u128, reference: u128) -> BudgetCheck {
    if reference == 0 {
        return BudgetCheck {
            label: BUDGET_NOT_AVAILABLE,
            over_budget: false,
        };
    }
    let over_budget =
        ours.saturating_mul(BUDGET_DENOMINATOR) > reference.saturating_mul(BUDGET_NUMERATOR);
    BudgetCheck {
        label: if over_budget {
            BUDGET_OVER
        } else {
            BUDGET_WITHIN
        },
        over_budget,
    }
}

const fn benchmark_status(over_latency_budget: bool) -> &'static str {
    if over_latency_budget {
        return STATUS_TRACKED_EXCEPTION;
    }
    STATUS_WITHIN_BUDGET
}

const fn count_if(condition: bool) -> usize {
    if condition { 1 } else { 0 }
}

#[cfg(test)]
#[path = "benchmark_tests.rs"]
mod tests;
