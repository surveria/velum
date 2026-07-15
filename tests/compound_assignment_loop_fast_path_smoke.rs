use velum::{Engine, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn structured_loop_preserves_compound_assignment_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        let total = 0;
        let record = { count: 1 };
        let values = [1, 2, 3, 4];

        for (let index = 0; index < 64; index++) {
            total += index & 3;
            record.count += 2;
            values[index & 3] += record.count & 1;
            if ((index & 7) === 0) {
                record.count -= 1;
            }
        }

        total + record.count + values[0] + values[1] + values[2] + values[3]
        ",
    )?;
    let before = vm.resource_usage();
    let value = vm.eval_compiled(&script)?;
    let after = vm.resource_usage();

    ensure_value(&value, &Value::Number(259.0))?;
    let segment_delta = after
        .bytecode_linear_segment_runs
        .checked_sub(before.bytecode_linear_segment_runs)
        .ok_or("segment counter moved backwards")?;
    let direct_delta = after
        .bytecode_linear_direct_runs
        .checked_sub(before.bytecode_linear_direct_runs)
        .ok_or("direct counter moved backwards")?;
    ensure_at_least(segment_delta, 64, "bytecode linear segment runs")?;
    ensure_at_least(direct_delta, 64, "bytecode linear direct runs")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, min: usize, label: &str) -> TestResult {
    if actual >= min {
        return Ok(());
    }
    Err(format!("expected {label} >= {min}, got {actual}").into())
}
