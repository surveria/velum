use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_labeled_break_completion_value() -> TestResult {
    expect_value("test262id: { 5; break test262id; 9; }", &Value::Number(5.0))?;
    expect_value("while (true) { 7; break; 9; }", &Value::Number(7.0))
}

#[test]
fn rejects_continue_to_non_iteration_label() -> TestResult {
    expect_error_contains(
        r"
        do {
            test262: {
                continue test262;
            }
        } while (false);
        ",
        "continue target is not an iteration statement",
    )
}

#[test]
fn rejects_labeled_lexical_and_async_function_declarations() -> TestResult {
    expect_error_contains(
        "label: let x;",
        "lexical declaration is not allowed as a label body",
    )?;
    expect_error_contains(
        "label: const x = null;",
        "lexical declaration is not allowed as a label body",
    )?;
    expect_error_contains(
        "label: async function f() {}",
        "async function declaration is not allowed as a label body",
    )
}

#[test]
fn rejects_break_and_continue_to_missing_labels() -> TestResult {
    expect_error_contains("break missing;", "break target label is not defined")?;
    expect_error_contains(
        "while (true) { continue missing; }",
        "continue target label is not defined",
    )
}

#[test]
fn rejects_strict_future_reserved_labels() -> TestResult {
    for label in [
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
    ] {
        let expected = if label == "let" {
            "expected binding name"
        } else {
            "reserved word"
        };
        expect_error_contains(
            &format!("\"use strict\"; {label}: while (false) {{}}"),
            expected,
        )?;
    }
    Ok(())
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn expect_value(source: &str, expected: &Value) -> TestResult {
    let actual = eval(source)?;
    if &actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn expect_error_contains(source: &str, expected: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected source to fail with '{expected}'").into());
    };
    let actual = error.to_string();
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{actual}' to contain '{expected}'").into())
}
