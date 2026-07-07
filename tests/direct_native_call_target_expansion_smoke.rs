use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiled_calls_mark_additional_direct_native_targets() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        function add_one(value) {
            return value + 1;
        }

        let direct_symbol = typeof Symbol("token") === "symbol";
        let own_names = Object.getOwnPropertyNames({ alpha: 1, beta: 2 }).length;
        let regexp_result = RegExp("a").test("cat");
        let call_result = add_one.call(null, 41);

        direct_symbol &&
            own_names === 2 &&
            regexp_result &&
            call_result === 42
                ? 42
                : 0
        "#,
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 5)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn additional_direct_targets_preserve_runtime_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        Object.getOwnPropertyNames = function() {
            return ["custom"];
        };
        RegExp.prototype.test = function() {
            return "patched";
        };
        Function.prototype.call = function() {
            return "called";
        };

        {
            let Symbol = function(value) {
                return "symbol:" + value;
            };
            let names = Object.getOwnPropertyNames({ alpha: 1 });
            function sample() {
                return 1;
            }

            names[0] === "custom" &&
                RegExp("a").test("a") === "patched" &&
                sample.call(null) === "called" &&
                Symbol("x") === "symbol:x"
                    ? 42
                    : 0
        }
        "#,
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 4)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_min_usize(actual: usize, expected_minimum: usize) -> TestResult {
    if actual >= expected_minimum {
        return Ok(());
    }

    Err(format!("expected at least {expected_minimum}, got {actual}").into())
}
