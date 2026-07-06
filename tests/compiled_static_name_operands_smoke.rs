use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const STATIC_NAME_SCRIPT: &str = r"
var Box = function Box(value) {
    this.value = value;
};
var box = new Box(4);
var record = {
    alpha: 1,
    beta: 2,
    method() {
        return this.alpha + this.beta;
    },
};
record.gamma = 3;
var total = record.method() + record.gamma;
for (var key in record) {
    total += 1;
}
try {
    throw total + box.value;
} catch (caught) {
    caught;
}
";

#[test]
fn compiled_static_names_preserve_binding_and_property_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(STATIC_NAME_SCRIPT)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(14.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(14.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
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
    Err(format!("expected {expected}, got {actual}").into())
}
