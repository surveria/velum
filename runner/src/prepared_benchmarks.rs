//! Execution of prepared benchmark cases.

use std::{hint::black_box, time::Duration};

use anyhow::bail;

use crate::{
    bench_engines::{BenchEngine, REFERENCE_ENGINE_ID},
    bench_measure::{self, MeasureConfig, MeasureStats},
    benchmark_protocol::{BenchmarkChecksum, BenchmarkLifecycle},
    cases::BenchmarkCase,
    quickjs_baseline::{BaselineKey, BaselineSample, QuickjsBaseline, harness_descriptor},
    timing,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ReferenceSource {
    Baseline,
    Live,
}

impl ReferenceSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Baseline => "quickjs_baseline",
            Self::Live => "quickjs_live",
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreparedMeasurement {
    pub stats: MeasureStats,
    pub checksum: BenchmarkChecksum,
    pub lifecycle: BenchmarkLifecycle,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct PreparedFailure {
    pub error: String,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub enum PreparedReference {
    NotConfigured,
    Measured {
        measurement: Box<PreparedMeasurement>,
        source: ReferenceSource,
    },
    Failed(PreparedFailure),
}

#[derive(Debug, Clone)]
pub struct PreparedCaseRun {
    pub ours: Result<PreparedMeasurement, PreparedFailure>,
    pub reference: PreparedReference,
    pub parity_error: Option<String>,
    pub case_elapsed: Duration,
}

pub fn run(
    case: &BenchmarkCase,
    source: &str,
    load_elapsed: Duration,
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
    baseline: &mut QuickjsBaseline,
    host_profile: &str,
) -> anyhow::Result<PreparedCaseRun> {
    let case_timer = timing::RunTimer::start();
    let ours = measure_prepared(&crate::bench_engines::VelumEngine, case, source, config).map(
        |mut measurement| {
            measurement.lifecycle.load = load_elapsed;
            measurement
        },
    );
    let key = baseline_key(case, source, config, host_profile);
    let reference = measure_reference(case, source, config, reference, baseline, &key)?;
    let parity_error = parity_error(&ours, &reference);
    Ok(PreparedCaseRun {
        ours,
        reference,
        parity_error,
        case_elapsed: case_timer.elapsed().saturating_add(load_elapsed),
    })
}

fn measure_reference(
    case: &BenchmarkCase,
    source: &str,
    config: MeasureConfig,
    reference: Option<&dyn BenchEngine>,
    baseline: &mut QuickjsBaseline,
    key: &BaselineKey,
) -> anyhow::Result<PreparedReference> {
    if let Some(sample) = baseline.lookup(key)? {
        return Ok(PreparedReference::Measured {
            measurement: Box::new(cached_measurement(sample, config)),
            source: ReferenceSource::Baseline,
        });
    }
    let Some(reference) = reference else {
        return Ok(PreparedReference::NotConfigured);
    };
    match measure_prepared(reference, case, source, config) {
        Ok(measurement) => {
            baseline.record(
                key.clone(),
                BaselineSample::from_measurement(measurement.checksum.clone(), measurement.stats),
            )?;
            Ok(PreparedReference::Measured {
                measurement: Box::new(measurement),
                source: ReferenceSource::Live,
            })
        }
        Err(failure) => Ok(PreparedReference::Failed(failure)),
    }
}

fn cached_measurement(sample: BaselineSample, config: MeasureConfig) -> PreparedMeasurement {
    let stats = sample.stats(config);
    PreparedMeasurement {
        stats,
        checksum: sample.checksum,
        lifecycle: BenchmarkLifecycle {
            warmup: stats.warmup_elapsed(),
            timed_run: stats.timed_run_elapsed(),
            ..BenchmarkLifecycle::default()
        },
        elapsed: Duration::ZERO,
    }
}

fn measure_prepared(
    engine: &dyn BenchEngine,
    case: &BenchmarkCase,
    source: &str,
    config: MeasureConfig,
) -> Result<PreparedMeasurement, PreparedFailure> {
    let total_timer = timing::RunTimer::start();
    let prepared = match engine.prepare(source, case.input) {
        Ok(prepared) => prepared,
        Err(error) => {
            return Err(PreparedFailure {
                error: error.to_string(),
                elapsed: total_timer.elapsed(),
            });
        }
    };
    let compile_elapsed = prepared.compile_elapsed;
    let setup_elapsed = prepared.setup_elapsed;
    let mut session = prepared.session;
    let measured = measure_session(engine.label(), session.as_mut(), config);
    let teardown_timer = timing::RunTimer::start();
    drop(session);
    let teardown_elapsed = teardown_timer.elapsed();
    match measured {
        Ok((stats, checksum, verify_elapsed)) => Ok(PreparedMeasurement {
            stats,
            checksum,
            lifecycle: BenchmarkLifecycle {
                load: Duration::ZERO,
                compile: compile_elapsed,
                setup: Some(setup_elapsed),
                warmup: stats.warmup_elapsed(),
                timed_run: stats.timed_run_elapsed(),
                verify: Some(verify_elapsed),
                teardown: Some(teardown_elapsed),
            },
            elapsed: total_timer.elapsed(),
        }),
        Err(error) => Err(PreparedFailure {
            error: error.to_string(),
            elapsed: total_timer.elapsed(),
        }),
    }
}

fn measure_session(
    engine_label: &str,
    session: &mut dyn crate::bench_engines::PreparedBenchSession,
    config: MeasureConfig,
) -> anyhow::Result<(MeasureStats, BenchmarkChecksum, Duration)> {
    let verify_timer = timing::RunTimer::start();
    let expected = session.run()?;
    let preflight_elapsed = verify_timer.elapsed();
    let stats = bench_measure::measure(config, || {
        let actual = session.run()?;
        ensure_checksum(engine_label, &actual, &expected)?;
        black_box(actual);
        Ok(())
    })?;
    let post_verify_timer = timing::RunTimer::start();
    let verified = session.verify()?;
    ensure_checksum(engine_label, &verified, &expected)?;
    let verify_elapsed = preflight_elapsed.saturating_add(post_verify_timer.elapsed());
    Ok((stats, expected, verify_elapsed))
}

fn ensure_checksum(
    engine_label: &str,
    actual: &BenchmarkChecksum,
    expected: &BenchmarkChecksum,
) -> anyhow::Result<()> {
    if actual == expected {
        return Ok(());
    }
    bail!(
        "{engine_label} benchmark checksum changed between runs: expected {expected}, got {actual}"
    )
}

fn parity_error(
    ours: &Result<PreparedMeasurement, PreparedFailure>,
    reference: &PreparedReference,
) -> Option<String> {
    let Ok(ours) = ours else {
        return None;
    };
    let PreparedReference::Measured {
        measurement: reference,
        ..
    } = reference
    else {
        return None;
    };
    if ours.checksum == reference.checksum {
        return None;
    }
    Some(format!(
        "checksum mismatch: Velum {}, QuickJS {}",
        ours.checksum, reference.checksum
    ))
}

fn baseline_key(
    case: &BenchmarkCase,
    source: &str,
    config: MeasureConfig,
    host_profile: &str,
) -> BaselineKey {
    let harness = harness_descriptor(case.mode.as_str(), &case.input.descriptor());
    BaselineKey::new(
        case.id,
        source,
        &harness,
        config,
        REFERENCE_ENGINE_ID,
        host_profile,
    )
}

#[cfg(test)]
mod tests {
    use super::{PreparedMeasurement, PreparedReference, parity_error};
    use crate::{
        bench_measure::{MeasureConfig, measure},
        benchmark_protocol::{BenchmarkChecksum, BenchmarkLifecycle},
    };
    use std::time::Duration;

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn detects_cross_engine_checksum_mismatch() -> TestResult {
        let ours = Ok(measurement(42.0)?);
        let reference = PreparedReference::Measured {
            measurement: Box::new(measurement(43.0)?),
            source: super::ReferenceSource::Live,
        };
        if parity_error(&ours, &reference).is_some() {
            return Ok(());
        }
        Err("prepared benchmark accepted a cross-engine checksum mismatch".into())
    }

    fn measurement(checksum: f64) -> Result<PreparedMeasurement, anyhow::Error> {
        let config = MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3)
            .with_quality(Duration::ZERO, u32::MAX);
        let stats = measure(config, || Ok(()))?;
        Ok(PreparedMeasurement {
            stats,
            checksum: BenchmarkChecksum::number(checksum),
            lifecycle: BenchmarkLifecycle::default(),
            elapsed: Duration::ZERO,
        })
    }
}
