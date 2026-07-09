use std::{env, fs, path::Path, time::Duration};

use anyhow::{Context as _, bail};
use tabled::Table;

use crate::{
    RUNNER_NAME,
    bench_engines::{BenchEngine, REFERENCE_ENGINE_ID, RsqjsEngine, make_reference},
    bench_measure::{self, MeasureConfig, MeasureStats, format_duration, ratio_values},
    fenced_table,
    jetstream_baseline::{BaselineKey, BaselineOutcome, BaselineSample, JetStreamQuickjsBaseline},
    quickjs_baseline::detect_host_profile,
    report_metadata, report_text, timing,
};

#[path = "jetstream_cases.rs"]
mod jetstream_cases;
#[path = "jetstream_model.rs"]
mod jetstream_model;
#[path = "jetstream_selection.rs"]
mod jetstream_selection;
use jetstream_model::{
    BUDGET_DENOMINATOR, BUDGET_NUMERATOR, BudgetCheck, DETAIL_COMPLETED, DETAIL_LATENCY_EXCEPTION,
    DETAIL_QUALITY_GATE, DETAIL_REFERENCE_COMPLETED, JetStreamCase, JetStreamCounts, JetStreamMode,
    JetStreamOutcome, LATENCY_INVALID, LATENCY_NOT_AVAILABLE, LATENCY_OVER, LATENCY_WITHIN,
    NOT_MEASURED, QUALITY_INVALID, QUALITY_VALID, REFERENCE_BASELINE_MISSING,
    REFERENCE_MEASURE_CACHED, REFERENCE_NOT_AVAILABLE, REFERENCE_NOT_CONFIGURED,
    REFERENCE_SOURCE_BASELINE, REFERENCE_SOURCE_DISABLED, REFERENCE_SOURCE_LIVE,
    REFERENCE_SOURCE_MISSING, ReferenceFlags, ReferenceMeasurement, ReferenceSample, SHELL_PRELUDE,
    STATUS_FAILED, STATUS_INVALID_BENCHMARK, STATUS_SKIPPED, STATUS_TRACKED_EXCEPTION,
    STATUS_WITHIN_BUDGET, SYNC_HARNESS,
};
pub use jetstream_model::{BUDGET_LABEL, JetStreamReport, JetStreamRow};

const ENV_SUITE_MAX_SECONDS: &str = "RSQJS_JETSTREAM_SUITE_MAX_SECONDS";
const DEFAULT_READ_SUITE_MAX_SECONDS: u64 = 120;
const DEFAULT_REFRESH_SUITE_MAX_SECONDS: u64 = 900;

pub fn run() -> anyhow::Result<JetStreamReport> {
    let timer = timing::RunTimer::start();
    let config = MeasureConfig::jetstream_from_env();
    let selection = jetstream_selection::JetStreamSelection::from_env()?;
    let selected_cases = selection.select(jetstream_cases::cases())?;
    let host_profile = detect_host_profile();
    let mut baseline = JetStreamQuickjsBaseline::from_env()?;
    let refresh = baseline.requires_live_reference();
    let suite_budget = suite_budget(refresh)?;
    let reference = if refresh {
        let reference = make_reference();
        if reference.is_none() {
            bail!("JetStream QuickJS baseline refresh requires the 'reference-quickjs' feature")
        }
        reference
    } else {
        None
    };
    let mut report = JetStreamReport::not_run();
    for case in selected_cases {
        let outcome = if timer.elapsed() >= suite_budget {
            skipped_outcome(
                case,
                &format!(
                    "JetStream suite wall budget of {} seconds was exhausted before this case",
                    suite_budget.as_secs()
                ),
            )
        } else {
            run_case(
                case,
                config,
                &host_profile,
                &mut baseline,
                reference.as_deref(),
            )?
        };
        report.measured = report.measured.saturating_add(outcome.counts.measured);
        report.failed = report.failed.saturating_add(outcome.counts.failed);
        report.invalid = report.invalid.saturating_add(outcome.counts.invalid);
        report.skipped = report.skipped.saturating_add(outcome.counts.skipped);
        report.over_latency_budget = report
            .over_latency_budget
            .saturating_add(outcome.counts.over_latency_budget);
        report.reference_missing = report
            .reference_missing
            .saturating_add(outcome.counts.reference_missing);
        report.rows.push(outcome.row);
    }
    baseline.finish()?;
    report.elapsed = timer.elapsed();
    Ok(report)
}

fn suite_budget(refresh: bool) -> anyhow::Result<Duration> {
    let default = if refresh {
        DEFAULT_REFRESH_SUITE_MAX_SECONDS
    } else {
        DEFAULT_READ_SUITE_MAX_SECONDS
    };
    let seconds = match env::var(ENV_SUITE_MAX_SECONDS) {
        Ok(value) => value.trim().parse::<u64>().with_context(|| {
            format!("{ENV_SUITE_MAX_SECONDS} must be a non-negative integer number of seconds")
        })?,
        Err(env::VarError::NotPresent) => default,
        Err(error) => return Err(error).context(format!("failed to read {ENV_SUITE_MAX_SECONDS}")),
    };
    Ok(Duration::from_secs(seconds))
}

pub fn render_section(report: &JetStreamReport) -> Vec<String> {
    vec![
        "## JetStream Shell Benchmarks".to_owned(),
        String::new(),
        summary(report),
        String::new(),
        fenced_table(&Table::new(&report.rows)),
        String::new(),
    ]
}

pub fn write_report(
    path: &Path,
    metadata: &report_metadata::RunMetadata,
    report: &JetStreamReport,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create JetStream report directory '{}'",
                parent.display()
            )
        })?;
    }
    fs::write(path, render_markdown(metadata, report))
        .with_context(|| format!("failed to write JetStream report '{}'", path.display()))?;
    println!("JetStream shell benchmark report: {}", path.display());
    Ok(())
}

fn render_markdown(metadata: &report_metadata::RunMetadata, report: &JetStreamReport) -> String {
    let mut sections = vec![
        "# rs-quickjs JetStream Shell Benchmark Report".to_owned(),
        String::new(),
        format!("Generated by {RUNNER_NAME}."),
        String::new(),
    ];
    sections.extend(report_metadata::render_section(metadata));
    sections.extend(render_section(report));
    sections.join("\n")
}

fn summary(report: &JetStreamReport) -> String {
    format!(
        "- Measured: {}\n- Failed candidates: {}\n- Invalid measurements: {}\n- Skipped: {}\n- Missing or stale QuickJS baseline: {}\n- Over latency budget ({}): {}\n- Elapsed: {}",
        report.measured,
        report.failed,
        report.invalid,
        report.skipped,
        report.reference_missing,
        BUDGET_LABEL,
        report.over_latency_budget,
        timing::format_duration(report.elapsed),
    )
}

fn run_case(
    case: &JetStreamCase,
    config: MeasureConfig,
    host_profile: &str,
    baseline: &mut JetStreamQuickjsBaseline,
    reference: Option<&dyn BenchEngine>,
) -> anyhow::Result<JetStreamOutcome> {
    match case.mode {
        JetStreamMode::Skipped(reason) => Ok(skipped_outcome(case, reason)),
        JetStreamMode::Timed => run_timed_case(case, config, host_profile, baseline, reference),
    }
}

fn run_timed_case(
    case: &JetStreamCase,
    config: MeasureConfig,
    host_profile: &str,
    baseline: &mut JetStreamQuickjsBaseline,
    reference: Option<&dyn BenchEngine>,
) -> anyhow::Result<JetStreamOutcome> {
    let case_timer = timing::RunTimer::start();
    let workload = match workload_source(case.files) {
        Ok(source) => source,
        Err(error) => {
            return Ok(failed_outcome(
                case,
                timing::format_duration(case_timer.elapsed()),
                &error.to_string(),
            ));
        }
    };
    let source = benchmark_source_from_workload(&workload);
    let harness = harness_descriptor();
    let baseline_key = BaselineKey::new(
        case.id,
        &workload,
        &harness,
        config,
        REFERENCE_ENGINE_ID,
        host_profile,
    );
    let ours = timing::timed(|| bench_measure::measure(config, || RsqjsEngine.eval(&source)));
    let reference = measure_reference(config, reference, &source, baseline, &baseline_key)?;
    let case_elapsed = timing::format_duration(case_timer.elapsed());
    Ok(match ours.value {
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
    })
}

fn measure_reference(
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
    source: &str,
    baseline: &mut JetStreamQuickjsBaseline,
    baseline_key: &BaselineKey,
) -> anyhow::Result<ReferenceMeasurement> {
    if let Some(outcome) = baseline.lookup(baseline_key) {
        return Ok(match outcome {
            BaselineOutcome::Measured(sample) => ReferenceMeasurement::Measured(ReferenceSample {
                stats: sample.stats(config),
                elapsed: None,
                source: REFERENCE_SOURCE_BASELINE,
            }),
            BaselineOutcome::Unavailable(detail) => ReferenceMeasurement::CachedUnavailable(detail),
        });
    }
    if baseline.is_disabled() {
        return Ok(ReferenceMeasurement::Disabled);
    }
    let Some(reference) = reference else {
        return Ok(ReferenceMeasurement::Missing);
    };
    let measured = timing::timed(|| bench_measure::measure(config, || reference.eval(source)));
    match measured.value {
        Ok(stats) => {
            baseline.record_measured(
                baseline_key.clone(),
                BaselineSample::from_measurement(stats),
            )?;
            Ok(ReferenceMeasurement::Measured(ReferenceSample {
                stats,
                elapsed: Some(measured.elapsed),
                source: REFERENCE_SOURCE_LIVE,
            }))
        }
        Err(error) => {
            let detail = format!("{}: {error}", reference.label());
            baseline.record_unavailable(baseline_key.clone(), &detail)?;
            Ok(ReferenceMeasurement::Failed(timing::Timed {
                value: detail,
                elapsed: measured.elapsed,
            }))
        }
    }
}

fn measured_with_reference_result(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    reference: ReferenceMeasurement,
    case_elapsed: String,
) -> JetStreamOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => {
            measured_with_reference(case, ours, reference, case_elapsed)
        }
        ReferenceMeasurement::Failed(note) => {
            reference_unavailable(case, ours, &note, case_elapsed)
        }
        ReferenceMeasurement::CachedUnavailable(detail) => measured_without_reference(
            case,
            ours,
            case_elapsed,
            REFERENCE_NOT_AVAILABLE,
            REFERENCE_SOURCE_BASELINE,
            Some(&format!("cached reference unavailable: {detail}")),
            false,
        ),
        ReferenceMeasurement::Missing => measured_without_reference(
            case,
            ours,
            case_elapsed,
            REFERENCE_BASELINE_MISSING,
            REFERENCE_SOURCE_MISSING,
            Some("content-addressed QuickJS baseline entry is missing or stale"),
            true,
        ),
        ReferenceMeasurement::Disabled => measured_without_reference(
            case,
            ours,
            case_elapsed,
            REFERENCE_NOT_CONFIGURED,
            REFERENCE_SOURCE_DISABLED,
            None,
            false,
        ),
    }
}

fn failed_with_reference(
    case: &JetStreamCase,
    detail: &str,
    rsqjs_measure: String,
    reference: ReferenceMeasurement,
    case_elapsed: String,
) -> JetStreamOutcome {
    match reference {
        ReferenceMeasurement::Measured(reference) => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns {
                case_elapsed,
                rsqjs_measure,
                quickjs_measure: reference.measure_text(),
            },
            timing::ReferenceColumns::measured(
                format_duration(reference.stats.median()),
                reference.stats.cv_percent_text(),
            ),
            reference_quality(reference.stats),
            reference.source,
            false,
            &detail_with_reference_quality(detail, reference.stats),
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
            REFERENCE_SOURCE_LIVE,
            false,
            &format!("{detail}; reference error: {}", note.value),
        ),
        ReferenceMeasurement::CachedUnavailable(reference_detail) => {
            failed_with_reference_measurement(
                case,
                timing::MeasurementColumns {
                    case_elapsed,
                    rsqjs_measure,
                    quickjs_measure: REFERENCE_MEASURE_CACHED.to_owned(),
                },
                timing::ReferenceColumns::not_measured(REFERENCE_NOT_AVAILABLE),
                NOT_MEASURED.to_owned(),
                REFERENCE_SOURCE_BASELINE,
                false,
                &format!("{detail}; cached reference unavailable: {reference_detail}"),
            )
        }
        ReferenceMeasurement::Missing => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns {
                case_elapsed,
                rsqjs_measure,
                quickjs_measure: NOT_MEASURED.to_owned(),
            },
            timing::ReferenceColumns::not_measured(REFERENCE_BASELINE_MISSING),
            NOT_MEASURED.to_owned(),
            REFERENCE_SOURCE_MISSING,
            true,
            detail,
        ),
        ReferenceMeasurement::Disabled => failed_with_reference_measurement(
            case,
            timing::MeasurementColumns {
                case_elapsed,
                rsqjs_measure,
                quickjs_measure: NOT_MEASURED.to_owned(),
            },
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_CONFIGURED),
            NOT_MEASURED.to_owned(),
            REFERENCE_SOURCE_DISABLED,
            false,
            detail,
        ),
    }
}

fn failed_with_reference_measurement(
    case: &JetStreamCase,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    quality: String,
    reference_source: &str,
    reference_missing: bool,
    detail: &str,
) -> JetStreamOutcome {
    JetStreamOutcome {
        row: failed_row(
            case,
            measurements,
            quickjs,
            quality,
            reference_source,
            detail,
        ),
        counts: JetStreamCounts {
            failed: 1,
            reference_missing: count_if(reference_missing),
            ..JetStreamCounts::default()
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

#[cfg(test)]
fn benchmark_source(files: &[&str]) -> anyhow::Result<String> {
    workload_source(files).map(|workload| benchmark_source_from_workload(&workload))
}

fn workload_source(files: &[&str]) -> anyhow::Result<String> {
    let mut script = String::new();
    for file in files {
        let source = fs::read_to_string(file)
            .with_context(|| format!("failed to read JetStream source '{file}'"))?;
        script.push_str("// JetStream source: ");
        script.push_str(file);
        script.push('\n');
        script.push_str(&source);
        script.push('\n');
    }
    Ok(script)
}

fn benchmark_source_from_workload(workload: &str) -> String {
    format!("{SHELL_PRELUDE}\n{workload}{SYNC_HARNESS}")
}

fn harness_descriptor() -> String {
    format!("timer=rust-instant\nprelude:\n{SHELL_PRELUDE}\nharness:\n{SYNC_HARNESS}")
}

fn measured_with_reference(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    reference: ReferenceSample,
    case_elapsed: String,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, Some(reference.stats)) {
        let measurements = timing::MeasurementColumns {
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: reference.measure_text(),
        };
        let quickjs = timing::ReferenceColumns::measured(
            format_duration(reference.stats.median()),
            reference.stats.cv_percent_text(),
        );
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            quickjs,
            reference.source,
            &detail,
            ReferenceFlags {
                skipped: false,
                missing: false,
            },
        );
    }
    let budget = budget_check(
        ours.value.median().as_nanos(),
        reference.stats.median().as_nanos(),
    );
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: jetstream_status(budget.over_budget).to_owned(),
            source: case.source_label(),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: reference.measure_text(),
            reference_source: reference.source.to_owned(),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: format_duration(reference.stats.median()),
            latency_ratio: ratio_values(
                ours.value.median().as_nanos(),
                reference.stats.median().as_nanos(),
            ),
            latency_budget: budget.label.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: reference.stats.cv_percent_text(),
            quality: QUALITY_VALID.to_owned(),
            detail: jetstream_detail(&detail_text(budget.over_budget)),
        },
        counts: JetStreamCounts {
            measured: 1,
            over_latency_budget: count_if(budget.over_budget),
            ..JetStreamCounts::default()
        },
    }
}

fn measured_without_reference(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    case_elapsed: String,
    reference_time: &str,
    reference_source: &str,
    reference_detail: Option<&str>,
    reference_missing: bool,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, None) {
        let measurements =
            timing::MeasurementColumns::without_reference(case_elapsed, ours.elapsed);
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            timing::ReferenceColumns::not_measured(reference_time),
            reference_source,
            &reference_detail.map_or_else(
                || detail.clone(),
                |reference_detail| format!("{detail}; {reference_detail}"),
            ),
            ReferenceFlags {
                skipped: true,
                missing: reference_missing,
            },
        );
    }
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: "✅ measured".to_owned(),
            source: case.source_label(),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: NOT_MEASURED.to_owned(),
            reference_source: reference_source.to_owned(),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: reference_time.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: "🟡 no reference".to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: jetstream_detail(&reference_detail.map_or_else(
                || DETAIL_COMPLETED.to_owned(),
                |detail| format!("{DETAIL_COMPLETED}; {detail}"),
            )),
        },
        counts: JetStreamCounts {
            measured: 1,
            skipped: 1,
            reference_missing: count_if(reference_missing),
            ..JetStreamCounts::default()
        },
    }
}

fn reference_unavailable(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    note: &timing::Timed<String>,
    case_elapsed: String,
) -> JetStreamOutcome {
    if let Some(detail) = quality_failure_detail(ours.value, None) {
        let measurements =
            timing::MeasurementColumns::measured(case_elapsed, ours.elapsed, note.elapsed);
        return invalid_measurement_outcome(
            case,
            ours,
            measurements,
            timing::ReferenceColumns::not_measured(REFERENCE_NOT_AVAILABLE),
            REFERENCE_SOURCE_LIVE,
            &format!("{detail}; reference error: {}", note.value),
            ReferenceFlags {
                skipped: true,
                missing: false,
            },
        );
    }
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: "✅ measured".to_owned(),
            source: case.source_label(),
            case_elapsed,
            rsqjs_measure: timing::format_duration(ours.elapsed),
            quickjs_measure: timing::format_duration(note.elapsed),
            reference_source: REFERENCE_SOURCE_LIVE.to_owned(),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: REFERENCE_NOT_AVAILABLE.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: LATENCY_NOT_AVAILABLE.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: jetstream_detail(&format!(
                "{DETAIL_COMPLETED}; reference error: {}",
                note.value
            )),
        },
        counts: JetStreamCounts {
            measured: 1,
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn invalid_measurement_outcome(
    case: &JetStreamCase,
    ours: timing::Timed<MeasureStats>,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    reference_source: &str,
    detail: &str,
    reference: ReferenceFlags,
) -> JetStreamOutcome {
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_INVALID_BENCHMARK.to_owned(),
            source: case.source_label(),
            case_elapsed: measurements.case_elapsed,
            rsqjs_measure: measurements.rsqjs_measure,
            quickjs_measure: measurements.quickjs_measure,
            reference_source: reference_source.to_owned(),
            rsqjs_time: format_duration(ours.value.median()),
            quickjs_time: quickjs.eval,
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: LATENCY_INVALID.to_owned(),
            rsqjs_cv: ours.value.cv_percent_text(),
            quickjs_cv: quickjs.cv,
            quality: QUALITY_INVALID.to_owned(),
            detail: jetstream_detail(detail),
        },
        counts: JetStreamCounts {
            measured: 1,
            failed: 1,
            invalid: 1,
            skipped: count_if(reference.skipped),
            reference_missing: count_if(reference.missing),
            ..JetStreamCounts::default()
        },
    }
}

fn failed_outcome(case: &JetStreamCase, case_elapsed: String, detail: &str) -> JetStreamOutcome {
    JetStreamOutcome {
        row: failed_row(
            case,
            timing::MeasurementColumns::not_measured(case_elapsed),
            timing::ReferenceColumns::not_measured(NOT_MEASURED),
            NOT_MEASURED.to_owned(),
            REFERENCE_SOURCE_MISSING,
            detail,
        ),
        counts: JetStreamCounts {
            failed: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn failed_row(
    case: &JetStreamCase,
    measurements: timing::MeasurementColumns,
    quickjs: timing::ReferenceColumns,
    quality: String,
    reference_source: &str,
    detail: &str,
) -> JetStreamRow {
    JetStreamRow {
        benchmark: case.id.to_owned(),
        status: STATUS_FAILED.to_owned(),
        source: case.source_label(),
        case_elapsed: measurements.case_elapsed,
        rsqjs_measure: measurements.rsqjs_measure,
        quickjs_measure: measurements.quickjs_measure,
        reference_source: reference_source.to_owned(),
        rsqjs_time: NOT_MEASURED.to_owned(),
        quickjs_time: quickjs.eval,
        latency_ratio: NOT_MEASURED.to_owned(),
        latency_budget: NOT_MEASURED.to_owned(),
        rsqjs_cv: NOT_MEASURED.to_owned(),
        quickjs_cv: quickjs.cv,
        quality,
        detail: jetstream_detail(detail),
    }
}

fn skipped_outcome(case: &JetStreamCase, reason: &str) -> JetStreamOutcome {
    JetStreamOutcome {
        row: JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_SKIPPED.to_owned(),
            source: case.source_label(),
            case_elapsed: NOT_MEASURED.to_owned(),
            rsqjs_measure: NOT_MEASURED.to_owned(),
            quickjs_measure: NOT_MEASURED.to_owned(),
            reference_source: NOT_MEASURED.to_owned(),
            rsqjs_time: NOT_MEASURED.to_owned(),
            quickjs_time: NOT_MEASURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: NOT_MEASURED.to_owned(),
            rsqjs_cv: NOT_MEASURED.to_owned(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: NOT_MEASURED.to_owned(),
            detail: jetstream_detail(reason),
        },
        counts: JetStreamCounts {
            skipped: 1,
            ..JetStreamCounts::default()
        },
    }
}

fn jetstream_detail(detail: &str) -> String {
    report_text::table_detail(detail)
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

const fn budget_check(ours: u128, reference: u128) -> BudgetCheck {
    if reference == 0 {
        return BudgetCheck {
            label: LATENCY_NOT_AVAILABLE,
            over_budget: false,
        };
    }
    let over_budget =
        ours.saturating_mul(BUDGET_DENOMINATOR) > reference.saturating_mul(BUDGET_NUMERATOR);
    BudgetCheck {
        label: if over_budget {
            LATENCY_OVER
        } else {
            LATENCY_WITHIN
        },
        over_budget,
    }
}

const fn jetstream_status(over_latency_budget: bool) -> &'static str {
    if over_latency_budget {
        return STATUS_TRACKED_EXCEPTION;
    }
    STATUS_WITHIN_BUDGET
}

const fn count_if(condition: bool) -> usize {
    if condition { 1 } else { 0 }
}

#[cfg(test)]
#[path = "jetstream_tests.rs"]
mod tests;
