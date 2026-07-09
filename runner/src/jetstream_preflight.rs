use std::collections::BTreeSet;

use super::{JetStreamCase, JetStreamMode, harness_descriptor, workload_source};
use crate::{
    bench_engines::REFERENCE_ENGINE_ID,
    bench_measure::MeasureConfig,
    jetstream_baseline::{BaselineKey, JetStreamQuickjsBaseline},
};

pub fn missing_reference_ids(
    cases: &[&JetStreamCase],
    config: MeasureConfig,
    host_profile: &str,
    baseline: &JetStreamQuickjsBaseline,
) -> BTreeSet<&'static str> {
    let mut missing = BTreeSet::new();
    if !baseline.is_read() {
        return missing;
    }
    let harness = harness_descriptor();
    for case in cases {
        if !matches!(case.mode, JetStreamMode::Timed) {
            continue;
        }
        let Ok(workload) = workload_source(case.files) else {
            continue;
        };
        let key = BaselineKey::new(
            case.id,
            &workload,
            &harness,
            config,
            REFERENCE_ENGINE_ID,
            host_profile,
        );
        if !baseline.contains(&key) {
            missing.insert(case.id);
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::missing_reference_ids;
    use crate::{
        bench_measure::MeasureConfig, jetstream::JetStreamCase,
        jetstream_baseline::JetStreamQuickjsBaseline,
    };
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn preflight_keeps_missing_timed_reference_visible_before_suite_budget() -> TestResult {
        const TIMED: JetStreamCase = JetStreamCase::timed("timed", &[]);
        const SKIPPED: JetStreamCase = JetStreamCase::skipped("skipped", "fixture");
        let baseline = JetStreamQuickjsBaseline::empty_read_for_test();
        let config = MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3);
        let missing = missing_reference_ids(&[&TIMED, &SKIPPED], config, "host", &baseline);
        if missing.len() == 1 && missing.contains("timed") {
            return Ok(());
        }
        Err(format!("unexpected missing reference ids: {missing:?}").into())
    }
}
