use velum::{Engine, Error, Runtime, RuntimeLimits, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn nested_numeric_function_fast_paths_preserve_bitwise_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var conversions = 0;
        var boxed = {
            valueOf: function() {
                conversions = conversions + 1;
                return 3;
            }
        };
        function step(value) {
            return ((value * 5) + 11) & 65535;
        }
        step(1) === 16 && step(-1) === 6 && step(boxed) === 26 &&
            conversions === 1 ? 42 : 0
        ",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn nested_numeric_function_fast_paths_preserve_runtime_step_limits() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_runtime_steps: 48,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let error = context
        .eval(
            r"
            function step(value) {
                return ((value * 5) + 11) & 65535;
            }
            step(1);
            step(2);
            step(3);
            step(4);
            step(5);
            step(6);
            step(7);
            step(8);
            ",
        )
        .err()
        .ok_or("expected nested numeric fast path runtime step limit to fail")?;
    ensure_resource_limit(&error)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_resource_limit(error: &Error) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) {
        return Ok(());
    }
    Err(format!("expected resource limit error, got {error}").into())
}
