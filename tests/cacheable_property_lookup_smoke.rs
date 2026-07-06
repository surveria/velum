use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_named_lookup_across_own_and_prototype_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.eval(
        r"
        let proto = { shared: 10 };
        let child = { own: 5 };
        child.__proto__ = proto;
        child.own + child.shared
        ",
    )?;
    ensure_value(&value, &Value::Number(15.0))?;
    let linked_version = vm.resource_usage().prototype_lookup_version;

    let value = vm.eval(r#""own" in child && "shared" in child"#)?;
    ensure_value(&value, &Value::Bool(true))?;
    ensure_u64(vm.resource_usage().prototype_lookup_version, linked_version)?;

    let value = vm.eval(
        r"
        proto.shared = 20;
        child.own + child.shared
        ",
    )?;
    ensure_value(&value, &Value::Number(25.0))?;
    ensure_u64(vm.resource_usage().prototype_lookup_version, linked_version)
}

#[test]
fn falls_back_for_array_specific_properties_without_losing_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.eval(
        r"
        let proto = { named: 7 };
        let items = [2, 3, 5];
        items.__proto__ = proto;
        items[0] + items.length + items.named
        ",
    )?;
    ensure_value(&value, &Value::Number(12.0))?;

    let value = vm.eval(r#""0" in items && "length" in items && "named" in items"#)?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn observes_structure_changes_after_cacheable_lookup_candidates() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.eval(
        r"
        let proto = { shared: 1 };
        let child = {};
        child.__proto__ = proto;
        child.shared
        ",
    )?;
    ensure_value(&value, &Value::Number(1.0))?;
    let linked_version = vm.resource_usage().prototype_lookup_version;

    let value = vm.eval("child.shared = 2; child.shared")?;
    ensure_value(&value, &Value::Number(2.0))?;
    let shadow_version = vm.resource_usage().prototype_lookup_version;
    ensure_greater_than_u64(shadow_version, linked_version, "shadow property version")?;

    let value = vm.eval("delete child.shared; child.shared")?;
    ensure_value(&value, &Value::Number(1.0))?;
    ensure_greater_than_u64(
        vm.resource_usage().prototype_lookup_version,
        shadow_version,
        "delete property version",
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_u64(actual: u64, expected: u64) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_greater_than_u64(actual: u64, minimum: u64, label: &str) -> TestResult {
    if actual > minimum {
        return Ok(());
    }
    Err(format!("expected {label} greater than {minimum}, got {actual}").into())
}
