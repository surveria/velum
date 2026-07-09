use super::{
    DETAIL_COMPLETED, JetStreamReport, JetStreamRow, LATENCY_WITHIN, QUALITY_VALID,
    REFERENCE_SOURCE_BASELINE, STATUS_WITHIN_BUDGET, jetstream_cases,
};

pub fn worst_case_report_fixture() -> JetStreamReport {
    let rows = jetstream_cases::cases()
        .iter()
        .map(|case| JetStreamRow {
            benchmark: case.id.to_owned(),
            status: STATUS_WITHIN_BUDGET.to_owned(),
            source: case.source_label(),
            case_elapsed: "2.00 ms".to_owned(),
            rsqjs_measure: "1.50 ms".to_owned(),
            quickjs_measure: "1.50 ms".to_owned(),
            reference_source: REFERENCE_SOURCE_BASELINE.to_owned(),
            rsqjs_time: "1.00 ms".to_owned(),
            quickjs_time: "1.25 ms".to_owned(),
            latency_ratio: "0.80x".to_owned(),
            latency_budget: LATENCY_WITHIN.to_owned(),
            rsqjs_cv: "1.0%".to_owned(),
            quickjs_cv: "1.0%".to_owned(),
            quality: QUALITY_VALID.to_owned(),
            detail: DETAIL_COMPLETED.to_owned(),
        })
        .collect::<Vec<_>>();
    JetStreamReport {
        measured: rows.len(),
        rows,
        failed: 0,
        invalid: 0,
        skipped: 0,
        over_latency_budget: 0,
        reference_missing: 0,
        elapsed: std::time::Duration::from_secs(1),
    }
}
