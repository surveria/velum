use velum::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiled_static_property_reads_have_occurrence_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval(
        r"
        var first = { slot: 1 };
        var second = { slot: 40 };
        ",
    )?;
    let script = vm.compile("first.slot + second.slot + first.slot")?;

    ensure_usize(script.usage().static_property_access_count(), 3)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn static_member_call_reference_uses_guarded_access_site_cache() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval(
        r"
        var proto = { method: function () { return this.v + 1; } };
        var first = { __proto__: proto, v: 1 };
        var second = { __proto__: proto, v: 40 };
        ",
    )?;
    let script = vm.compile("first.method() + second.method()")?;

    ensure_usize(script.usage().static_property_access_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(43.0))?;

    let value = vm
        .context()
        .eval("proto.method = function () { return this.v + 2; }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(45.0))?;

    let value = vm
        .context()
        .eval("first.method = function () { return this.v + 10; }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(53.0))
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
