use tabled::{Table, Tabled};

use super::{
    cv_text, duration_text, fenced_table, optional_duration_text, push_tsv_row, ratio_text,
};
use crate::{
    jetstream::BUDGET_LABEL,
    report_schema::{JetStreamDetailRecord, JetStreamRecord, JetStreamSuite},
};

#[derive(Debug, Tabled)]
struct JetStreamTableRow {
    benchmark: String,
    status: String,
    source: String,
    case_elapsed: String,
    velum_measure: String,
    quickjs_measure: String,
    velum_time: String,
    quickjs_time: String,
    latency_ratio: String,
    latency_budget: String,
    velum_cv: String,
    quickjs_cv: String,
    quality: String,
    reference_source: String,
    detail: String,
}

pub(super) fn render_section(report: &JetStreamSuite) -> Vec<String> {
    vec![
        "## JetStream Shell Benchmarks".to_owned(),
        String::new(),
        format!(
            "- Selected official rows: {}\n- Measured: {}\n- Failed candidates: {}\n- Invalid measurements: {}\n- Skipped candidates: {}\n- Unavailable reference measurements: {}\n- Missing or stale QuickJS baseline: {}\n- Over latency budget ({}): {}\n- Elapsed: {}",
            report.counts.total,
            report.counts.measured,
            report.counts.failed,
            report.counts.invalid,
            report.counts.skipped,
            report.counts.unavailable_reference,
            report.counts.missing_reference,
            BUDGET_LABEL,
            report.counts.over_latency_budget,
            duration_text(report.duration_ns),
        ),
        String::new(),
        fenced_table(&Table::new(table_rows(report))),
        String::new(),
    ]
}

pub(super) fn append_timing_rows(body: &mut String, suite: &JetStreamSuite) {
    for row in &suite.rows {
        let detail = suite.details.iter().find(|detail| detail.id == row.id);
        let case_elapsed = optional_duration_text(detail.and_then(|value| value.case_duration_ns));
        let velum_measure =
            optional_duration_text(detail.and_then(|value| value.engine_wall_duration_ns));
        let quickjs_measure =
            optional_duration_text(detail.and_then(|value| value.reference_wall_duration_ns));
        let source = row.source.as_deref().unwrap_or("-");
        let reference_source = row
            .reference_source
            .map_or_else(String::new, |source| source.label().to_owned());
        push_tsv_row(
            body,
            &[
                "jetstream",
                "JetStream Shell Benchmarks",
                &row.id,
                row.status.label(),
                source,
                "",
                &case_elapsed,
                &velum_measure,
                &quickjs_measure,
                &row.detail,
                "",
                "",
                "",
                &reference_source,
            ],
        );
    }
}

fn table_rows(suite: &JetStreamSuite) -> Vec<JetStreamTableRow> {
    suite
        .rows
        .iter()
        .map(|row| {
            let detail = suite.details.iter().find(|detail| detail.id == row.id);
            table_row(row, detail)
        })
        .collect()
}

fn table_row(row: &JetStreamRecord, detail: Option<&JetStreamDetailRecord>) -> JetStreamTableRow {
    JetStreamTableRow {
        benchmark: row.id.clone(),
        status: row.status.label().to_owned(),
        source: row.source.clone().unwrap_or_else(|| "-".to_owned()),
        case_elapsed: optional_duration_text(detail.and_then(|value| value.case_duration_ns)),
        velum_measure: optional_duration_text(
            detail.and_then(|value| value.engine_wall_duration_ns),
        ),
        quickjs_measure: optional_duration_text(
            detail.and_then(|value| value.reference_wall_duration_ns),
        ),
        velum_time: optional_duration_text(row.engine_median_duration_ns),
        quickjs_time: optional_duration_text(row.reference_median_duration_ns),
        latency_ratio: ratio_text(row.latency_ratio_centi_units),
        latency_budget: detail.map_or_else(
            || "-".to_owned(),
            |value| value.latency_budget.label().to_owned(),
        ),
        velum_cv: row
            .engine_cv_permille
            .map_or_else(|| "-".to_owned(), cv_text),
        quickjs_cv: row
            .reference_cv_permille
            .map_or_else(|| "-".to_owned(), cv_text),
        quality: detail.map_or_else(|| "-".to_owned(), |value| value.quality.label().to_owned()),
        reference_source: row
            .reference_source
            .map_or_else(|| "-".to_owned(), |source| source.label().to_owned()),
        detail: row.detail.clone(),
    }
}
