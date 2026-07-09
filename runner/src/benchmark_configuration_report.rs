use std::time::Duration;

use crate::benchmark_protocol::BenchmarkMethodology;

use super::{
    BenchmarkCounts, BenchmarkOutcome, BenchmarkReport, BenchmarkRow, NOT_MEASURED,
    QUALITY_INVALID, STATUS_FAILED, benchmark_detail, push_outcome,
};

pub(super) fn configuration_failure_report(elapsed: Duration, error: &str) -> BenchmarkReport {
    let mut report = BenchmarkReport {
        rows: Vec::new(),
        measured: 0,
        in_process_measured: 0,
        failed: 0,
        invalid: 0,
        skipped: 0,
        over_latency_budget: 0,
        over_memory_budget: 0,
        elapsed,
    };
    push_outcome(&mut report, configuration_failure_outcome(error));
    report
}

pub(super) fn configuration_failure_outcome(error: &str) -> BenchmarkOutcome {
    BenchmarkOutcome {
        row: BenchmarkRow {
            benchmark: "benchmark_configuration".to_owned(),
            status: STATUS_FAILED.to_owned(),
            source: "runner environment".to_owned(),
            iterations: 0,
            case_elapsed: NOT_MEASURED.to_owned(),
            rsqjs_measure: NOT_MEASURED.to_owned(),
            quickjs_measure: NOT_MEASURED.to_owned(),
            rsqjs_eval: NOT_MEASURED.to_owned(),
            quickjs_eval: NOT_MEASURED.to_owned(),
            latency_ratio: NOT_MEASURED.to_owned(),
            latency_budget: NOT_MEASURED.to_owned(),
            memory_ratio: NOT_MEASURED.to_owned(),
            rsqjs_cv: NOT_MEASURED.to_owned(),
            quickjs_cv: NOT_MEASURED.to_owned(),
            quality: QUALITY_INVALID.to_owned(),
            detail: benchmark_detail(error),
            mode: NOT_MEASURED.to_owned(),
            lifecycle: NOT_MEASURED.to_owned(),
            checksum: NOT_MEASURED.to_owned(),
            reference_source: NOT_MEASURED.to_owned(),
            methodology: BenchmarkMethodology::not_measured(),
            count_contribution: super::BenchmarkCountContribution::default(),
        },
        counts: BenchmarkCounts {
            failed: 1,
            ..BenchmarkCounts::default()
        },
    }
}
