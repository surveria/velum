use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let actual = eval(source)?;
    if actual == Value::String(expected.to_owned()) {
        return Ok(());
    }
    Err(format!("expected string {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected {source:?} to fail").into());
    };
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error {message:?} to contain {expected:?}").into())
}

#[test]
fn evaluates_sequence_operands_left_to_right_and_returns_the_last_value() -> TestResult {
    ensure_string(
        r#"
        let trace = "";
        let value = (trace = trace + "a", trace = trace + "b", 42);
        trace + ":" + value
        "#,
        "ab:42",
    )
}

#[test]
fn preserves_sequence_precedence_and_full_expression_contexts() -> TestResult {
    ensure_string(
        r#"
        function selected() {
            return 1, 2, 3;
        }
        let target = {first: 1, second: 2};
        "" + (1 + (2, 3)) + ":" + target["first", "second"] + ":"
            + (0, selected)()
        "#,
        "4:2:3",
    )
}

#[test]
fn keeps_assignment_expression_delimiters_outside_parenthesized_sequences() -> TestResult {
    ensure_string(
        r#"
        function pair(left, right) {
            return left + ":" + right;
        }
        let first = (1, 2), second = 3;
        let values = [(4, 5), 6];
        let object = {[(7, "key")]: (8, 9), other: 10};
        pair(first, second) + ":" + values[0] + ":" + values[1] + ":"
            + object.key + ":" + object.other
        "#,
        "2:3:5:6:9:10",
    )
}

#[test]
fn sequence_expression_is_not_an_assignment_target() -> TestResult {
    ensure_error_contains(
        "let a = 1; let b = 2; (a, b) = 3;",
        "invalid assignment target",
    )
}

#[test]
fn exposes_surrounding_early_errors_after_a_leading_sequence_operand() -> TestResult {
    ensure_error_contains(
        r#"0, ([element]) => { "use strict"; };"#,
        "use strict directive is not allowed with non-simple parameters",
    )?;
    ensure_error_contains("0, (left, left) => {};", "duplicate parameter name")?;
    ensure_error_contains("0, class { method(value = yield) {} };", "yield")?;
    ensure_error_contains("0, function () { await 1; };", "await expression")?;
    ensure_error_contains(
        "let value; for (value of [], []) {}",
        "expected ')' after for-in",
    )
}
