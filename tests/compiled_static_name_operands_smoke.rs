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

const ESCAPED_FUNCTION_SOURCE: &str = r"
var counterFactory = function counterFactory(seed) {
    return function(delta) {
        seed = seed + delta;
        return seed;
    };
};
counterFactory;
";

const ESCAPED_FUNCTION_CALL_SOURCE: &str = r"
var firstCounter = counterFactory(10);
firstCounter(5) + firstCounter(1);
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

#[test]
fn compiled_missing_static_name_does_not_intern_atom() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile("missingCompiledBinding")?;
    let atom_count = vm.resource_usage().atom_count;

    let Err(error) = vm.eval_compiled(&script) else {
        return Err("expected missing compiled binding to fail".into());
    };
    ensure_contains(&error.to_string(), "ReferenceError")?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn escaped_compiled_function_reuses_static_name_atom_cache() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let define = vm.compile(ESCAPED_FUNCTION_SOURCE)?;
    let call = vm.compile(ESCAPED_FUNCTION_CALL_SOURCE)?;

    let value = vm.eval_compiled(&define)?;
    ensure_function(&value)?;

    let value = vm.eval_compiled(&call)?;
    ensure_value(&value, &Value::Number(31.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&call)?;
    ensure_value(&value, &Value::Number(31.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_function(value: &Value) -> TestResult {
    if matches!(value, Value::Function(_)) {
        return Ok(());
    }
    Err(format!("expected function value, got {value:?}").into())
}

fn ensure_contains(actual: &str, expected: &str) -> TestResult {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected '{actual}' to contain '{expected}'").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
