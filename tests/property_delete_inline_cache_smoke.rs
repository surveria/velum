use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiled_static_delete_cache_invalidates_missing_after_shape_change() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = {};
        var clear = function () {
            return delete holder.slot;
        };
        var first = clear();
        holder.slot = 7;
        var second = clear();
        var third = clear();
        first && second && third && !("slot" in holder) ? 42 : 0;
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 3)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 3)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_static_delete_cache_preserves_non_configurable_descriptor() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = {};
        Object.defineProperty(holder, "fixed", {
            value: 9,
            configurable: false
        });
        var clear = function () {
            return delete holder.fixed;
        };
        var first = clear();
        var second = clear();
        first === false && second === false && holder.fixed === 9 ? 42 : 0;
        "#,
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_dynamic_delete_cache_invalidates_missing_after_shape_change() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = {};
        var key = "slot";
        var clear = function () {
            return delete holder[key];
        };
        var first = clear();
        holder[key] = 7;
        var second = clear();
        var third = clear();
        first && second && third && !(key in holder) ? 42 : 0;
        "#,
    )?;

    ensure_usize(script.usage().static_property_access_count(), 3)?;
    ensure_usize(script.usage().bytecode_property_operand_count(), 3)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected usize {expected}, got {actual}").into())
}
