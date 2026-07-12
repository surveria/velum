use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_parenthesized_exponentiation_and_targets() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let power = 2 ** 3 ** 2;
        let grouped = (-2) ** 2;
        let negated = -(2 ** 2);
        print(power, grouped, negated);

        let value = 2;
        let assigned = (value) **= 3;
        let old = (value)++;
        let current = ++(value);
        print(assigned, old, current, value);

        let target = { slot: 4 };
        let propPower = (target.slot) **= 2;
        print(propPower, target.slot);

        let missingType = typeof (missing);
        let deleteMissing = delete (missing);
        let object = { value: 1 };
        let deleteProperty = delete (object.value);
        print(missingType, deleteMissing, deleteProperty, object.value);

        let choose = function(value) {
            return value;
        };
        let called = (choose)(42);
        print(called);

        power === 512 &&
            grouped === 4 &&
            negated === -4 &&
            assigned === 8 &&
            old === 8 &&
            current === 10 &&
            value === 10 &&
            propPower === 16 &&
            target.slot === 16 &&
            missingType === 'undefined' &&
            deleteMissing === true &&
            deleteProperty === true &&
            object.value === undefined &&
            called === 42
        ",
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "512 4 -4".to_owned(),
            "8 8 10 10".to_owned(),
            "16 16".to_owned(),
            "undefined true true undefined".to_owned(),
            "42".to_owned(),
        ],
    )
}

#[test]
fn exponentiation_special_cases_match_math_pow_across_execution_tiers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        function exponentiate(left, right) {
            return left ** right;
        }
        let generic = ({ valueOf: function() { return 1; } }) ** NaN;
        let functionFast = exponentiate(-1, Infinity);
        let flatMapFast = [1].flatMap(function(value) {
            return [value ** NaN];
        })[0];
        let linear = 0;
        for (let index = 0; index < 4; index = index + 1) {
            linear = 1 ** NaN;
        }

        Number.isNaN(generic) &&
            Number.isNaN(functionFast) &&
            Number.isNaN(flatMapFast) &&
            Number.isNaN(linear) &&
            Number.isNaN(Math.pow(1, NaN)) &&
            Number.isNaN(Math.pow(-1, Infinity))
            ? 42
            : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_unparenthesized_unary_left_exponentiation() -> TestResult {
    ensure_error_contains("-2 ** 2", "unary expression cannot be the left operand")?;
    ensure_error_contains(
        "typeof missing ** 2",
        "unary expression cannot be the left operand",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    error_contains(&error, expected)
}

fn error_contains(error: &Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error '{message}' to contain '{expected}'").into())
}
