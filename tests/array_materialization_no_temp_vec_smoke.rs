use velum::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn array_literal_and_direct_constructor_preserve_ordered_elements() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var order = "";
        var mark = function(label, value) {
            order = order + label;
            return value;
        };

        var literal = [mark("a", 1), mark("b", 2), mark("c", 3)];
        var constructed = Array(mark("d", 4), mark("e", 5), mark("f", 6));

        literal.join("|") === "1|2|3" &&
            constructed.join("|") === "4|5|6" &&
            order === "abcdef" ? 42 : 0
        "#,
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
