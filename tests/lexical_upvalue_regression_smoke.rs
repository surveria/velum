use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn hoisted_function_captures_later_lexical_binding() -> TestResult {
    ensure_eval(
        r"
        function readLater() {
            function read() { return later; }
            const later = 42;
            return read();
        }
        readLater()
        ",
        &Value::Number(42.0),
    )
}

#[test]
fn out_of_order_upvalue_visits_do_not_truncate_higher_slots() -> TestResult {
    ensure_eval(
        r"
        function makeReader(left, readMiddle, right) {
            return function() {
                return left + readMiddle() + right + readMiddle();
            };
        }
        makeReader(10, function() { return 5; }, 22)()
        ",
        &Value::Number(42.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if &value != expected {
        return Err(format!(
            "expected {expected:?}, received {value:?}; output: {:?}",
            context.output()
        )
        .into());
    }
    Ok(())
}
