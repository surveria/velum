use rs_quickjs::{Error, Runtime, RuntimeLimits, SourceId, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn eval_lexer_and_parser_failures_are_catchable_syntax_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let direct;
        let indirect;
        let alias = eval;
        try {
            eval("@");
        } catch (error) {
            direct = error instanceof SyntaxError && error.name === "SyntaxError";
        }
        try {
            alias("break missingLabel");
        } catch (error) {
            indirect = error instanceof SyntaxError && error.name === "SyntaxError";
        }
        direct && indirect
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn dynamic_function_constructors_share_the_syntax_error_boundary() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let ordinary;
        let asynchronous;
        try {
            Function("}");
        } catch (error) {
            ordinary = error instanceof SyntaxError;
        }
        try {
            Object.getPrototypeOf(async function() {}).constructor("}");
        } catch (error) {
            asynchronous = error instanceof SyntaxError;
        }
        let lexical;
        try {
            Function("@error");
        } catch (error) {
            lexical = error instanceof SyntaxError;
        }
        ordinary && asynchronous && lexical
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn uncaught_eval_syntax_errors_keep_the_dynamic_source_span() -> TestResult {
    const DYNAMIC_SOURCE: &str = "@";

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval("eval('@')") else {
        return Err("expected eval syntax error".into());
    };

    ensure_error_name(&error, "SyntaxError")?;
    let Some(span) = error.source_span() else {
        return Err("expected eval syntax error source span".into());
    };
    if span.source_id() != SourceId::for_source(DYNAMIC_SOURCE) {
        return Err(format!(
            "expected dynamic source id {}, got {}",
            SourceId::for_source(DYNAMIC_SOURCE),
            span.source_id()
        )
        .into());
    }
    let expected_offset = DYNAMIC_SOURCE.len();
    if span.start() != expected_offset || span.end() != expected_offset {
        return Err(format!(
            "expected dynamic EOF span {expected_offset}..{expected_offset}, got {span:?}"
        )
        .into());
    }
    Ok(())
}

#[test]
fn eval_resource_limits_remain_engine_failures() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_source_len: 30,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let Err(error) = context.eval(r#"eval("x".repeat(100))"#) else {
        return Err("expected eval source limit failure".into());
    };

    if matches!(error, Error::ResourceLimit { .. }) {
        return Ok(());
    }
    Err(format!("expected resource limit, got {error}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_name(error: &Error, expected: &str) -> TestResult {
    if error.javascript_error_name() == Some(expected) {
        return Ok(());
    }
    Err(format!("expected JavaScript {expected}, got {error}").into())
}
