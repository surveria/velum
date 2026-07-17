//! Direct measurements of the public Rust embedding facade.

use std::{
    cell::Cell,
    hint::black_box,
    task::{Context as TaskContext, Waker},
    time::Duration,
};

use anyhow::{Context as _, bail};
use velum::{
    Engine, EngineConfig, HostObjectOptions, JsValueRef, OwnedValue, PropertyKeyRef, RetainedValue,
    RuntimeLimits, Vm, VmConfig,
};

use crate::{
    bench_measure::{self, MeasureConfig, MeasureStats},
    benchmark_case::EmbeddingBenchmark,
    benchmark_protocol::{BenchmarkLifecycle, BenchmarkReferenceSource, ReportedLifecycle},
    cases::BenchmarkCase,
    timing,
};

use super::{
    BenchmarkOutcome, REFERENCE_NOT_CONFIGURED, benchmark_detail, failed_outcome,
    measured_without_reference,
};

const SYNC_CALLS_PER_BATCH: usize = 10_000;
const PROPERTY_GETS_PER_BATCH: usize = 20_000;
const RUST_CALLBACKS_PER_BATCH: usize = 10_000;
const ASYNC_COMPLETIONS_PER_BATCH: usize = 512;
const HOST_PAYLOAD_READS_PER_BATCH: usize = 100_000;

const ANSWER: f64 = 42.0;
const ASYNC_VALUE: f64 = 1.0;
const ASYNC_EXPECTED_TOTAL: f64 = 512.0;
const ANSWER_PROPERTY: &str = "answer";
const PROMISE_THEN_PROPERTY: &str = "then";
const ASYNC_SINK_PROPERTY: &str = "total";
const ASYNC_SINK_GLOBAL: &str = "__velumEmbeddingBenchSink";
const ASYNC_RECORD_GLOBAL: &str = "__velumEmbeddingBenchRecord";
const SYNC_FUNCTION_SOURCE: &str = "(function embeddingBenchAdd(value) { return value + 1; })";
const ASYNC_SINK_SOURCE: &str = r"
    var __velumEmbeddingBenchSink = { total: 0 };
    var __velumEmbeddingBenchRecord = function (value) {
        __velumEmbeddingBenchSink.total += value;
    };
";

const DETAIL_EMBEDDING_COMPLETED: &str =
    "sequential direct embedding benchmark completed; no equivalent QuickJS Rust API";

struct EmbeddingRun {
    stats: MeasureStats,
    measure_elapsed: Duration,
    lifecycle: BenchmarkLifecycle,
    case_elapsed: Duration,
}

struct HostCounter {
    value: Cell<usize>,
}

trait BenchmarkResult<T> {
    fn benchmark_context(self, operation: &str) -> anyhow::Result<T>;
}

impl<T> BenchmarkResult<T> for velum::Result<T> {
    fn benchmark_context(self, operation: &str) -> anyhow::Result<T> {
        self.map_err(|error| anyhow::Error::msg(format!("{operation}: {error}")))
    }
}

enum EmbeddingState {
    SyncCall {
        vm: Vm,
        callable: RetainedValue,
    },
    PropertyGet {
        vm: Vm,
        object: RetainedValue,
    },
    RustCallback {
        vm: Vm,
        callable: RetainedValue,
    },
    AsyncCompletion {
        vm: Vm,
        callable: RetainedValue,
        sink: RetainedValue,
        record: RetainedValue,
    },
    HostObjectPayload {
        vm: Vm,
        object: RetainedValue,
    },
}

pub(super) fn run(
    case: &BenchmarkCase,
    benchmark: EmbeddingBenchmark,
    config: MeasureConfig,
) -> BenchmarkOutcome {
    let measured = match measure(benchmark, config) {
        Ok(measured) => measured,
        Err(error) => {
            return failed_outcome(case, "-".to_owned(), &format!("{error:#}"));
        }
    };
    let mut outcome = measured_without_reference(
        case,
        timing::Timed {
            value: measured.stats,
            elapsed: measured.measure_elapsed,
        },
        timing::format_duration(measured.case_elapsed),
    );
    outcome.row.lifecycle = render_lifecycle(measured.lifecycle);
    outcome.row.detail = benchmark_detail(&format!(
        "{DETAIL_EMBEDDING_COMPLETED}; operations_per_batch={}",
        operations_per_batch(benchmark),
    ));
    REFERENCE_NOT_CONFIGURED.clone_into(&mut outcome.row.reference_source);
    outcome.row.methodology.lifecycle = Some(ReportedLifecycle::embedding_api(measured.lifecycle));
    outcome.row.methodology.reference_source = Some(BenchmarkReferenceSource::NotConfigured);
    outcome
}

fn measure(benchmark: EmbeddingBenchmark, config: MeasureConfig) -> anyhow::Result<EmbeddingRun> {
    let case_timer = timing::RunTimer::start();
    let setup = timing::timed(|| EmbeddingState::setup(benchmark));
    let mut case_state = setup.value.context("embedding benchmark setup failed")?;
    let measured = timing::timed(|| bench_measure::measure(config, || case_state.run_batch()));
    let teardown = timing::timed(|| case_state.teardown());
    teardown
        .value
        .context("embedding benchmark teardown failed")?;
    let stats = measured
        .value
        .context("embedding benchmark measurement failed")?;
    Ok(EmbeddingRun {
        stats,
        measure_elapsed: measured.elapsed,
        lifecycle: BenchmarkLifecycle {
            load: Duration::ZERO,
            compile: None,
            setup: Some(setup.elapsed),
            warmup: stats.warmup_elapsed(),
            timed_run: stats.timed_run_elapsed(),
            verify: None,
            teardown: Some(teardown.elapsed),
        },
        case_elapsed: case_timer.elapsed(),
    })
}

impl EmbeddingState {
    fn setup(benchmark: EmbeddingBenchmark) -> anyhow::Result<Self> {
        let limits = RuntimeLimits {
            max_runtime_steps: usize::MAX,
            ..RuntimeLimits::default()
        };
        let engine = Engine::with_config(EngineConfig::with_default_vm_config(
            VmConfig::with_limits(limits),
        ));
        let mut vm = engine.create_vm();
        match benchmark {
            EmbeddingBenchmark::SyncCall => {
                let callable = vm
                    .eval_retained(SYNC_FUNCTION_SOURCE)
                    .benchmark_context("JavaScript function setup failed")?;
                Ok(Self::SyncCall { vm, callable })
            }
            EmbeddingBenchmark::PropertyGet => {
                let object = vm
                    .create_object()
                    .benchmark_context("object setup failed")?;
                vm.set_property_or_throw(
                    (&object).into(),
                    PropertyKeyRef::Name(ANSWER_PROPERTY),
                    JsValueRef::Number(ANSWER),
                )
                .benchmark_context("property setup failed")?;
                Ok(Self::PropertyGet { vm, object })
            }
            EmbeddingBenchmark::RustCallback => {
                let callable = vm
                    .create_host_function_typed("embeddingBenchAdd", |call| {
                        let left = call.number(0, "left")?;
                        let right = call.number(1, "right")?;
                        Ok(left + right)
                    })
                    .benchmark_context("Rust callback setup failed")?;
                Ok(Self::RustCallback { vm, callable })
            }
            EmbeddingBenchmark::AsyncCompletion => {
                let callable = vm
                    .create_async_host_function_typed("embeddingBenchAsync", |_call| {
                        Ok(async move { Ok(ASYNC_VALUE) })
                    })
                    .benchmark_context("async host function setup failed")?;
                drop(
                    vm.eval(ASYNC_SINK_SOURCE)
                        .benchmark_context("async sink setup failed")?,
                );
                let sink = required_global(&vm, ASYNC_SINK_GLOBAL)?;
                let record = required_global(&vm, ASYNC_RECORD_GLOBAL)?;
                Ok(Self::AsyncCompletion {
                    vm,
                    callable,
                    sink,
                    record,
                })
            }
            EmbeddingBenchmark::HostObjectPayload => {
                let object = vm
                    .create_host_object(
                        HostCounter {
                            value: Cell::new(0),
                        },
                        HostObjectOptions::new(std::mem::size_of::<HostCounter>()),
                    )
                    .benchmark_context("host object setup failed")?;
                Ok(Self::HostObjectPayload { vm, object })
            }
        }
    }

    fn run_batch(&mut self) -> anyhow::Result<()> {
        match self {
            Self::SyncCall { vm, callable } => sync_call_batch(vm, callable),
            Self::PropertyGet { vm, object } => property_get_batch(vm, object),
            Self::RustCallback { vm, callable } => rust_callback_batch(vm, callable),
            Self::AsyncCompletion {
                vm,
                callable,
                sink,
                record,
            } => async_completion_batch(vm, callable, sink, record),
            Self::HostObjectPayload { vm, object } => host_payload_batch(vm, object),
        }
    }

    fn teardown(self) -> anyhow::Result<()> {
        match self {
            Self::SyncCall { vm, callable } | Self::RustCallback { vm, callable } => {
                callable
                    .release()
                    .benchmark_context("callable release failed")?;
                finish_vm(vm)
            }
            Self::PropertyGet { vm, object } | Self::HostObjectPayload { vm, object } => {
                object
                    .release()
                    .benchmark_context("object release failed")?;
                finish_vm(vm)
            }
            Self::AsyncCompletion {
                mut vm,
                callable,
                sink,
                record,
            } => {
                black_box((
                    vm.cancel_host_futures()
                        .benchmark_context("host future cancellation failed")?,
                    vm.cancel_jobs()
                        .benchmark_context("Promise job cancellation failed")?,
                ));
                callable
                    .release()
                    .benchmark_context("async callable release failed")?;
                sink.release()
                    .benchmark_context("async sink release failed")?;
                record
                    .release()
                    .benchmark_context("async record callback release failed")?;
                finish_vm(vm)
            }
        }
    }
}

fn sync_call_batch(vm: &mut Vm, callable: &RetainedValue) -> anyhow::Result<()> {
    let args = [JsValueRef::Number(41.0)];
    for _ in 0..SYNC_CALLS_PER_BATCH {
        let value = vm
            .call_owned(callable, &args)
            .benchmark_context("JavaScript call failed")?;
        ensure_number(&value, ANSWER, "JavaScript call result")?;
        black_box(value);
    }
    Ok(())
}

fn property_get_batch(vm: &mut Vm, object: &RetainedValue) -> anyhow::Result<()> {
    for _ in 0..PROPERTY_GETS_PER_BATCH {
        let value = vm
            .get_property_owned(
                JsValueRef::Retained(object),
                PropertyKeyRef::Name(ANSWER_PROPERTY),
            )
            .benchmark_context("property get failed")?;
        ensure_number(&value, ANSWER, "property result")?;
        black_box(value);
    }
    Ok(())
}

fn rust_callback_batch(vm: &mut Vm, callable: &RetainedValue) -> anyhow::Result<()> {
    let args = [JsValueRef::Number(20.0), JsValueRef::Number(22.0)];
    for _ in 0..RUST_CALLBACKS_PER_BATCH {
        let value = vm
            .call_owned(callable, &args)
            .benchmark_context("Rust callback dispatch failed")?;
        ensure_number(&value, ANSWER, "Rust callback result")?;
        black_box(value);
    }
    Ok(())
}

fn async_completion_batch(
    vm: &mut Vm,
    callable: &RetainedValue,
    sink: &RetainedValue,
    record: &RetainedValue,
) -> anyhow::Result<()> {
    vm.set_property_or_throw(
        JsValueRef::Retained(sink),
        PropertyKeyRef::Name(ASYNC_SINK_PROPERTY),
        JsValueRef::Number(0.0),
    )
    .benchmark_context("async sink reset failed")?;
    let mut promises = Vec::new();
    promises
        .try_reserve_exact(ASYNC_COMPLETIONS_PER_BATCH)
        .context("async Promise batch allocation failed")?;
    for _ in 0..ASYNC_COMPLETIONS_PER_BATCH {
        let promise = vm
            .call_retained(callable, &[])
            .benchmark_context("async host call failed")?;
        let chained = vm
            .call_method(
                JsValueRef::Retained(&promise),
                PropertyKeyRef::Name(PROMISE_THEN_PROPERTY),
                &[JsValueRef::Retained(record)],
            )
            .benchmark_context("Promise reaction registration failed")?;
        black_box(chained);
        promises.push(promise);
    }

    let mut task_context = TaskContext::from_waker(Waker::noop());
    let poll = vm
        .poll_host_futures(&mut task_context)
        .benchmark_context("async host future poll failed")?;
    ensure_count(
        poll.completed(),
        ASYNC_COMPLETIONS_PER_BATCH,
        "completed async host futures",
    )?;
    ensure_count(poll.pending(), 0, "pending async host futures")?;
    ensure_count(
        vm.run_jobs()
            .benchmark_context("Promise reaction drain failed")?,
        ASYNC_COMPLETIONS_PER_BATCH,
        "Promise reaction jobs",
    )?;
    let total = vm
        .get_property_owned(
            JsValueRef::Retained(sink),
            PropertyKeyRef::Name(ASYNC_SINK_PROPERTY),
        )
        .benchmark_context("async checksum read failed")?;
    ensure_number(&total, ASYNC_EXPECTED_TOTAL, "async completion checksum")?;
    black_box(total);
    for promise in promises {
        promise
            .release()
            .benchmark_context("Promise handle release failed")?;
    }
    black_box(
        vm.collect_garbage()
            .benchmark_context("async batch collection failed")?,
    );
    Ok(())
}

fn host_payload_batch(vm: &Vm, object: &RetainedValue) -> anyhow::Result<()> {
    let before = vm
        .host_payload::<HostCounter>(object)
        .benchmark_context("host payload lookup failed")?
        .value
        .get();
    let expected = before
        .checked_add(HOST_PAYLOAD_READS_PER_BATCH)
        .context("host payload benchmark counter overflowed")?;
    for _ in 0..HOST_PAYLOAD_READS_PER_BATCH {
        let payload = vm
            .host_payload::<HostCounter>(object)
            .benchmark_context("host payload lookup failed")?;
        let next = payload
            .value
            .get()
            .checked_add(1)
            .context("host payload benchmark counter overflowed")?;
        payload.value.set(next);
        black_box(next);
    }
    ensure_count(
        vm.host_payload::<HostCounter>(object)
            .benchmark_context("host payload verification failed")?
            .value
            .get(),
        expected,
        "host payload counter",
    )
}

fn required_global(vm: &Vm, name: &str) -> anyhow::Result<RetainedValue> {
    let Some(value) = vm
        .get_global_retained(name)
        .benchmark_context("benchmark global lookup failed")?
    else {
        bail!("embedding benchmark global '{name}' is missing")
    };
    Ok(value)
}

fn finish_vm(mut vm: Vm) -> anyhow::Result<()> {
    black_box(
        vm.collect_garbage()
            .benchmark_context("benchmark teardown collection failed")?,
    );
    black_box(
        vm.finish()
            .benchmark_context("benchmark VM teardown failed")?,
    );
    Ok(())
}

fn ensure_number(value: &OwnedValue, expected: f64, label: &str) -> anyhow::Result<()> {
    if value == &OwnedValue::Number(expected) {
        return Ok(());
    }
    bail!("expected {label} {expected}, got {value:?}")
}

fn ensure_count(actual: usize, expected: usize, label: &str) -> anyhow::Result<()> {
    if actual == expected {
        return Ok(());
    }
    bail!("expected {label} {expected}, got {actual}")
}

const fn operations_per_batch(benchmark: EmbeddingBenchmark) -> usize {
    match benchmark {
        EmbeddingBenchmark::SyncCall => SYNC_CALLS_PER_BATCH,
        EmbeddingBenchmark::PropertyGet => PROPERTY_GETS_PER_BATCH,
        EmbeddingBenchmark::RustCallback => RUST_CALLBACKS_PER_BATCH,
        EmbeddingBenchmark::AsyncCompletion => ASYNC_COMPLETIONS_PER_BATCH,
        EmbeddingBenchmark::HostObjectPayload => HOST_PAYLOAD_READS_PER_BATCH,
    }
}

fn render_lifecycle(lifecycle: BenchmarkLifecycle) -> String {
    format!(
        "load=-;compile=-;setup={};warmup={};run={};verify=-;teardown={}",
        optional_duration(lifecycle.setup),
        timing::format_duration(lifecycle.warmup),
        timing::format_duration(lifecycle.timed_run),
        optional_duration(lifecycle.teardown),
    )
}

fn optional_duration(duration: Option<Duration>) -> String {
    duration.map_or_else(|| "-".to_owned(), timing::format_duration)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{EmbeddingBenchmark, measure, run};
    use crate::{
        bench_measure::MeasureConfig,
        benchmark_protocol::{BenchmarkMode, BenchmarkReferenceSource},
        cases::BenchmarkCase,
    };

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn direct_embedding_cases_run_through_the_public_facade() -> TestResult {
        let config = MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3)
            .with_quality(Duration::ZERO, u32::MAX)
            .with_budget(Duration::from_secs(2), Duration::from_secs(10));
        for benchmark in [
            EmbeddingBenchmark::SyncCall,
            EmbeddingBenchmark::PropertyGet,
            EmbeddingBenchmark::RustCallback,
            EmbeddingBenchmark::AsyncCompletion,
            EmbeddingBenchmark::HostObjectPayload,
        ] {
            let measured = measure(benchmark, config)?;
            if measured.stats.total_iters() == 0 {
                return Err(format!("embedding benchmark {benchmark:?} ran no batches").into());
            }
        }
        Ok(())
    }

    #[test]
    fn embedding_report_records_direct_api_methodology() -> TestResult {
        let config = MeasureConfig::new(Duration::ZERO, Duration::from_millis(1), 3)
            .with_quality(Duration::ZERO, u32::MAX)
            .with_budget(Duration::from_secs(2), Duration::from_secs(10));
        let case = BenchmarkCase::embedding(
            "embedding_test",
            "runner/src/embedding_benchmarks.rs",
            EmbeddingBenchmark::PropertyGet,
        );
        let outcome = run(&case, EmbeddingBenchmark::PropertyGet, config);
        if outcome.row.mode != "embedding_api"
            || outcome.row.methodology.mode != Some(BenchmarkMode::EmbeddingApi)
            || outcome.row.methodology.lifecycle.is_none()
            || outcome.row.methodology.reference_source
                != Some(BenchmarkReferenceSource::NotConfigured)
            || !outcome.row.detail.contains("operations_per_batch=20000")
        {
            return Err(format!("unexpected embedding benchmark row: {:?}", outcome.row).into());
        }
        Ok(())
    }
}
