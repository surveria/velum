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

const CATCH_LAYOUT_SOURCE: &str = r"
var value = 0;
try {
    throw 4;
} catch (error) {
    let offset = 3;
    value = error + offset;
}
value;
";

const HOISTED_VAR_LAYOUT_SOURCE: &str = r"
var run = function run(alpha) {
    var total;
    {
        var beta = alpha + 2;
        let shadow = beta + 1;
        total = shadow + alpha;
    }
    return total;
};
run(3) + run(4);
";

const GLOBAL_FRAME_AFTER_BUILTINS_SOURCE: &str = r#"
var zeta = Number("1");
var alpha = 2;
var middle = 3;
zeta = zeta + alpha + middle;
zeta;
"#;

const GLOBAL_SLOT_OPERAND_OPERATIONS_SOURCE: &str = r"
var Box = function Box(value) {
    this.value = value;
};
var total = 1;
total += 2;
total++;
var made = new Box(total);
made.value;
";

const LOCAL_FRAME_METADATA_SOURCE: &str = r"
var run = function run(seed) {
    let total = seed;
    {
        let delta = 2;
        total = total + delta;
    }
    {
        let delta = 5;
        total = total + delta;
    }
    return total;
};
run(3) + run(4);
";

const UPVALUE_FRAME_CELLS_SOURCE: &str = r"
var makeCounter = function makeCounter(seed) {
    var total = seed;
    return function add(delta) {
        total = total + delta;
        return total;
    };
};
var left = makeCounter(10);
var right = makeCounter(100);
left(1) + left(2) + right(3) + left(4);
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
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 1)?;
    ensure_usize(usage.local_binding_slot_count(), 5)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1003.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1003.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_layout_drives_catch_and_body_lexical_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(CATCH_LAYOUT_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 1)?;
    ensure_usize(usage.local_binding_slot_count(), 2)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(7.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(7.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_layout_drives_hoisted_var_frame_slots() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(HOISTED_VAR_LAYOUT_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 1)?;
    ensure_usize(usage.local_binding_slot_count(), 4)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(20.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(20.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_global_slots_are_separate_from_builtins() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("Number; Array; Object")?;
    let builtin_bindings = vm.resource_usage().global_bindings;
    ensure_greater_than(builtin_bindings, 0, "builtin global bindings")?;

    let script = vm.compile(GLOBAL_FRAME_AFTER_BUILTINS_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 3)?;
    ensure_usize(usage.local_binding_slot_count(), 0)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 1)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(6.0))?;
    ensure_optional_value(vm.get_global("zeta").as_ref(), &Value::Number(6.0))?;
    ensure_optional_value(vm.get_global("alpha").as_ref(), &Value::Number(2.0))?;
    ensure_optional_value(vm.get_global("middle").as_ref(), &Value::Number(3.0))?;
    ensure_usize(
        vm.resource_usage().global_bindings,
        builtin_bindings.saturating_add(3),
    )
}

#[test]
fn compiled_global_slot_operands_drive_binding_operations() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(GLOBAL_SLOT_OPERAND_OPERATIONS_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 3)?;
    ensure_usize(usage.local_binding_slot_count(), 1)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(4.0))?;
    ensure_optional_value(vm.get_global("total").as_ref(), &Value::Number(4.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(4.0))?;
    ensure_optional_value(vm.get_global("total").as_ref(), &Value::Number(4.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_local_frame_metadata_separates_same_slot_blocks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(LOCAL_FRAME_METADATA_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 1)?;
    ensure_usize(usage.local_binding_slot_count(), 4)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 0)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(21.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(21.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

#[test]
fn compiled_upvalue_frame_cells_preserve_closure_instances() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(UPVALUE_FRAME_CELLS_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.global_binding_slot_count(), 3)?;
    ensure_usize(usage.local_binding_slot_count(), 3)?;
    ensure_usize(usage.upvalue_binding_slot_count(), 1)?;
    ensure_usize(usage.unresolved_static_binding_count(), 0)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(144.0))?;
    let atom_count = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(144.0))?;
    ensure_usize(vm.resource_usage().atom_count, atom_count)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_optional_value(actual: Option<&Value>, expected: &Value) -> TestResult {
    let Some(actual) = actual else {
        return Err(format!("expected value {expected:?}, got no binding").into());
    };
    ensure_value(actual, expected)
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_greater_than(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual > minimum {
        return Ok(());
    }
    Err(format!("expected {label} above {minimum}, got {actual}").into())
}
