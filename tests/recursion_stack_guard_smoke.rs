use velum::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn catches_plain_recursive_call_stack_exhaustion() -> TestResult {
    ensure_source_returns_42(
        r"
        let exhausted = false;
        function f0(a1, a2, a3, a4) {
            try { f0(); } catch (error) {
                exhausted = error instanceof RangeError;
            }
        }
        f0(f0, f0, f0, f0);
        exhausted ? 42 : 0;
        ",
    )
}

#[test]
fn catches_rest_parameter_recursive_call_stack_exhaustion() -> TestResult {
    ensure_source_returns_42(
        r"
        let exhausted = false;
        function f0(a1, ...a2) {
            try { f0(); } catch (error) {
                exhausted = error instanceof RangeError;
            }
        }
        f0(f0, f0, f0);
        exhausted ? 42 : 0;
        ",
    )
}

#[test]
fn catches_recursive_derived_constructor_call_stack_exhaustion() -> TestResult {
    ensure_source_returns_42(
        r"
        let exhausted = false;
        class C1 extends Uint32Array {
            constructor(a3, a4, a5) {
                try { new C1(); } catch (error) {
                    if (error instanceof RangeError) {
                        exhausted = true;
                    }
                }
            }
        }
        try { new C1(); } catch (error) {}
        exhausted ? 42 : 0;
        ",
    )
}

#[test]
fn lets_embedders_tighten_the_native_stack_budget() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_call_depth: usize::MAX,
        max_call_stack_bytes: 1,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let value = context.eval(
        r"
        function recurse() {
            return recurse();
        }
        try {
            recurse();
        } catch (error) {
            error instanceof RangeError ? 42 : 0;
        }
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_source_returns_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
