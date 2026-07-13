use rs_quickjs::{Error, Runtime, RuntimeLimits, SourceId, SourceSpan, Value, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const NAMED_SOURCE: &str = "let answer = 40 + 2; answer";
const SOURCE_NAME: &str = "app/main.js";

#[test]
fn keeps_stable_source_identity_without_retaining_source_text() -> TestResult {
    let runtime = Runtime::new();
    let first = runtime.compile_named(SOURCE_NAME, NAMED_SOURCE)?;
    let second = runtime.compile_named(SOURCE_NAME, NAMED_SOURCE)?;
    let anonymous = runtime.compile(NAMED_SOURCE)?;
    let renamed = runtime.compile_named("app/renamed.js", NAMED_SOURCE)?;
    let changed = runtime.compile_named(SOURCE_NAME, "let answer = 43; answer")?;

    ensure_source_id(
        first.source_id(),
        SourceId::for_named_source(SOURCE_NAME, NAMED_SOURCE),
    )?;
    ensure_source_id(first.source_id(), second.source_id())?;
    ensure_optional_text(first.source_name(), Some(SOURCE_NAME))?;
    ensure_optional_text(anonymous.source_name(), None)?;
    ensure_different_source_id(first.source_id(), anonymous.source_id())?;
    ensure_different_source_id(first.source_id(), renamed.source_id())?;
    ensure_different_source_id(first.source_id(), changed.source_id())?;
    ensure_different_source_id(
        SourceId::for_named_source("a\u{2}", "b"),
        SourceId::for_named_source("a", "\u{2}b"),
    )?;
    ensure_source_id(anonymous.source_id(), SourceId::for_source(NAMED_SOURCE))
}

#[test]
fn preserves_named_identity_when_a_script_runs_in_multiple_vms() -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile_named(SOURCE_NAME, NAMED_SOURCE)?;
    let expected_id = script.source_id();
    let cloned = script.clone();
    let mut first_vm = Vm::new();
    let mut second_vm = Vm::new();

    ensure_value(&first_vm.eval_compiled(&script)?, &Value::Number(42.0))?;
    ensure_value(&second_vm.eval_compiled(&cloned)?, &Value::Number(42.0))?;
    ensure_source_id(script.source_id(), expected_id)?;
    ensure_source_id(cloned.source_id(), expected_id)
}

#[test]
fn exposes_named_lexer_diagnostic_as_a_utf8_byte_span() -> TestResult {
    let vm = Vm::new();
    let source = "§";
    let Err(error) = vm.compile_named("invalid-token.js", source) else {
        return Err("expected invalid token compilation to fail".into());
    };
    let Error::Lex { message, span } = &error else {
        return Err(format!("expected lexer error, got {error}").into());
    };

    ensure_text_contains(message, "unexpected character")?;
    ensure_source_id(
        span.source_id(),
        SourceId::for_named_source("invalid-token.js", source),
    )?;
    ensure_usize(span.start(), 0)?;
    ensure_usize(span.end(), source.len())?;
    ensure_optional_span(error.source_span(), Some(*span))?;
    ensure_text_contains(&error.to_string(), "lexer error at 0:")
}

#[test]
fn advances_diagnostics_by_utf8_byte_boundaries() -> TestResult {
    let runtime = Runtime::new();
    let source_name = "unicode-invalid-token.js";
    let source = "let π = 'λ';\n§";
    let Err(error) = runtime.compile_named(source_name, source) else {
        return Err("expected invalid token after Unicode source to fail".into());
    };
    let Error::Lex { span, .. } = error else {
        return Err(format!("expected lexer error, got {error}").into());
    };
    let expected_start = source
        .len()
        .checked_sub('§'.len_utf8())
        .ok_or("Unicode diagnostic start underflowed")?;
    ensure_source_id(
        span.source_id(),
        SourceId::for_named_source(source_name, source),
    )?;
    ensure_usize(span.start(), expected_start)?;
    ensure_usize(span.end(), source.len())
}

#[test]
fn exposes_named_parser_diagnostic_at_end_of_source() -> TestResult {
    let runtime = Runtime::new();
    let source = "let camera =";
    let Err(error) = runtime.compile_named("incomplete.js", source) else {
        return Err("expected incomplete source compilation to fail".into());
    };
    let Error::Parse { message, span } = &error else {
        return Err(format!("expected parser error, got {error}").into());
    };

    ensure_text_contains(message, "expected expression")?;
    ensure_source_id(
        span.source_id(),
        SourceId::for_named_source("incomplete.js", source),
    )?;
    ensure_usize(span.start(), source.len())?;
    ensure_usize(span.end(), source.len())?;
    if !span.is_empty() {
        return Err("expected an end-of-source point span".into());
    }
    ensure_optional_span(error.source_span(), Some(*span))
}

#[test]
fn exposes_the_complete_unexpected_parser_token_range() -> TestResult {
    let runtime = Runtime::new();
    let source = "let camera = if";
    let Err(error) = runtime.compile_named("unexpected-keyword.js", source) else {
        return Err("expected unexpected keyword compilation to fail".into());
    };
    let Error::Parse { message, span } = &error else {
        return Err(format!("expected parser error, got {error}").into());
    };

    ensure_text_contains(message, "expected expression")?;
    ensure_source_id(
        span.source_id(),
        SourceId::for_named_source("unexpected-keyword.js", source),
    )?;
    ensure_usize(span.start(), source.len().saturating_sub(2))?;
    ensure_usize(span.end(), source.len())?;
    if span.is_empty() {
        return Err("expected the complete keyword token range".into());
    }
    Ok(())
}

#[test]
fn preserves_source_span_when_adding_error_context() -> TestResult {
    let runtime = Runtime::new();
    let source = "@";
    let Err(error) = runtime.compile(source) else {
        return Err("expected invalid anonymous source compilation to fail".into());
    };
    let expected_span = error
        .source_span()
        .ok_or("expected a source span before adding context")?;
    let contextual = error.with_context("embedded module");

    ensure_optional_span(contextual.source_span(), Some(expected_span))?;
    ensure_source_id(expected_span.source_id(), SourceId::for_source(source))?;
    ensure_text_contains(&contextual.to_string(), "embedded module")
}

#[test]
fn exposes_the_executing_identifier_span_on_runtime_reference_errors() -> TestResult {
    let runtime = Runtime::new();
    let source_name = "runtime-reference.js";
    let source = "let ready = true;\nmissingCamera;";
    let expected = named_marker_span(source_name, source, "missingCamera")?;
    let script = runtime.compile_named(source_name, source)?;
    let mut context = runtime.context();
    let Err(error) = context.eval_compiled(&script) else {
        return Err("expected missing binding evaluation to fail".into());
    };

    ensure_optional_text(error.javascript_error_name(), Some("ReferenceError"))?;
    ensure_optional_span(error.source_span(), Some(expected))?;
    let metadata = error
        .javascript_error_metadata()
        .ok_or("expected built-in Error metadata")?;
    ensure_optional_span(metadata.source_span(), Some(expected))
}

#[test]
fn exposes_the_executing_call_span_on_host_runtime_failures() -> TestResult {
    let runtime = Runtime::new();
    let source_name = "host-runtime.js";
    let source = "hostFail();";
    let expected = named_marker_span(source_name, source, "hostFail()")?;
    let script = runtime.compile_named(source_name, source)?;
    let mut context = runtime.context();
    context.register_host_function("hostFail", |_call| Err(Error::runtime("camera offline")))?;
    let Err(error) = context.eval_compiled(&script) else {
        return Err("expected host runtime failure".into());
    };

    if !matches!(error, Error::Runtime { .. }) {
        return Err(format!("expected Runtime error, got {error:?}").into());
    }
    ensure_optional_span(error.source_span(), Some(expected))
}

#[test]
fn preserves_the_resource_limit_channel_with_an_executing_span() -> TestResult {
    let runtime = Runtime::new();
    let source_name = "host-limit.js";
    let source = "hostLimit();";
    let expected = named_marker_span(source_name, source, "hostLimit()")?;
    let script = runtime.compile_named(source_name, source)?;
    let mut context = runtime.context();
    context.register_host_function("hostLimit", |_call| {
        Err(Error::limit("host budget exhausted"))
    })?;
    let Err(error) = context.eval_compiled(&script) else {
        return Err("expected host resource limit".into());
    };

    if !matches!(error, Error::ResourceLimit { .. }) {
        return Err(format!("expected ResourceLimit error, got {error:?}").into());
    }
    ensure_optional_span(error.source_span(), Some(expected))
}

#[test]
fn exposes_the_throw_statement_span_for_primitive_values() -> TestResult {
    let runtime = Runtime::new();
    let source_name = "primitive-throw.js";
    let source = "throw 42;";
    let expected = named_marker_span(source_name, source, source)?;
    let script = runtime.compile_named(source_name, source)?;
    let mut context = runtime.context();
    let Err(error) = context.eval_compiled(&script) else {
        return Err("expected primitive throw".into());
    };

    ensure_value(
        error
            .javascript_value()
            .ok_or("expected original thrown value")?,
        &Value::Number(42.0),
    )?;
    ensure_optional_span(error.source_span(), Some(expected))
}

#[test]
fn preserves_the_inner_error_origin_across_function_frames() -> TestResult {
    let runtime = Runtime::new();
    let source_name = "nested-error.js";
    let source = concat!(
        "function fail() { throw new TypeError(\"camera\"); }\n",
        "fail();"
    );
    let marker = "throw new TypeError(\"camera\");";
    let expected = named_marker_span(source_name, source, marker)?;
    let script = runtime.compile_named(source_name, source)?;
    let mut context = runtime.context();
    let Err(error) = context.eval_compiled(&script) else {
        return Err("expected nested TypeError".into());
    };

    ensure_optional_text(error.javascript_error_name(), Some("TypeError"))?;
    ensure_optional_span(error.source_span(), Some(expected))?;
    let metadata = error
        .javascript_error_metadata()
        .ok_or("expected nested Error metadata")?;
    ensure_optional_span(metadata.source_span(), Some(expected))
}

#[test]
fn enforces_source_name_limits_for_compile_and_execution() -> TestResult {
    let permissive_runtime = Runtime::new();
    let script = permissive_runtime.compile_named("long-name.js", "42")?;
    let limits = RuntimeLimits {
        max_string_len: 4,
        ..RuntimeLimits::default()
    };
    let constrained_runtime = Runtime::with_limits(limits.clone());
    let Err(compile_error) = constrained_runtime.compile_named("large", "42") else {
        return Err("expected source name compile limit to fail".into());
    };
    ensure_resource_limit(&compile_error)?;

    let mut constrained_vm = Vm::with_config(VmConfig::with_limits(limits));
    let Err(execution_error) = constrained_vm.eval_compiled(&script) else {
        return Err("expected compiled source name limit to fail".into());
    };
    ensure_resource_limit(&execution_error)
}

#[test]
fn rejects_reversed_public_source_spans() -> TestResult {
    let source_id = SourceId::for_source("42");
    if SourceSpan::new(source_id, 2, 1).is_none() {
        return Ok(());
    }
    Err("expected reversed source span construction to fail".into())
}

fn ensure_source_id(actual: SourceId, expected: SourceId) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected source id {expected}, got {actual}").into())
}

fn ensure_different_source_id(left: SourceId, right: SourceId) -> TestResult {
    if left != right {
        return Ok(());
    }
    Err(format!("expected distinct source ids, both were {left}").into())
}

fn ensure_optional_text(actual: Option<&str>, expected: Option<&str>) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected source name {expected:?}, got {actual:?}").into())
}

fn ensure_optional_span(actual: Option<SourceSpan>, expected: Option<SourceSpan>) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected source span {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_text_contains(actual: &str, expected: &str) -> TestResult {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected '{actual}' to contain '{expected}'").into())
}

fn ensure_resource_limit(error: &Error) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) {
        return Ok(());
    }
    Err(format!("expected resource limit error, got {error}").into())
}

fn named_marker_span(source_name: &str, source: &str, marker: &str) -> TestResultSpan {
    let start = source
        .find(marker)
        .ok_or_else(|| format!("marker {marker:?} is not present in source"))?;
    let end = start
        .checked_add(marker.len())
        .ok_or("source marker range overflowed")?;
    SourceSpan::new(SourceId::for_named_source(source_name, source), start, end)
        .ok_or_else(|| "source marker range is reversed".into())
}

type TestResultSpan = std::result::Result<SourceSpan, Box<dyn std::error::Error>>;
