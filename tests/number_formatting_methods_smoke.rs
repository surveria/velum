use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn to_fixed_rounds_half_up() -> TestResult {
    eval_is_42(
        r#"
        (0).toFixed(0) === "0" &&
            (2.5).toFixed(0) === "3" &&
            (0.5).toFixed(0) === "1" &&
            (1.005).toFixed(2) === "1.00" &&
            (8.575).toFixed(2) === "8.57" &&
            (123.456).toFixed(2) === "123.46" &&
            (-1.23).toFixed(1) === "-1.2" &&
            (0).toFixed(3) === "0.000" &&
            (9.999).toFixed(2) === "10.00"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn to_fixed_falls_back_to_to_string_for_large_and_non_finite() -> TestResult {
    eval_is_42(
        r#"
        (1e21).toFixed(2) === "1e+21" &&
            (NaN).toFixed(2) === "NaN" &&
            (Infinity).toFixed(2) === "Infinity" &&
            (-Infinity).toFixed(2) === "-Infinity"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn to_exponential_formats_mantissa_and_exponent() -> TestResult {
    eval_is_42(
        r#"
        (123.456).toExponential(2) === "1.23e+2" &&
            (0.0001234).toExponential(2) === "1.23e-4" &&
            (5).toExponential(0) === "5e+0" &&
            (0).toExponential(4) === "0.0000e+0" &&
            (123.456).toExponential() === "1.23456e+2" &&
            (100).toExponential() === "1e+2"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn to_precision_selects_fixed_or_exponential() -> TestResult {
    eval_is_42(
        r#"
        (123.456).toPrecision(4) === "123.5" &&
            (0.0001234).toPrecision(2) === "0.00012" &&
            (123456).toPrecision(3) === "1.23e+5" &&
            (1.5).toPrecision(1) === "2" &&
            (100).toPrecision(5) === "100.00" &&
            (0).toPrecision(3) === "0.00" &&
            (123.456).toPrecision() === "123.456"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn methods_reject_out_of_range_digit_arguments() -> TestResult {
    eval_is_42(
        r"
        let count = 0;
        try { (1).toFixed(-1); } catch (e) { if (e instanceof RangeError) count += 1; }
        try { (1).toFixed(101); } catch (e) { if (e instanceof RangeError) count += 1; }
        try { (1).toExponential(101); } catch (e) { if (e instanceof RangeError) count += 1; }
        try { (1).toPrecision(0); } catch (e) { if (e instanceof RangeError) count += 1; }
        try { (1).toPrecision(101); } catch (e) { if (e instanceof RangeError) count += 1; }
        count === 5 ? 42 : 0
        ",
    )
}

#[test]
fn number_to_string_uses_ecmascript_notation() -> TestResult {
    eval_is_42(
        r#"
        String(1e21) === "1e+21" &&
            String(1e20) === "100000000000000000000" &&
            String(1e-6) === "0.000001" &&
            String(1e-7) === "1e-7" &&
            String(0.1) === "0.1" &&
            String(-0) === "0" &&
            String(123456789) === "123456789" &&
            Number.MIN_VALUE === 5e-324 &&
            String(Number.MIN_VALUE) === "5e-324"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn accepts_boxed_number_receivers() -> TestResult {
    eval_is_42(
        r#"
        Number.prototype.toFixed.call(new Number(3.14159), 2) === "3.14" &&
            Number.prototype.toExponential.call(new Number(1234), 1) === "1.2e+3" &&
            Number.prototype.toPrecision.call(new Number(0.5), 3) === "0.500"
            ? 42
            : 0
        "#,
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
