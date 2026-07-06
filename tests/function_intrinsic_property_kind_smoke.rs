use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn intrinsic_function_property_kind_paths_preserve_metadata() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r"
        let sample = function sample(left, right) { return left + right; };
        let native = Math.max;
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;
    let declared_atoms = vm.resource_usage().atom_count;

    let value = vm.context().eval(
        r"
        sample.name;
        sample.length;
        sample.prototype;
        native.name;
        native.length;
        native.prototype;
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;
    ensure_usize(vm.resource_usage().atom_count, declared_atoms)?;

    let value = vm.context().eval(
        r"
        Object.defineProperty(sample, 'name', { value: 'renamed', enumerable: true });
        let descriptor = Object.getOwnPropertyDescriptor(sample, 'name');
        let customNameVisible =
            sample.name === 'renamed' &&
            descriptor.enumerable === true &&
            Object.keys(sample)[0] === 'name';
        let deletedName = delete sample.name && !('name' in sample);
        let protectedPrototypes =
            delete sample.prototype === false &&
            delete native.prototype === false;
        let nativeMetadata =
            native.name === 'max' &&
            native.length === 2;
        customNameVisible && deletedName && protectedPrototypes && nativeMetadata
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
