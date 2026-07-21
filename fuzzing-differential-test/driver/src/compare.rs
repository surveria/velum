use std::{
    cell::RefCell,
    fmt::Write as _,
    rc::Rc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use velum::{Error, Runtime, RuntimeLimits};

use crate::{
    node_worker::NodeWorker,
    time::{duration_millis_u64, duration_nanos_u64},
};

const DIFFERENTIAL_MAX_CALL_STACK_BYTES: usize = 984 * 1_024;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatus {
    Ok,
    JsError,
    Timeout,
    Crash,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineOutcome {
    pub status: OutcomeStatus,
    pub elapsed_nanos: u64,
    pub stdout_sha256: String,
    pub stdout_bytes: u64,
    pub error_name: Option<String>,
    pub error_message: Option<String>,
}

impl EngineOutcome {
    #[must_use]
    pub const fn is_completed(&self) -> bool {
        matches!(self.status, OutcomeStatus::Ok | OutcomeStatus::JsError)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CaseRecord {
    pub case_id: String,
    pub worker_pid: u32,
    pub sequence: u64,
    pub script_sha256: String,
    pub script_bytes: u64,
    pub classification: CaseClassification,
    pub saved_script: Option<String>,
    pub ratio_velum_to_v8: Option<f64>,
    pub velum: EngineOutcome,
    pub v8: EngineOutcome,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseClassification {
    Match,
    Mismatch,
    Slow,
    V8Timeout,
    V8Crash,
}

#[derive(Debug, Clone, Copy)]
pub struct CompareConfig {
    pub engine_timeout: Duration,
    pub slow_ratio: f64,
    pub slow_min: Duration,
}

/// Executes one script in Velum and V8, then classifies the result.
///
/// # Errors
///
/// Returns an error when V8 communication fails in a non-recoverable way or
/// Velum host callback setup fails.
pub fn compare_script(
    source: &str,
    node_worker: &mut NodeWorker,
    config: CompareConfig,
) -> anyhow::Result<ComparedScript> {
    let velum = execute_velum(source)?;
    let v8 = node_worker.execute(source, duration_millis_u64(config.engine_timeout))?;
    let ratio = timing_ratio(&velum, &v8);
    let classification = classify(&velum, &v8, ratio, config);
    Ok(ComparedScript {
        velum,
        v8,
        ratio,
        classification,
    })
}

#[derive(Debug)]
pub struct ComparedScript {
    pub velum: EngineOutcome,
    pub v8: EngineOutcome,
    pub ratio: Option<f64>,
    pub classification: CaseClassification,
}

fn classify(
    velum: &EngineOutcome,
    v8: &EngineOutcome,
    ratio: Option<f64>,
    config: CompareConfig,
) -> CaseClassification {
    if v8.status == OutcomeStatus::Timeout {
        return CaseClassification::V8Timeout;
    }
    if v8.status == OutcomeStatus::Crash {
        return CaseClassification::V8Crash;
    }
    if !equivalent(velum, v8) {
        return CaseClassification::Mismatch;
    }
    if let Some(value) = ratio
        && value >= config.slow_ratio
        && velum.elapsed_nanos >= duration_nanos_u64(config.slow_min)
    {
        return CaseClassification::Slow;
    }
    CaseClassification::Match
}

fn equivalent(velum: &EngineOutcome, v8: &EngineOutcome) -> bool {
    if velum.status != v8.status {
        return false;
    }
    match velum.status {
        OutcomeStatus::Ok => velum.stdout_sha256 == v8.stdout_sha256,
        OutcomeStatus::JsError => velum.error_name == v8.error_name,
        OutcomeStatus::Timeout | OutcomeStatus::Crash => true,
    }
}

fn timing_ratio(velum: &EngineOutcome, v8: &EngineOutcome) -> Option<f64> {
    if !velum.is_completed() || !v8.is_completed() || v8.elapsed_nanos == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    Some(velum.elapsed_nanos as f64 / v8.elapsed_nanos as f64)
}

fn execute_velum(source: &str) -> anyhow::Result<EngineOutcome> {
    let started = Instant::now();
    let output = Rc::new(RefCell::new(String::new()));
    let output_sink = Rc::clone(&output);
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_call_stack_bytes: DIFFERENTIAL_MAX_CALL_STACK_BYTES,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    context
        .register_host_function_typed("fuzzilli", move |call| {
            let operation = call.string(0, "operation")?;
            if operation != "FUZZILLI_PRINT" {
                return Ok(());
            }
            let value = call.required_value(1, "value")?;
            let mut output = output_sink.try_borrow_mut().map_err(|error| {
                Error::runtime(format!("fuzz output is already borrowed: {error}"))
            })?;
            writeln!(output, "{}", value.as_value())
                .map_err(|error| Error::runtime(format!("failed to write fuzz output: {error}")))?;
            Ok(())
        })
        .map_err(|error| {
            anyhow::anyhow!("failed to register the Fuzzilli host callback: {error}")
        })?;

    let result = context.eval(source);
    drop(context.take_output());
    let elapsed_nanos = duration_nanos_u64(started.elapsed());
    let stdout = output.borrow().clone();
    match result {
        Ok(_value) => Ok(outcome(
            OutcomeStatus::Ok,
            elapsed_nanos,
            &stdout,
            None,
            None,
        )),
        Err(error) => {
            let message = error.to_string();
            Ok(outcome(
                OutcomeStatus::JsError,
                elapsed_nanos,
                &stdout,
                Some(error_name_from_text(&message)),
                Some(message),
            ))
        }
    }
}

#[must_use]
pub fn outcome(
    status: OutcomeStatus,
    elapsed_nanos: u64,
    stdout: &str,
    error_name: Option<String>,
    error_message: Option<String>,
) -> EngineOutcome {
    EngineOutcome {
        status,
        elapsed_nanos,
        stdout_sha256: sha256_hex(stdout.as_bytes()),
        stdout_bytes: u64::try_from(stdout.len()).unwrap_or(u64::MAX),
        error_name,
        error_message,
    }
}

#[must_use]
pub fn error_name_from_text(message: &str) -> String {
    let Some((name, _)) = message.split_once(':') else {
        return "Error".to_owned();
    };
    if name.ends_with("Error") {
        return name.to_owned();
    }
    "Error".to_owned()
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
