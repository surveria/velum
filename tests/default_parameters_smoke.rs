use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn applies_default_parameter_when_argument_is_missing_or_undefined() -> TestResult {
    let value = eval(
        r"
        function pick(value = 41) {
            return value + 1;
        }
        pick() + pick(undefined) + pick(9);
        ",
    )?;
    ensure_value(&value, &Value::Number(94.0))
}

#[test]
fn default_parameter_can_read_previous_parameter_and_outer_binding() -> TestResult {
    let value = eval(
        r"
        let base = 5;
        function add(left, right = left + base) {
            return right;
        }
        add(7);
        ",
    )?;
    ensure_value(&value, &Value::Number(12.0))
}

#[test]
fn supports_default_parameter_trailing_comma_and_function_length() -> TestResult {
    let value = eval(
        r"
        function combine(left, right = 3,) {
            return left + right + combine.length;
        }
        combine(4);
        ",
    )?;
    ensure_value(&value, &Value::Number(8.0))
}

#[test]
fn async_function_uses_default_parameter_before_body() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        async function answer(value = 40) {
            return value + 2;
        }
        let resolved = 0;
        answer(undefined).then(function(value) {
            resolved = value;
        });
        ",
    )?;
    let value = context.eval("resolved")?;
    ensure_value(&value, &Value::Number(42.0))
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
