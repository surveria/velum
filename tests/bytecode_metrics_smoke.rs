use std::{error::Error as StdError, io};

use rs_quickjs::Engine;

type TestResult = Result<(), Box<dyn StdError>>;

#[test]
fn compiled_usage_counts_do_while_child_metrics() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile("do {} while (Array.isArray([]) && 1 + 2);")?;
    let usage = script.usage();

    ensure_usize(usage.bytecode_property_operand_count(), 1, "property")?;
    ensure_usize(
        usage.bytecode_direct_native_call_count(),
        1,
        "direct native call",
    )?;
    ensure_usize(
        usage.bytecode_array_native_call_count(),
        1,
        "array native call",
    )?;
    ensure_usize(
        usage.bytecode_numeric_instruction_count(),
        1,
        "numeric instruction",
    )
}

#[test]
fn compiled_usage_counts_with_and_pattern_children() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();

    let with_script = vm.compile("with ({}) { 1 + 2; }")?;
    ensure_usize(
        with_script.usage().bytecode_numeric_instruction_count(),
        1,
        "with numeric instruction",
    )?;

    let pattern_script = vm.compile("let [value = 1 + 2] = [];")?;
    ensure_usize(
        pattern_script.usage().bytecode_numeric_instruction_count(),
        1,
        "pattern numeric instruction",
    )
}

#[test]
fn compiled_usage_counts_function_defaults_and_hoisted_declarations() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile(
        "function declared(value = 1 + 2) { return value; }\n\
         let expression = function (value = 3 + 4) { return value; };",
    )?;

    ensure_usize(
        script.usage().bytecode_numeric_instruction_count(),
        2,
        "function default numeric instructions",
    )
}

#[test]
fn compiled_usage_counts_every_computed_super_child() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();
    let script = vm.compile(
        "class Base {}\n\
         class Derived extends Base {\n\
             method() {\n\
                 super[1 + 2];\n\
                 super[3 + 4] = 5 + 6;\n\
                 super[7 + 8] += 9 + 10;\n\
                 super[11 + 12]++;\n\
             }\n\
         }",
    )?;

    ensure_usize(
        script.usage().bytecode_numeric_instruction_count(),
        6,
        "computed super numeric instructions",
    )
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{label} mismatch: expected {expected}, got {actual}"
        ))
        .into())
    }
}
