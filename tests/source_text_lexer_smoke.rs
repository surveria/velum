use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn hashbang_comment_runs_only_at_source_start() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"#!/usr/bin/env rsqjs
40 + 2
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;

    let Err(error) = context.eval(";\n#!\n1") else {
        return Err("expected hashbang after the source start to fail".into());
    };
    ensure_error_contains(&error, "unexpected character '#'")
}

#[test]
fn eval_accepts_hashbang_comment_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(r"eval('#!\n40 + 2')")?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn unicode_and_escaped_identifiers_bind_by_decoded_name() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
var \u{61} = 1;
var \u0062 = 2;
var café = 3;
var 一 = 4;
var withJoin\u200cPart = 5;
a + b + café + 一 + withJoin\u200cPart
        ",
    )?;

    ensure_value(&value, &Value::Number(15.0))
}

#[test]
fn escaped_identifier_start_must_decode_to_identifier_start() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval(r"var \u0030bad = 1") else {
        return Err("expected escaped digit identifier start to fail".into());
    };

    ensure_error_contains(&error, "invalid identifier character '0'")
}

#[test]
fn reserved_words_are_not_binding_identifiers_but_can_name_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
debugger;
let holder = { class: 20, import: 10, with: 5, export: 7 };
holder.class + holder.import + holder.with + holder['export']
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;

    let Err(error) = context.eval(r"var w\u0069th = 1") else {
        return Err("expected escaped reserved binding identifier to fail".into());
    };

    ensure_error_contains(&error, "expected binding name")
}

#[test]
fn escaped_async_is_not_contextual_async_keyword() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
var \u0061sync = 40;
async + 2
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;

    let Err(error) = context.eval(r"\u0061sync function f(){}") else {
        return Err("expected escaped async function spelling to fail".into());
    };

    ensure_error_contains(&error, "expected statement terminator")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &rs_quickjs::Error, expected: &str) -> TestResult {
    let actual = error.to_string();
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error containing {expected:?}, got {actual:?}").into())
}
