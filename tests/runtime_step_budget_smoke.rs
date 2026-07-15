use velum::{Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const STEP_SOURCE: &str = "1";

#[test]
fn starts_a_new_budget_without_discarding_lifetime_steps() -> TestResult {
    let probe_runtime = Runtime::new();
    let mut probe = probe_runtime.context();
    let value = probe.eval(STEP_SOURCE)?;
    ensure_value(&value, &Value::Number(1.0))?;
    let steps_per_eval = probe.runtime_steps();
    if steps_per_eval == 0 {
        return Err("probe evaluation did not charge runtime steps".into());
    }

    let runtime = Runtime::with_limits(RuntimeLimits {
        max_runtime_steps: steps_per_eval,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let first = context.eval(STEP_SOURCE)?;
    ensure_value(&first, &Value::Number(1.0))?;
    let first_total = context.runtime_steps();

    context.begin_runtime_step_budget();
    let second = context.eval(STEP_SOURCE)?;
    ensure_value(&second, &Value::Number(1.0))?;
    let expected_total = first_total
        .checked_add(steps_per_eval)
        .ok_or("expected runtime step total overflowed")?;
    ensure_usize(context.runtime_steps(), expected_total)
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
