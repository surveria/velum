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
    engine262_worker::Engine262Worker,
    node_worker::NodeWorker,
    time::{duration_millis_u64, duration_nanos_u64},
};

const DIFFERENTIAL_MAX_CALL_STACK_BYTES: usize = 984 * 1_024;
const ECMASCRIPT_ERROR_NAMES: [&str; 8] = [
    "AggregateError",
    "ReferenceError",
    "SyntaxError",
    "RangeError",
    "TypeError",
    "EvalError",
    "URIError",
    "Error",
];
const RESIZABLE_ARRAY_BUFFER_MARKER: &str = "maxByteLength";
const LEXER_ERROR_PREFIX: &str = "lexer error";
const PARSER_ERROR_PREFIX: &str = "parser error";
const RESOURCE_MANAGEMENT_KEYWORD: &str = "using";
const RESOURCE_FOR_OF_KEYWORD: &str = "of";
const SYNTAX_ERROR_NAME: &str = "SyntaxError";
const VELUM_RESOURCE_LIMIT_PREFIX: &str = "resource limit exceeded:";

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

impl Default for EngineOutcome {
    fn default() -> Self {
        outcome(
            OutcomeStatus::Crash,
            0,
            "",
            Some("Unavailable".to_owned()),
            Some("Outcome is not available in this record schema".to_owned()),
        )
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
    #[serde(default)]
    pub findings: Vec<CaseFinding>,
    pub saved_script: Option<String>,
    #[serde(default)]
    pub saved_scripts: Vec<String>,
    pub ratio_velum_to_v8: Option<f64>,
    pub velum: EngineOutcome,
    #[serde(default)]
    pub engine262: EngineOutcome,
    pub v8: EngineOutcome,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseClassification {
    Match,
    #[serde(alias = "mismatch")]
    CorrectnessMismatch,
    #[serde(alias = "slow")]
    PerformanceSlow,
    VelumTimeout,
    VelumCrash,
    VelumResourceLimit,
    Engine262Timeout,
    Engine262Crash,
    Engine262Unsupported,
    V8Timeout,
    V8Crash,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseFinding {
    CorrectnessMismatch,
    PerformanceSlow,
    VelumTimeout,
    VelumCrash,
    VelumResourceLimit,
    Engine262Timeout,
    Engine262Crash,
    Engine262Unsupported,
    V8Timeout,
    V8Crash,
}

#[derive(Debug, Clone, Copy)]
pub struct CompareConfig {
    pub engine262_timeout: Duration,
    pub v8_timeout: Duration,
    pub slow_ratio: f64,
    pub slow_min: Duration,
}

/// Executes one script in Velum, Engine262, and V8, then classifies the result.
///
/// # Errors
///
/// Returns an error when reference-engine communication fails in a
/// non-recoverable way or Velum host callback setup fails.
pub fn compare_script(
    source: &str,
    engine262_worker: &mut Engine262Worker,
    node_worker: &mut NodeWorker,
    config: CompareConfig,
) -> anyhow::Result<ComparedScript> {
    let velum = execute_velum(source)?;
    let engine262 =
        engine262_worker.execute(source, duration_millis_u64(config.engine262_timeout))?;
    let v8 = node_worker.execute(source, duration_millis_u64(config.v8_timeout))?;
    let ratio = timing_ratio(&velum, &v8);
    let findings = findings(source, &velum, &engine262, &v8, ratio, config);
    let classification = primary_classification(&findings);
    Ok(ComparedScript {
        velum,
        engine262,
        v8,
        ratio,
        classification,
        findings,
    })
}

#[derive(Debug)]
pub struct ComparedScript {
    pub velum: EngineOutcome,
    pub engine262: EngineOutcome,
    pub v8: EngineOutcome,
    pub ratio: Option<f64>,
    pub classification: CaseClassification,
    pub findings: Vec<CaseFinding>,
}

fn findings(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
    ratio: Option<f64>,
    config: CompareConfig,
) -> Vec<CaseFinding> {
    let mut findings = Vec::new();
    if velum.status == OutcomeStatus::Timeout {
        findings.push(CaseFinding::VelumTimeout);
    }
    if velum.status == OutcomeStatus::Crash {
        findings.push(CaseFinding::VelumCrash);
    }
    let velum_resource_limit = is_velum_resource_limit(velum);
    if velum_resource_limit {
        findings.push(CaseFinding::VelumResourceLimit);
    }
    if engine262.status == OutcomeStatus::Timeout {
        findings.push(CaseFinding::Engine262Timeout);
    }
    if engine262.status == OutcomeStatus::Crash {
        findings.push(CaseFinding::Engine262Crash);
    }
    if v8.status == OutcomeStatus::Timeout {
        findings.push(CaseFinding::V8Timeout);
    }
    if v8.status == OutcomeStatus::Crash {
        findings.push(CaseFinding::V8Crash);
    }
    let engine262_unsupported = is_engine262_unsupported(source, velum, engine262, v8);
    if engine262_unsupported {
        findings.push(CaseFinding::Engine262Unsupported);
    }
    let correctness_oracle = correctness_oracle(source, engine262, v8, engine262_unsupported);
    if let Some(correctness_oracle) = correctness_oracle
        && velum.is_completed()
        && !velum_resource_limit
        && correctness_oracle.is_completed()
        && !equivalent(velum, correctness_oracle)
    {
        findings.push(CaseFinding::CorrectnessMismatch);
    }
    if let Some(value) = ratio
        && value >= config.slow_ratio
        && velum.elapsed_nanos >= duration_nanos_u64(config.slow_min)
    {
        findings.push(CaseFinding::PerformanceSlow);
    }
    findings
}

fn primary_classification(findings: &[CaseFinding]) -> CaseClassification {
    for candidate in [
        CaseFinding::VelumCrash,
        CaseFinding::VelumTimeout,
        CaseFinding::VelumResourceLimit,
        CaseFinding::CorrectnessMismatch,
        CaseFinding::Engine262Crash,
        CaseFinding::Engine262Timeout,
        CaseFinding::V8Crash,
        CaseFinding::V8Timeout,
        CaseFinding::Engine262Unsupported,
        CaseFinding::PerformanceSlow,
    ] {
        if findings.contains(&candidate) {
            return match candidate {
                CaseFinding::CorrectnessMismatch => CaseClassification::CorrectnessMismatch,
                CaseFinding::PerformanceSlow => CaseClassification::PerformanceSlow,
                CaseFinding::VelumTimeout => CaseClassification::VelumTimeout,
                CaseFinding::VelumCrash => CaseClassification::VelumCrash,
                CaseFinding::VelumResourceLimit => CaseClassification::VelumResourceLimit,
                CaseFinding::Engine262Timeout => CaseClassification::Engine262Timeout,
                CaseFinding::Engine262Crash => CaseClassification::Engine262Crash,
                CaseFinding::Engine262Unsupported => CaseClassification::Engine262Unsupported,
                CaseFinding::V8Timeout => CaseClassification::V8Timeout,
                CaseFinding::V8Crash => CaseClassification::V8Crash,
            };
        }
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

fn is_engine262_unsupported(
    source: &str,
    velum: &EngineOutcome,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    is_engine262_missing_global(engine262)
        || (source.contains(RESIZABLE_ARRAY_BUFFER_MARKER)
            && !equivalent(velum, engine262)
            && !equivalent(engine262, v8))
        || is_reference_unsupported_resource_management(source, engine262, v8)
        || (engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
            && !equivalent(velum, engine262)
            && !equivalent(engine262, v8))
}

fn is_velum_resource_limit(velum: &EngineOutcome) -> bool {
    velum.status == OutcomeStatus::JsError
        && velum.error_name.as_deref() == Some("Error")
        && velum
            .error_message
            .as_deref()
            .is_some_and(|message| message.starts_with(VELUM_RESOURCE_LIMIT_PREFIX))
}

fn correctness_oracle<'a>(
    source: &str,
    engine262: &'a EngineOutcome,
    v8: &'a EngineOutcome,
    engine262_unsupported: bool,
) -> Option<&'a EngineOutcome> {
    if !engine262_unsupported {
        return Some(engine262);
    }
    if is_reference_unsupported_resource_management(source, engine262, v8) {
        return None;
    }
    if is_v8_missing_global(v8) {
        return None;
    }
    Some(v8)
}

fn is_reference_unsupported_resource_management(
    source: &str,
    engine262: &EngineOutcome,
    v8: &EngineOutcome,
) -> bool {
    references_reject_as_syntax(engine262, v8) && source_contains_resource_management_syntax(source)
}

fn references_reject_as_syntax(engine262: &EngineOutcome, v8: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && v8.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
        && v8.error_name.as_deref() == Some(SYNTAX_ERROR_NAME)
}

fn source_contains_resource_management_syntax(source: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative_start) = source
        .get(search_start..)
        .and_then(|tail| tail.find(RESOURCE_MANAGEMENT_KEYWORD))
    {
        let start = search_start.saturating_add(relative_start);
        let end = start.saturating_add(RESOURCE_MANAGEMENT_KEYWORD.len());
        if is_keyword_boundary(source, start, end)
            && resource_binding_follows_using(source.get(end..).unwrap_or_default())
        {
            return true;
        }
        search_start = end;
    }
    false
}

fn is_keyword_boundary(source: &str, start: usize, end: usize) -> bool {
    let previous = source
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back());
    let next = source.get(end..).and_then(|suffix| suffix.chars().next());
    !previous.is_some_and(is_ascii_identifier_part) && !next.is_some_and(is_ascii_identifier_part)
}

fn resource_binding_follows_using(rest: &str) -> bool {
    let tail = rest.trim_start();
    let Some(first) = tail.chars().next() else {
        return false;
    };
    if !is_ascii_identifier_start(first) {
        return false;
    }
    let name_end = tail
        .char_indices()
        .find_map(|(index, value)| (!is_ascii_identifier_part(value)).then_some(index))
        .unwrap_or(tail.len());
    let Some(after_name) = tail.get(name_end..) else {
        return false;
    };
    let after_name = after_name.trim_start();
    after_name.starts_with('=') || starts_with_word(after_name, RESOURCE_FOR_OF_KEYWORD)
}

fn starts_with_word(source: &str, word: &str) -> bool {
    let Some(after) = source.get(word.len()..) else {
        return false;
    };
    source.starts_with(word) && !after.chars().next().is_some_and(is_ascii_identifier_part)
}

const fn is_ascii_identifier_start(value: char) -> bool {
    value == '_' || value == '$' || value.is_ascii_alphabetic()
}

const fn is_ascii_identifier_part(value: char) -> bool {
    is_ascii_identifier_start(value) || value.is_ascii_digit()
}

fn is_engine262_missing_global(engine262: &EngineOutcome) -> bool {
    engine262.status == OutcomeStatus::JsError
        && engine262.error_name.as_deref() == Some("ReferenceError")
        && engine262
            .error_message
            .as_deref()
            .is_some_and(is_engine262_missing_global_message)
}

fn is_engine262_missing_global_message(message: &str) -> bool {
    message.contains("\"Intl\" is not defined")
        || message.contains("Intl is not defined")
        || message.contains("\"SharedArrayBuffer\" is not defined")
        || message.contains("SharedArrayBuffer is not defined")
}

fn is_v8_missing_global(v8: &EngineOutcome) -> bool {
    v8.status == OutcomeStatus::JsError
        && v8.error_name.as_deref() == Some("ReferenceError")
        && v8
            .error_message
            .as_deref()
            .is_some_and(is_v8_missing_global_message)
}

fn is_v8_missing_global_message(message: &str) -> bool {
    message.contains("Iterator is not defined")
        || message.contains("AsyncIterator is not defined")
        || message.contains("DisposableStack is not defined")
        || message.contains("AsyncDisposableStack is not defined")
        || message.contains("SuppressedError is not defined")
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
    let trimmed = message.trim_start();
    if trimmed.starts_with(LEXER_ERROR_PREFIX) || trimmed.starts_with(PARSER_ERROR_PREFIX) {
        return SYNTAX_ERROR_NAME.to_owned();
    }
    for (start, _) in message.char_indices() {
        for name in ECMASCRIPT_ERROR_NAMES {
            let Some(candidate) = message.get(start..) else {
                continue;
            };
            if candidate.starts_with(name) && is_error_name_match(message, start, name.len()) {
                return name.to_owned();
            }
        }
    }
    "Error".to_owned()
}

fn is_error_name_match(message: &str, start: usize, length: usize) -> bool {
    let end = start.saturating_add(length);
    let Some(after) = message.get(end..) else {
        return false;
    };
    let follows_error_name = after.is_empty() || after.starts_with(':');
    if !follows_error_name {
        return false;
    }
    let previous = message
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back());
    !previous.is_some_and(|value| value.is_ascii_alphanumeric() || value == '_')
}

#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::ensure;

    use super::{
        CaseFinding, CompareConfig, OutcomeStatus, SYNTAX_ERROR_NAME, error_name_from_text,
        findings, outcome,
    };

    fn config() -> CompareConfig {
        CompareConfig {
            engine262_timeout: Duration::from_secs(30),
            v8_timeout: Duration::from_secs(4),
            slow_ratio: 2.0,
            slow_min: Duration::from_millis(5),
        }
    }

    #[test]
    fn error_name_parser_extracts_nested_js_error() -> anyhow::Result<()> {
        let name =
            error_name_from_text("javascript exception: TypeError: constructor requires 'new'");
        ensure!(name == "TypeError", "unexpected error name: {name}");
        Ok(())
    }

    #[test]
    fn error_name_parser_preserves_primary_reference_error() -> anyhow::Result<()> {
        let name = error_name_from_text("ReferenceError: \"Intl\" is not defined");
        ensure!(name == "ReferenceError", "unexpected error name: {name}");
        Ok(())
    }

    #[test]
    fn error_name_parser_maps_lexer_errors_to_syntax_error() -> anyhow::Result<()> {
        let name = error_name_from_text(
            "lexer error at 11: invalid regular expression pattern: RegExp compile error",
        );
        ensure!(name == SYNTAX_ERROR_NAME, "unexpected error name: {name}");
        Ok(())
    }

    #[test]
    fn engine262_intl_gap_is_not_a_correctness_mismatch() -> anyhow::Result<()> {
        let velum = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("RangeError".to_owned()),
            None,
        );
        let engine262 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("ReferenceError".to_owned()),
            Some("ReferenceError: \"Intl\" is not defined".to_owned()),
        );
        let v8 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("RangeError".to_owned()),
            None,
        );
        let findings = findings(
            "new Intl.Segmenter('ckb_IR')",
            &velum,
            &engine262,
            &v8,
            None,
            config(),
        );
        ensure!(findings.contains(&CaseFinding::Engine262Unsupported));
        ensure!(!findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }

    #[test]
    fn engine262_unsupported_falls_back_to_v8_for_correctness() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("ReferenceError".to_owned()),
            Some("ReferenceError: \"SharedArrayBuffer\" is not defined".to_owned()),
        );
        let v8 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("RangeError".to_owned()),
            None,
        );
        let findings = findings(
            "new BigInt64Array(new SharedArrayBuffer(6, { maxByteLength: 6 }))",
            &velum,
            &engine262,
            &v8,
            None,
            config(),
        );
        ensure!(findings.contains(&CaseFinding::Engine262Unsupported));
        ensure!(findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }

    #[test]
    fn engine262_syntax_gap_falls_back_to_v8() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some(SYNTAX_ERROR_NAME.to_owned()),
            Some("SyntaxError: Unexpected token".to_owned()),
        );
        let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let findings = findings(
            "const value = /G4}9\\111?/dm;",
            &velum,
            &engine262,
            &v8,
            None,
            config(),
        );
        ensure!(findings.contains(&CaseFinding::Engine262Unsupported));
        ensure!(!findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }

    #[test]
    fn resource_management_syntax_gap_is_not_a_correctness_mismatch() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some(SYNTAX_ERROR_NAME.to_owned()),
            Some("SyntaxError: Unexpected token".to_owned()),
        );
        let v8 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some(SYNTAX_ERROR_NAME.to_owned()),
            Some("Unexpected identifier 'value'".to_owned()),
        );
        let findings = findings(
            "for (using value of []) {}",
            &velum,
            &engine262,
            &v8,
            None,
            config(),
        );
        ensure!(findings.contains(&CaseFinding::Engine262Unsupported));
        ensure!(!findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }

    #[test]
    fn velum_resource_limit_is_not_a_correctness_mismatch() -> anyhow::Result<()> {
        let velum = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("Error".to_owned()),
            Some("resource limit exceeded: runtime steps exceeded 100000".to_owned()),
        );
        let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let v8 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let findings = findings("for (;;) {}", &velum, &engine262, &v8, None, config());
        ensure!(findings.contains(&CaseFinding::VelumResourceLimit));
        ensure!(!findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }

    #[test]
    fn missing_v8_fallback_global_is_not_a_correctness_mismatch() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("ReferenceError".to_owned()),
            Some("ReferenceError: \"SharedArrayBuffer\" is not defined".to_owned()),
        );
        let v8 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("ReferenceError".to_owned()),
            Some("Iterator is not defined".to_owned()),
        );
        let findings = findings(
            "new SharedArrayBuffer(8); Iterator.zip(new Map());",
            &velum,
            &engine262,
            &v8,
            None,
            config(),
        );
        ensure!(findings.contains(&CaseFinding::Engine262Unsupported));
        ensure!(!findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }

    #[test]
    fn matching_engine262_result_is_not_replaced_by_v8_fallback() -> anyhow::Result<()> {
        let velum = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let engine262 = outcome(OutcomeStatus::Ok, 1, "", None, None);
        let v8 = outcome(
            OutcomeStatus::JsError,
            1,
            "",
            Some("TypeError".to_owned()),
            Some("Math.f16round is not a function".to_owned()),
        );
        let findings = findings(
            "new ArrayBuffer(5, { maxByteLength: 10 }); Math.f16round(1);",
            &velum,
            &engine262,
            &v8,
            None,
            config(),
        );
        ensure!(!findings.contains(&CaseFinding::Engine262Unsupported));
        ensure!(!findings.contains(&CaseFinding::CorrectnessMismatch));
        Ok(())
    }
}
