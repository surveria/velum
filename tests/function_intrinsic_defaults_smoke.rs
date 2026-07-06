use rs_quickjs::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn intrinsic_function_defaults_are_reused_for_descriptors() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r"
        let localName = function localName(left, right, extra) { return left + right + extra; };
        let native = Math.max;
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;

    let value = vm.context().eval(
        r"
        let localNameDescriptor = Object.getOwnPropertyDescriptor(localName, 'name');
        let localLengthDescriptor = Object.getOwnPropertyDescriptor(localName, 'length');
        let nativeNameDescriptor = Object.getOwnPropertyDescriptor(native, 'name');
        let nativeLengthDescriptor = Object.getOwnPropertyDescriptor(native, 'length');
        localNameDescriptor.value === 'localName' &&
            localNameDescriptor.configurable === true &&
            localNameDescriptor.enumerable === false &&
            localNameDescriptor.writable === false &&
            localLengthDescriptor.value === 3 &&
            localLengthDescriptor.configurable === true &&
            localLengthDescriptor.enumerable === false &&
            localLengthDescriptor.writable === false &&
            nativeNameDescriptor.value === 'max' &&
            nativeNameDescriptor.configurable === true &&
            nativeNameDescriptor.enumerable === false &&
            nativeNameDescriptor.writable === false &&
            nativeLengthDescriptor.value === 2 &&
            nativeLengthDescriptor.configurable === true &&
            nativeLengthDescriptor.enumerable === false &&
            nativeLengthDescriptor.writable === false
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))?;
    let first_descriptor_atoms = vm.resource_usage().atom_count;

    let value = vm.context().eval(
        r"
        Object.getOwnPropertyDescriptor(localName, 'name').value;
        Object.getOwnPropertyDescriptor(localName, 'length').value;
        Object.getOwnPropertyDescriptor(native, 'name').value;
        Object.getOwnPropertyDescriptor(native, 'length').value;
        0
        ",
    )?;
    ensure_value(&value, &Value::Number(0.0))?;
    ensure_usize(vm.resource_usage().atom_count, first_descriptor_atoms)
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
