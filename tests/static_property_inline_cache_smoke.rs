use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn cached_static_property_reads_preserve_receiver_identity() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval(
        r"
        var first = { slot: 1 };
        var second = { slot: 40 };
        ",
    )?;
    let script = vm.compile(
        r"
        first.slot + second.slot
        ",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))
}

#[test]
fn cached_static_property_reads_follow_shape_and_prototype_changes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval(
        r"
        var proto = { slot: 40 };
        var child = { __proto__: proto };
        ",
    )?;
    let script = vm.compile("child.slot")?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(40.0))?;

    let value = vm.context().eval("proto.slot = 41; child.slot")?;
    ensure_value(&value, &Value::Number(41.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))?;

    let value = vm.context().eval("child.slot = 42; child.slot")?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let value = vm.context().eval("delete child.slot; child.slot")?;
    ensure_value(&value, &Value::Number(41.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))
}

#[test]
fn cached_static_property_reads_keep_missing_and_proto_paths_correct() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval(
        r"
        var proto = { base: 42 };
        var child = { __proto__: proto };
        ",
    )?;
    let missing = vm.compile("child.missing")?;
    let proto_read = vm.compile("child.__proto__.base")?;

    let value = vm.eval_compiled(&missing)?;
    ensure_value(&value, &Value::Undefined)?;
    let value = vm.eval_compiled(&missing)?;
    ensure_value(&value, &Value::Undefined)?;

    let value = vm.eval_compiled(&proto_read)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let value = vm.eval_compiled(&proto_read)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
