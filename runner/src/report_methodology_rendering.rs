use std::time::Duration;

use crate::{
    report_benchmark_methodology::{
        BenchmarkChecksum, BenchmarkLifecycle, BenchmarkMethodology, LifecyclePhase,
        LifecyclePhaseKind,
    },
    timing,
};

pub fn methodology_mode(methodology: Option<&BenchmarkMethodology>) -> String {
    methodology
        .and_then(|value| value.mode)
        .map_or_else(|| "-".to_owned(), |value| value.label().to_owned())
}

pub fn methodology_lifecycle(methodology: Option<&BenchmarkMethodology>) -> String {
    let Some(lifecycle) = methodology.and_then(|value| value.lifecycle.as_ref()) else {
        return "-".to_owned();
    };
    render_lifecycle(lifecycle)
}

pub fn methodology_checksum(methodology: Option<&BenchmarkMethodology>) -> String {
    methodology
        .and_then(|value| value.checksum.as_ref())
        .map_or_else(|| "-".to_owned(), BenchmarkChecksum::label)
}

pub fn methodology_reference(methodology: Option<&BenchmarkMethodology>) -> String {
    methodology
        .and_then(|value| value.reference_source)
        .map_or_else(|| "-".to_owned(), |value| value.label().to_owned())
}

fn render_lifecycle(lifecycle: &BenchmarkLifecycle) -> String {
    format!(
        "load={};compile={};setup={};warmup={};run={};verify={};teardown={}",
        lifecycle_phase(&lifecycle.load),
        lifecycle_phase(&lifecycle.compile),
        lifecycle_phase(&lifecycle.setup),
        lifecycle_phase(&lifecycle.warmup),
        lifecycle_phase(&lifecycle.run),
        lifecycle_phase(&lifecycle.verify),
        lifecycle_phase(&lifecycle.teardown),
    )
}

fn lifecycle_phase(phase: &LifecyclePhase) -> String {
    match phase.kind {
        LifecyclePhaseKind::Duration => phase.duration_ns.map_or_else(
            || "-".to_owned(),
            |duration_ns| timing::format_duration(Duration::from_nanos(duration_ns)),
        ),
        LifecyclePhaseKind::Measured => "measured".to_owned(),
        LifecyclePhaseKind::PerOperation => "per_operation".to_owned(),
        LifecyclePhaseKind::NotMeasured => "-".to_owned(),
    }
}
