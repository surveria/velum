use rs_quickjs::{Engine, Error, Runtime, RuntimeLimits, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bytecode_quickens_numeric_add_and_array_length_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var values = [1, 2, 3];
        var plain = { length: 9 };
        var numeric = values.length + 4 + 5;
        var text = "len=" + values.length;
        var fallbackTotal = plain.length + "go".length + Math.max.length;
        numeric === 12 &&
            text === "len=3" &&
            fallbackTotal === 13 ? 42 : 0
        "#,
    )?;
    let usage = script.usage();

    ensure_at_least(
        usage.bytecode_numeric_instruction_count(),
        5,
        "bytecode numeric instructions",
    )?;
    ensure_at_least(
        usage.bytecode_property_operand_count(),
        5,
        "bytecode property operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_numeric_comparisons_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;
        for (var i = 0; i < 8; i = i + 1) {
            if (i >= 2 && i <= 5 && i > 3) {
                total = total + i;
            }
        }
        total === 9 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        6,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let value = vm.eval("\"a\" < \"b\"")?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn bytecode_quickens_numeric_equalities_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var total = 0;
        for (var i = 0; i < 8; i = i + 1) {
            if (i == 2) {
                total = total + 3;
            }
            if (i === 4) {
                total = total + 5;
            }
            if (i != 5) {
                total = total + 1;
            }
            if (i !== 7) {
                total = total + 1;
            }
        }
        var textFallback = "same" === "same" && "same" !== "other";
        var nanFallback = NaN !== NaN;
        total === 22 && textFallback && nanFallback ? 42 : 0
        "#,
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        12,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn bytecode_quickens_numeric_unary_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;
        for (var i = 0; i < 6; i = i + 1) {
            total = total + +i + -(-i);
        }
        total === 30 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        7,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let Err(error) = vm.eval("-\"x\"") else {
        return Err("expected non-number unary to use generic fallback error".into());
    };
    ensure_error_contains(&error, "unary '-' expects a number")
}

#[test]
fn bytecode_quickens_numeric_bitwise_and_shifts_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var direct = (5 & 3) + (5 | 2) + (5 ^ 1) + (1 << 4) + (-8 >> 1) + (-1 >>> 1);
        var value = 1;
        value |= 6;
        value ^= 3;
        value <<= 2;
        value >>= 1;
        value >>>= 1;
        direct === 2147483671 && value === 4 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        12,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let value = vm.eval("\"5\" & 3")?;
    ensure_value(&value, &Value::Number(1.0))
}

#[test]
fn bytecode_quickens_static_array_index_reads_and_writes_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var values = [10, 20];
        var first = values[0];
        values[1] = first + 2;
        var missing = values[3];
        values[3] = 99;

        var plain = {};
        plain[0] = 5;
        plain[0] = plain[0] + 1;

        var text = "go";
        first === 10 &&
            values[1] === 12 &&
            missing === undefined &&
            values.length === 4 &&
            values[3] === 99 &&
            plain[0] === 6 &&
            text[0] === "g" ? 42 : 0
        "#,
    )?;
    ensure_at_least(
        script.usage().bytecode_property_operand_count(),
        8,
        "bytecode property operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_dynamic_array_index_reads_and_writes_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var values = [4, 5, 6];
        var index = 1;
        var first = values[index];
        values[index + 1] = first + 10;

        var key = "0";
        var fromStringKey = values[key];

        var far = 100000;
        values[far] = 7;

        var plain = {};
        plain[index] = 8;

        var text = "hi";
        var zero = 0;

        first === 5 &&
            values[2] === 15 &&
            fromStringKey === 4 &&
            values[far] === 7 &&
            plain[1] === 8 &&
            text[zero] === "h" ? 42 : 0
        "#,
    )?;
    ensure_at_least(
        script.usage().bytecode_property_operand_count(),
        9,
        "bytecode property operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_array_index_read_modify_write_with_fallbacks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        Array.prototype[4] = 20;

        var values = [1, 2, 3];
        var staticPost = values[0]++;
        var staticPre = ++values[1];
        values[2] += 10;

        var dynamicIndex = 1;
        values[dynamicIndex] += 5;
        var dynamicPost = values[dynamicIndex]++;

        var far = 100000;
        values[far] = 7;
        values[far] += 2;

        var inherited = [0];
        inherited[4] += 2;

        var plain = {};
        plain[0] = 1;
        plain[0]++;
        plain[0] += 2;

        var ok = staticPost === 1 &&
            staticPre === 3 &&
            dynamicPost === 8 &&
            values[0] === 2 &&
            values[1] === 9 &&
            values[2] === 13 &&
            values[far] === 9 &&
            inherited[4] === 22 &&
            inherited.length === 5 &&
            plain[0] === 4;

        delete Array.prototype[4];
        ok ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_property_operand_count(),
        12,
        "bytecode property operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_linear_numeric_loop_blocks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var values = [1, 2, 3, 4];
        var index = 0;
        var total = 0;

        while (index < 8) {
            var slot = index & 3;
            total = total + values[slot];
            index = index + 1;
        }

        total === 20 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        4,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_numeric_binding_chains() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;

        for (var index = 0; index < 16; index = index + 1) {
            total = total + 1 + 2 + 3 + 4 + 5 + 6;
            total = total + 7 + 8 + 9 + 10 + 11 + 12;
            total = total + (index & 3);
        }

        total === 1272 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        16,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_runs_linear_segments_inside_mixed_loop_blocks() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;

        for (var index = 0; index < 16; index = index + 1) {
            if ((index & 1) === 0) {
                total = total + 1 + 2 + 3;
            } else {
                total = total + 4 + 5;
            }
            total += index & 3;
        }

        total === 144 ? 42 : 0
        ",
    )?;

    let initial_segments = vm.resource_usage().bytecode_linear_segment_runs;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let segment_delta = vm
        .resource_usage()
        .bytecode_linear_segment_runs
        .checked_sub(initial_segments)
        .ok_or("bytecode linear segment counter moved backwards")?;
    ensure_at_least(segment_delta, 16, "bytecode linear segment runs")?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_loop_branch_conditions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var values = [1, 2, 3, 4];
        var total = 0;

        for (var index = 0; index < 32; index = index + 1) {
            if ((index & 3) === 0) {
                continue;
            }
            if (index > 28) {
                break;
            }
            total = total + values[index & 3];
        }

        total === 63 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        6,
        "bytecode numeric instructions",
    )?;

    let initial_segments = vm.resource_usage().bytecode_linear_segment_runs;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let segment_delta = vm
        .resource_usage()
        .bytecode_linear_segment_runs
        .checked_sub(initial_segments)
        .ok_or("bytecode linear segment counter moved backwards")?;
    ensure_at_least(segment_delta, 32, "bytecode linear segment runs")
}

#[test]
fn bytecode_runs_direct_linear_loop_condition_and_update() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;

        for (var index = 0; index < 32; index = index + 1) {
            total = total + index;
        }

        total === 496 ? 42 : 0
        ",
    )?;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let direct_run_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_at_least(direct_run_delta, 60, "bytecode linear direct runs")
}

#[test]
fn bytecode_function_fast_paths_fall_back_for_generic_add() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var join = function(left, right) {
            return left + right;
        };

        join("rs", "qjs") === "rsqjs" ? 42 : 0
        "#,
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        1,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn bytecode_function_fast_paths_preserve_runtime_step_limits() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_runtime_steps: 24,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let error = context
        .eval(
            r"
            var next = function() {
                return 1;
            };

            next();
            next();
            next();
            next();
            next();
            next();
            next();
            next();
            next();
            next();
            ",
        )
        .err()
        .ok_or("expected runtime step limit to fail")?;
    ensure_resource_limit(&error)
}

#[test]
fn bytecode_quickens_numeric_compound_binding_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;
        var value = 1;

        for (var index = 0; index < 16; index = index + 1) {
            total += index & 3;
            value |= index;
            value ^= index & 7;
            value <<= 1;
            value >>>= 1;
        }

        total === 24 && value === 8 ? 42 : 0
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        6,
        "bytecode numeric instructions",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    let atoms = vm.resource_usage().atom_count;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, atoms)
}

#[test]
fn bytecode_quickens_property_update_and_compound_paths() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;
        var record = { count: 1, mask: 3 };
        var values = [1, 2, 3, 4];

        for (var index = 0; index < 16; index++) {
            total++;
            record.count += 2;
            record.mask ^= index;
            record.mask |= values[index & 3];
            ++values[index & 3];
            values[index & 3] <<= 1;
            values[index & 3] >>= 1;
            values[index & 3] += record.count & 1;
        }

        total + record.count + record.mask + values[0] + values[1] + values[2] + values[3]
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_numeric_instruction_count(),
        8,
        "bytecode numeric instructions",
    )?;

    let initial_segments = vm.resource_usage().bytecode_linear_segment_runs;
    let initial_direct_runs = vm.resource_usage().bytecode_linear_direct_runs;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(101.0))?;
    let usage = vm.resource_usage();
    let segment_delta = usage
        .bytecode_linear_segment_runs
        .checked_sub(initial_segments)
        .ok_or("bytecode linear segment counter moved backwards")?;
    let direct_delta = usage
        .bytecode_linear_direct_runs
        .checked_sub(initial_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    ensure_at_least(segment_delta, 16, "bytecode linear segment runs")?;
    ensure_at_least(direct_delta, 32, "bytecode linear direct runs")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(value: usize, minimum: usize, label: &str) -> TestResult {
    if value >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} >= {minimum}, got {value}").into())
}

fn ensure_error_contains(error: &rs_quickjs::Error, text: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(text) {
        return Ok(());
    }
    Err(format!("expected error containing '{text}', got '{message}'").into())
}

fn ensure_resource_limit(error: &Error) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) {
        return Ok(());
    }
    Err(format!("expected resource limit error, got {error}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
