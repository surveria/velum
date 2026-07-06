use rs_quickjs::{Engine, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const BYTECODE_LOOP_SOURCE: &str = r"
var camera0 = 1;
var camera1 = 2;
var camera2 = 3;
var camera3 = 4;
var total = 0;
var index = 0;
while (index < 32) {
    var slot = index & 3;
    total = total + camera0 + camera1 + camera2 + camera3 + slot;
    index = index + 1;
}
total;
";

const BYTECODE_PROPERTY_SOURCE: &str = r"
var values = [1, 2, 3, 4];
var holder = { offset: 10 };
holder.total = values[0] + values[1] + holder.offset;
holder.total;
";

const BYTECODE_HOIST_SOURCE: &str = r"
if (false) {
    var hidden = 41;
}
hidden;
";

const BYTECODE_STRUCTURED_SOURCE: &str = r"
var obj = { a: 1, b: 2 };
var seen = 0;
for (var key in obj) {
    seen += obj[key];
}
switch (seen) {
    case 3:
        seen = seen + 1;
        break;
    default:
        seen = 0;
}
try {
    if (seen === 4) {
        throw new Test262Error('boom');
    }
} catch (error) {
    seen = seen + 1;
} finally {
    seen = seen + 1;
}
var plus = function(value) {
    return value + 1;
};
Math.max(seen, plus(5));
";

const BYTECODE_FUNCTION_HOIST_SOURCE: &str = r"
var read = function() {
    if (false) {
        var hidden = 9;
    }
    return hidden;
};
read();
";

const BYTECODE_CLOSURE_SOURCE: &str = r"
var make = function(base) {
    var offset = 2;
    return function(value) {
        return base + offset + value;
    };
};
var add = make(10);
add(5);
";

const BYTECODE_DIRECT_BINDING_OPERANDS_SOURCE: &str = r"
var Box = function Box(value) {
    this.value = value;
};
var make = function make(seed) {
    let total = seed;
    return function add(delta) {
        total = total + delta;
        total += 1;
        total++;
        return total;
    };
};
var add = make(3);
var made = new Box(add(4));
made.value + add(1);
";

const BYTECODE_PROPERTY_NATIVE_NUMERIC_SOURCE: &str = r"
var values = Array(1, 2, 3);
values.push(4);
var obj = { value: 6 };
var fake = {
    max: function(left, right) {
        return left - right;
    }
};
var number = obj.value * 7;
Math.max(number, Math.abs(-3)) + fake.max(9, 4) + values.length;
";

#[test]
fn compiled_script_exposes_bytecode_instruction_count() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile("let value = 1 + 2 + 3; value")?;

    ensure_positive(
        script.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(6.0))
}

#[test]
fn bytecode_executes_repeated_binding_loop_without_atom_growth() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_LOOP_SOURCE)?;
    ensure_positive(
        script.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(368.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(368.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_executes_property_array_and_object_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_PROPERTY_SOURCE)?;
    ensure_positive(
        script.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(13.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(13.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_hoist_plan_replaces_top_level_ast_hoist() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_HOIST_SOURCE)?;

    ensure_usize(script.usage().bytecode_hoisted_var_count(), 1)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Undefined)
}

#[test]
fn bytecode_executes_structured_control_flow_and_function_calls() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let structured = vm.compile(BYTECODE_STRUCTURED_SOURCE)?;

    ensure_positive(
        structured.usage().bytecode_instruction_count(),
        "bytecode instructions",
    )?;

    let value = vm.eval_compiled(&structured)?;
    ensure_value(&value, &Value::Number(6.0))
}

#[test]
fn bytecode_functions_use_function_local_hoist_plan() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_FUNCTION_HOIST_SOURCE)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Undefined)
}

#[test]
fn bytecode_functions_capture_closure_upvalues_without_ast_body() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_CLOSURE_SOURCE)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(17.0))
}

#[test]
fn bytecode_carries_direct_binding_operands_for_hot_binding_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_DIRECT_BINDING_OPERANDS_SOURCE)?;
    let usage = script.usage();

    ensure_usize(usage.unresolved_static_binding_count(), 0)?;
    ensure_positive(
        usage.bytecode_binding_operand_count(),
        "bytecode binding operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(21.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(21.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_carries_property_native_and_numeric_operands() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(BYTECODE_PROPERTY_NATIVE_NUMERIC_SOURCE)?;
    let usage = script.usage();

    ensure_positive(
        usage.bytecode_property_operand_count(),
        "bytecode property operands",
    )?;
    ensure_positive(
        usage.bytecode_direct_native_call_count(),
        "bytecode direct native calls",
    )?;
    ensure_positive(
        usage.bytecode_array_native_call_count(),
        "bytecode array native calls",
    )?;
    ensure_positive(
        usage.bytecode_numeric_instruction_count(),
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(51.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(51.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual != expected {
        return Err(format!("expected {expected:?}, got {actual:?}").into());
    }
    Ok(())
}

fn ensure_positive(value: usize, label: &str) -> TestResult {
    if value == 0 {
        return Err(format!("expected positive {label}, got {value}").into());
    }
    Ok(())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual != expected {
        return Err(format!("expected {expected}, got {actual}").into());
    }
    Ok(())
}
