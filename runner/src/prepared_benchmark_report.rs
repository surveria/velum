use crate::{
    benchmark_protocol::BenchmarkLifecycle,
    cases::BenchmarkCase,
    prepared_benchmarks::{PreparedCaseRun, PreparedReference},
    timing,
};

use super::{
    BUDGET_INVALID, BenchmarkOutcome, NOT_MEASURED, QUALITY_INVALID, REFERENCE_NOT_CONFIGURED,
    ReferenceMeasurement, STATUS_FAILED, benchmark_detail, failed_with_reference,
    measured_with_reference_result,
};

pub(super) fn outcome(case: &BenchmarkCase, run: &PreparedCaseRun) -> BenchmarkOutcome {
    let reference = reference_measurement(&run.reference);
    let case_elapsed = timing::format_duration(run.case_elapsed);
    let mut outcome = match &run.ours {
        Ok(ours) => measured_with_reference_result(
            case,
            timing::Timed {
                value: ours.stats,
                elapsed: ours.elapsed,
            },
            reference,
            case_elapsed,
        ),
        Err(failure) => failed_with_reference(
            case,
            &failure.error,
            timing::format_duration(failure.elapsed),
            reference,
            case_elapsed,
        ),
    };
    outcome.row.mode = case.mode.to_string();
    reference_source(&run.reference).clone_into(&mut outcome.row.reference_source);
    if let Ok(ours) = &run.ours {
        outcome.row.lifecycle = render_lifecycle(ours.lifecycle);
        outcome.row.checksum = ours.checksum.to_string();
    }
    if let Some(error) = &run.parity_error {
        STATUS_FAILED.clone_into(&mut outcome.row.status);
        NOT_MEASURED.clone_into(&mut outcome.row.latency_ratio);
        BUDGET_INVALID.clone_into(&mut outcome.row.latency_budget);
        QUALITY_INVALID.clone_into(&mut outcome.row.quality);
        outcome.row.detail = benchmark_detail(error);
        outcome.counts.failed = 1;
        outcome.counts.over_latency_budget = 0;
    }
    outcome
}

fn reference_measurement(reference: &PreparedReference) -> ReferenceMeasurement {
    match reference {
        PreparedReference::NotConfigured => ReferenceMeasurement::NotConfigured,
        PreparedReference::Measured { measurement, .. } => {
            ReferenceMeasurement::Measured(timing::Timed {
                value: measurement.stats,
                elapsed: measurement.elapsed,
            })
        }
        PreparedReference::Failed(failure) => ReferenceMeasurement::Failed(timing::Timed {
            value: failure.error.clone(),
            elapsed: failure.elapsed,
        }),
    }
}

const fn reference_source(reference: &PreparedReference) -> &'static str {
    match reference {
        PreparedReference::NotConfigured => REFERENCE_NOT_CONFIGURED,
        PreparedReference::Measured { source, .. } => source.as_str(),
        PreparedReference::Failed(_) => "quickjs_live_failed",
    }
}

fn render_lifecycle(lifecycle: BenchmarkLifecycle) -> String {
    format!(
        "load={};compile={};setup={};warmup={};run={};verify={};teardown={}",
        timing::format_duration(lifecycle.load),
        optional_duration(lifecycle.compile),
        optional_duration(lifecycle.setup),
        timing::format_duration(lifecycle.warmup),
        timing::format_duration(lifecycle.timed_run),
        optional_duration(lifecycle.verify),
        optional_duration(lifecycle.teardown),
    )
}

fn optional_duration(duration: Option<std::time::Duration>) -> String {
    duration.map_or_else(|| NOT_MEASURED.to_owned(), timing::format_duration)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        bench_measure::{MeasureConfig, MeasureSnapshot, MeasureStats},
        benchmark_protocol::{BenchmarkChecksum, BenchmarkLifecycle},
        cases::BenchmarkCase,
        prepared_benchmarks::{PreparedCaseRun, PreparedMeasurement, PreparedReference},
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn parity_error_does_not_double_count_an_invalid_measurement() -> TestResult {
        let config = MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3)
            .with_quality(Duration::from_millis(2), u32::MAX);
        let stats = MeasureStats::from_snapshot(
            MeasureSnapshot {
                median: Duration::from_millis(1),
                cv_permille: 0,
                iters_per_sample: 1,
                samples: 3,
                median_sample: Duration::from_millis(1),
                warmup_elapsed: Duration::ZERO,
                timed_run_elapsed: Duration::from_millis(3),
                iteration_cap_reached: false,
            },
            config,
        );
        let run = PreparedCaseRun {
            ours: Ok(PreparedMeasurement {
                stats,
                checksum: BenchmarkChecksum::number(42.0),
                lifecycle: BenchmarkLifecycle::default(),
                elapsed: Duration::from_millis(3),
            }),
            reference: PreparedReference::NotConfigured,
            parity_error: Some("checksum mismatch".to_owned()),
            case_elapsed: Duration::from_millis(3),
        };
        let case = BenchmarkCase::prepared_sentinel("sentinel", "sentinel.js");
        let outcome = super::outcome(&case, &run);
        if outcome.counts.failed == 1 && outcome.counts.invalid == 1 {
            return Ok(());
        }
        Err(format!(
            "expected one failed invalid row, got failed={} invalid={}",
            outcome.counts.failed, outcome.counts.invalid
        )
        .into())
    }
}
