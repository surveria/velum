use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const CLOSURE_LAYOUT_SOURCE: &str = r"
var makeCounter = function makeCounter(seed) {
    let total = seed;
    return function(step) {
        total = total + step;
        return total;
    };
};
var counter = makeCounter(10);
counter(2) + counter(3);
";

const SHADOWED_LAYOUT_SOURCE: &str = r"
var value = 1;
{
    let value = 2;
    value;
}
value;
";

const UNRESOLVED_LAYOUT_SOURCE: &str = "missingValue + 1;";

const FOR_IN_LAYOUT_SOURCE: &str = r"
var record = { alpha: 1, beta: 2 };
var total = 0;
for (let key in record) {
    total = total + record[key];
}
total;
";

const PARAM_FRAME_LAYOUT_SOURCE: &str = r"
var run = function run(zeta, alpha, middle) {
    let total = zeta + alpha * 10 + middle * 100;
    {
        let alpha = 7;
        total = total + alpha;
    }
    return total + zeta + middle;
};
run(1, 2, 3) + run(4, 5, 6);
";

#[test]
fn compiled_layout_counts_global_local_and_upvalue_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(CLOSURE_LAYOUT_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 2)?;
    ensure_usize(usage.local_binding_slot_count(), 3)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 1)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;
    ensure_usize(
        usage.resolved_static_binding_count(),
        usage.static_binding_count(),
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(27.0))
}

#[test]
fn compiled_layout_separates_shadowed_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(SHADOWED_LAYOUT_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 1)?;
    ensure_usize(usage.local_binding_slot_count(), 1)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1.0))
}

#[test]
fn compiled_layout_keeps_missing_bindings_unresolved() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile(UNRESOLVED_LAYOUT_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 0)?;
    ensure_usize(usage.local_binding_slot_count(), 0)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 1)?;
    ensure_usize(usage.resolved_static_binding_count(), 0)
}

#[test]
fn compiled_layout_cache_preserves_for_in_lexical_bindings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(FOR_IN_LAYOUT_SOURCE)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(3.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(3.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_layout_drives_function_parameter_frame_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(PARAM_FRAME_LAYOUT_SOURCE)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1003.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1003.0))?;
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
