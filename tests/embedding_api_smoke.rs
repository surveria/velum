use rs_quickjs::{Engine, EngineConfig, Error, RuntimeLimits, Value, VmConfig, VmResourceUsage};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn creates_isolated_vms_with_separate_globals_and_output() -> TestResult {
    let engine = Engine::new();
    let mut front_vm = engine.create_vm();
    let mut rear_vm = engine.create_vm();

    let front_value = front_vm.context().eval(
        r#"
        let camera = "front";
        print("vm", camera);
        camera
        "#,
    )?;
    let rear_value = rear_vm.context().eval(
        r#"
        let camera = "rear";
        print("vm", camera);
        camera
        "#,
    )?;

    ensure_value(&front_value, &Value::String("front".to_owned()))?;
    ensure_value(&rear_value, &Value::String("rear".to_owned()))?;
    ensure_optional_value(
        front_vm.context().get_global("camera").as_ref(),
        &Value::String("front".to_owned()),
    )?;
    ensure_optional_value(
        rear_vm.context().get_global("camera").as_ref(),
        &Value::String("rear".to_owned()),
    )?;
    ensure_output(front_vm.context().output(), &["vm front".to_owned()])?;
    ensure_output(rear_vm.context().output(), &["vm rear".to_owned()])?;

    let front_again = front_vm.context().eval("camera")?;
    let rear_again = rear_vm.context().eval("camera")?;
    ensure_value(&front_again, &Value::String("front".to_owned()))?;
    ensure_value(&rear_again, &Value::String("rear".to_owned()))
}

#[test]
fn applies_vm_limits_without_poisoning_other_engines() -> TestResult {
    let constrained_limits = RuntimeLimits {
        max_runtime_steps: 1,
        ..RuntimeLimits::default()
    };
    let constrained_engine = Engine::with_config(EngineConfig::with_default_vm_config(
        VmConfig::with_limits(constrained_limits),
    ));
    let mut constrained_vm = constrained_engine.create_vm();

    let Err(error) = constrained_vm.context().eval("let value = 1 + 2; value") else {
        return Err("expected constrained VM to hit a runtime step limit".into());
    };
    ensure_resource_limit(&error)?;

    let default_engine = Engine::new();
    let mut default_vm = default_engine.create_vm();
    let value = default_vm.context().eval("let value = 40; value + 2")?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn reports_vm_resource_usage_at_teardown() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r#"
        let status = "ready";
        print(status);
        status
        "#,
    )?;
    ensure_value(&value, &Value::String("ready".to_owned()))?;

    let report = vm.teardown_report();
    ensure_positive(report.resources.runtime_steps, "runtime steps")?;
    ensure_usage(
        &report.resources,
        &VmResourceUsage {
            runtime_steps: report.resources.runtime_steps,
            output_entries: 1,
            global_bindings: 1,
        },
    )?;

    let finished = vm.finish();
    ensure_usage(&finished.resources, &report.resources)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_optional_value(actual: Option<&Value>, expected: &Value) -> TestResult {
    let Some(actual) = actual else {
        return Err(format!("expected global value {expected:?}, got no binding").into());
    };
    ensure_value(actual, expected)
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_resource_limit(error: &Error) -> TestResult {
    if matches!(error, Error::ResourceLimit { .. }) {
        return Ok(());
    }
    Err(format!("expected resource limit error, got {error}").into())
}

fn ensure_positive(actual: usize, label: &str) -> TestResult {
    if actual > 0 {
        return Ok(());
    }
    Err(format!("expected positive {label}, got {actual}").into())
}

fn ensure_usage(actual: &VmResourceUsage, expected: &VmResourceUsage) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected usage {expected:?}, got {actual:?}").into())
}
