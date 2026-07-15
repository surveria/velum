use velum::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn cached_static_property_update_and_compound_preserve_repeated_state() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval("let holder = { slot: 10 }; 0")?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("holder.slot++; ++holder.slot; holder.slot += 5; holder.slot")?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(17.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(24.0))
}

#[test]
fn cached_dynamic_property_update_and_compound_preserve_repeated_state() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r#"
        let holder = { slot: 3 };
        function key() { return "slot"; }
        0
        "#,
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("holder[key()]++; holder[key()] += 4; holder[key()]")?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(8.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(13.0))
}

#[test]
fn cached_proto_name_updates_use_ordinary_accessor_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r#"
        let getterHits = 0;
        let setterHits = 0;
        let stored = 20;
        let holder = {};
        Object.defineProperty(holder, "__proto__", {
            get() { getterHits = getterHits + 1; return stored; },
            set(value) { setterHits = setterHits + 1; stored = value; },
            configurable: true
        });
        function key() { return "__proto__"; }
        0
        "#,
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let static_script = vm.compile("holder.__proto__ += 1; holder.__proto__")?;
    let value = vm.eval_compiled(&static_script)?;
    ensure_value(&value, &Value::Number(21.0))?;
    let value = vm.eval_compiled(&static_script)?;
    ensure_value(&value, &Value::Number(22.0))?;

    let dynamic_script = vm.compile("holder[key()] += 1; holder[key()]")?;
    let value = vm.eval_compiled(&dynamic_script)?;
    ensure_value(&value, &Value::Number(23.0))?;
    let value = vm.eval_compiled(&dynamic_script)?;
    ensure_value(&value, &Value::Number(24.0))?;

    let value = vm.eval(
        "getterHits * 10 + setterHits + \
         (Object.getPrototypeOf(holder) === Object.prototype ? 0 : 1000)",
    )?;
    ensure_value(&value, &Value::Number(84.0))
}

#[test]
fn cached_static_compound_falls_back_for_prototype_hits() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r"
        let proto = { slot: 10 };
        let holder = {};
        holder.__proto__ = proto;
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("holder.slot += 2; holder.slot")?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(12.0))?;
    let value = vm.eval("proto.slot")?;
    ensure_value(&value, &Value::Number(10.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(14.0))
}

#[test]
fn cached_static_compound_preserves_non_writable_descriptor_result() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r"
        let holder = {};
        Object.defineProperty(holder, 'slot', {
            value: 4,
            writable: false,
            enumerable: true,
            configurable: true
        });
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let script = vm.compile("(holder.slot += 3) * 10 + holder.slot")?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(74.0))?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(74.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
