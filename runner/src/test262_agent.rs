use std::{
    collections::VecDeque,
    mem,
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use parking_lot::{Condvar, Mutex};
use velum::{Context, Error, Runtime, SharedArrayBufferHandle, Value};

use crate::{test262_compat_harness, test262_metadata::test262_limits};

pub const START_HOST_NAME: &str = "__velumTest262AgentStart";
pub const BROADCAST_HOST_NAME: &str = "__velumTest262AgentBroadcast";
pub const GET_REPORT_HOST_NAME: &str = "__velumTest262AgentGetReport";
pub const WAIT_REPORT_HOST_NAME: &str = "__velumTest262AgentWaitReport";
pub const REPORT_HOST_NAME: &str = "__velumTest262AgentReport";
pub const SLEEP_HOST_NAME: &str = "__velumTest262AgentSleep";
pub const MONOTONIC_NOW_HOST_NAME: &str = "__velumTest262AgentMonotonicNow";
const BROADCAST_BINDING_NAME: &str = "__velumTest262AgentBroadcastValue";
const AGENT_FAILURE_PREFIX: &str = "Test262AgentFailure:";
const RECEIVE_BROADCAST_MARKER: &str = "$262.agent.receiveBroadcast";
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;
const REPORT_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

const ASYNC_REPORT_SOURCE: &str = r"
$262.agent.getReportAsync = function getReportAsync() {
    return Promise.resolve(__velumTest262AgentWaitReport());
};
";

const WORKER_HOST_SOURCE: &str = r"
var $262 = {
    global: globalThis,
    agent: {
        receiveBroadcast: function receiveBroadcast(callback) {
            return callback(__velumTest262AgentBroadcastValue);
        },
        report: function report(value) {
            return __velumTest262AgentReport(String(value));
        },
        leaving: function leaving() {},
        sleep: __velumTest262AgentSleep,
        monotonicNow: __velumTest262AgentMonotonicNow
    }
};
";

type WorkerResult = Result<(), String>;

#[derive(Debug)]
pub struct Test262AgentCoordinator {
    sources: Mutex<Vec<String>>,
    reports: Mutex<VecDeque<String>>,
    report_ready: Condvar,
    workers: Mutex<Vec<JoinHandle<WorkerResult>>>,
    started_at: Instant,
}

impl Test262AgentCoordinator {
    pub fn install(context: &mut Context) -> velum::Result<Arc<Self>> {
        let coordinator = Arc::new(Self {
            sources: Mutex::new(Vec::new()),
            reports: Mutex::new(VecDeque::new()),
            report_ready: Condvar::new(),
            workers: Mutex::new(Vec::new()),
            started_at: Instant::now(),
        });
        install_start(context, &coordinator)?;
        install_broadcast(context, &coordinator)?;
        install_get_report(context, &coordinator)?;
        install_wait_report(context, &coordinator)?;
        install_sleep(context)?;
        install_monotonic_now(context, &coordinator)?;
        Ok(coordinator)
    }

    pub fn finish(&self) -> anyhow::Result<()> {
        let workers = mem::take(&mut *self.workers.lock());
        let mut failures = Vec::new();
        for worker in workers {
            match worker.join() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => failures.push(error),
                Err(_) => failures.push("Test262 agent thread terminated unexpectedly".to_owned()),
            }
        }
        if failures.is_empty() {
            return Ok(());
        }
        anyhow::bail!("{}", failures.join("; "))
    }

    fn start(self: &Arc<Self>, source: String) -> velum::Result<()> {
        if source.contains(RECEIVE_BROADCAST_MARKER) {
            self.sources.lock().push(source);
            return Ok(());
        }
        self.spawn_worker(source, None)
    }

    fn broadcast(self: &Arc<Self>, handle: &SharedArrayBufferHandle) -> velum::Result<()> {
        let sources = mem::take(&mut *self.sources.lock());
        for source in sources {
            self.spawn_worker(source, Some(handle.clone()))?;
        }
        Ok(())
    }

    fn spawn_worker(
        self: &Arc<Self>,
        source: String,
        handle: Option<SharedArrayBufferHandle>,
    ) -> velum::Result<()> {
        let index = self.workers.lock().len();
        let worker_coordinator = self.clone();
        let worker = thread::Builder::new()
            .name(format!("velum-test262-agent-{index}"))
            .spawn(move || {
                let result = run_worker(&source, handle.as_ref(), &worker_coordinator);
                if let Err(error) = &result {
                    worker_coordinator.report(format!("{AGENT_FAILURE_PREFIX}{error}"));
                }
                result
            })
            .map_err(|error| Error::runtime(format!("failed to start Test262 agent: {error}")))?;
        self.workers.lock().push(worker);
        Ok(())
    }

    fn get_report(&self) -> Option<String> {
        self.reports.lock().pop_front()
    }

    fn report(&self, report: String) {
        self.reports.lock().push_back(report);
        self.report_ready.notify_one();
    }

    fn wait_report(&self) -> Option<String> {
        let mut reports = self.reports.lock();
        if reports.is_empty() {
            self.report_ready
                .wait_for(&mut reports, REPORT_WAIT_TIMEOUT);
        }
        reports.pop_front()
    }

    fn monotonic_milliseconds(&self) -> f64 {
        self.started_at.elapsed().as_secs_f64() * MILLISECONDS_PER_SECOND
    }
}

pub fn install_async_report(context: &mut Context) -> velum::Result<()> {
    context.eval(ASYNC_REPORT_SOURCE).map(|_value| ())
}

fn install_start(
    context: &mut Context,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> velum::Result<()> {
    let state = coordinator.clone();
    context.register_host_function_typed(START_HOST_NAME, move |call| {
        state.start(call.string(0, "source")?.to_owned())
    })
}

fn install_broadcast(
    context: &mut Context,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> velum::Result<()> {
    let state = coordinator.clone();
    context.register_host_function_typed(BROADCAST_HOST_NAME, move |call| {
        let handle = call
            .required_value(0, "sharedBuffer")?
            .to_shared_array_buffer()?;
        state.broadcast(&handle)
    })
}

fn install_get_report(
    context: &mut Context,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> velum::Result<()> {
    let state = coordinator.clone();
    context.register_host_function(GET_REPORT_HOST_NAME, move |_call| {
        Ok(state.get_report().map_or(Value::Null, Value::from))
    })
}

fn install_wait_report(
    context: &mut Context,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> velum::Result<()> {
    let state = coordinator.clone();
    context.register_host_function(WAIT_REPORT_HOST_NAME, move |_call| {
        Ok(state.wait_report().map_or(Value::Null, Value::from))
    })
}

fn install_report(
    context: &mut Context,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> velum::Result<()> {
    let state = coordinator.clone();
    context.register_host_function_typed(REPORT_HOST_NAME, move |call| {
        state.report(call.string(0, "report")?.to_owned());
        Ok(())
    })
}

fn install_sleep(context: &mut Context) -> velum::Result<()> {
    context.register_host_function_typed(SLEEP_HOST_NAME, |call| {
        if let Some(duration) = sleep_duration(call.number(0, "milliseconds")?) {
            thread::sleep(duration);
        }
        Ok(())
    })
}

fn install_monotonic_now(
    context: &mut Context,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> velum::Result<()> {
    let state = coordinator.clone();
    context.register_host_function_typed(MONOTONIC_NOW_HOST_NAME, move |_call| {
        Ok(state.monotonic_milliseconds())
    })
}

fn sleep_duration(milliseconds: f64) -> Option<Duration> {
    if !milliseconds.is_finite() || milliseconds <= 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(milliseconds / MILLISECONDS_PER_SECOND).ok()
}

fn run_worker(
    source: &str,
    handle: Option<&SharedArrayBufferHandle>,
    coordinator: &Arc<Test262AgentCoordinator>,
) -> WorkerResult {
    let runtime = Runtime::with_limits(test262_limits());
    let mut context = runtime.context();
    context.set_agent_can_block(true);
    if let Some(handle) = handle {
        context
            .register_shared_array_buffer(BROADCAST_BINDING_NAME, handle)
            .map_err(|error| error.to_string())?;
    }
    install_report(&mut context, coordinator).map_err(|error| error.to_string())?;
    install_sleep(&mut context).map_err(|error| error.to_string())?;
    install_monotonic_now(&mut context, coordinator).map_err(|error| error.to_string())?;
    context
        .eval(test262_compat_harness::STA_SOURCE)
        .map_err(|error| error.to_string())?;
    context
        .eval(test262_compat_harness::AGENT_ASSERT_SOURCE)
        .map_err(|error| error.to_string())?;
    context
        .eval(WORKER_HOST_SOURCE)
        .map_err(|error| error.to_string())?;
    context.eval(source).map_err(|error| error.to_string())?;
    context.run_jobs().map_err(|error| error.to_string())?;
    if context.output().is_empty() {
        return Ok(());
    }
    Err("Test262 agent produced unexpected host output".to_owned())
}
