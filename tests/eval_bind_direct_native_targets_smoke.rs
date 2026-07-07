use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn eval_and_bind_compile_to_direct_native_targets() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        function add(left, right) {
            return left + right;
        }

        let bound = add.bind(null, 40);
        let evalResult = eval("21 + 21");

        bound(2) + evalResult
        "#,
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(84.0))
}

#[test]
fn eval_and_bind_direct_targets_preserve_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        Function.prototype.bind = function() {
            return function() {
                return "patched";
            };
        };

        {
            let eval = function(source) {
                return "shadow:" + source;
            };
            function add(left, right) {
                return left + right;
            }

            let bound = add.bind(null, 1);

            eval("x") === "shadow:x" &&
                bound() === "patched"
                    ? 42
                    : 0
        }
        "#,
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 2)?;
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
