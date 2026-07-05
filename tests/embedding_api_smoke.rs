use rs_quickjs::{
    Engine, EngineConfig, Error, RuntimeLimits, Value, Vm, VmConfig, VmResourceUsage,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const ISOLATED_VM_LABELS: [&str; 8] = [
    "front", "rear", "side", "gate", "lobby", "roof", "garage", "hall",
];
const COMPILED_COUNTER_SOURCE: &str = "counter = counter + 1; counter";
const COMPILED_LABEL_SOURCE: &str = r#"print("compiled", label); label"#;

struct VmCase {
    label: &'static str,
    vm: Vm,
}

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
fn keeps_many_vms_isolated_after_one_vm_fails() -> TestResult {
    let engine = Engine::new();
    let mut cases = Vec::with_capacity(ISOLATED_VM_LABELS.len());

    for label in ISOLATED_VM_LABELS {
        let mut vm = engine.create_vm();
        let source = format!(
            r#"
            let camera = "{label}";
            print("ready", camera);
            camera
            "#
        );
        let value = vm.context().eval(&source)?;
        ensure_value(&value, &Value::String(label.to_owned()))?;
        cases.push(VmCase { label, vm });
    }

    let constrained_limits = RuntimeLimits {
        max_runtime_steps: 1,
        ..RuntimeLimits::default()
    };
    let mut failing_vm = Vm::with_config(VmConfig::with_limits(constrained_limits));
    let Err(error) = failing_vm.context().eval("let value = 1 + 2; value") else {
        return Err("expected isolated failing VM to hit a runtime step limit".into());
    };
    ensure_resource_limit(&error)?;

    for case in &mut cases {
        let expected_value = Value::String(case.label.to_owned());
        let expected_output = [format!("ready {}", case.label)];
        ensure_optional_value(
            case.vm.context().get_global("camera").as_ref(),
            &expected_value,
        )?;
        ensure_output(case.vm.context().output(), &expected_output)?;

        let value = case.vm.context().eval("camera")?;
        ensure_value(&value, &expected_value)?;
    }

    for case in cases {
        let report = case.vm.finish();
        ensure_positive(report.resources.runtime_steps, "runtime steps")?;
        ensure_usage(
            &report.resources,
            &VmResourceUsage {
                runtime_steps: report.resources.runtime_steps,
                output_entries: 1,
                global_bindings: 1,
                atom_count: report.resources.atom_count,
            },
        )?;
    }

    Ok(())
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
            atom_count: report.resources.atom_count,
        },
    )?;

    let finished = vm.finish();
    ensure_usage(&finished.resources, &report.resources)
}

#[test]
fn tracks_atoms_for_bindings_without_interning_missing_names() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let initial_atoms = vm.resource_usage().atom_count;
    let Err(error) = vm.context().eval("missingBinding") else {
        return Err("expected missing binding lookup to fail".into());
    };
    ensure_runtime_error(&error)?;
    ensure_usize(vm.resource_usage().atom_count, initial_atoms)?;

    let value = vm
        .context()
        .eval("let camera = 41; camera = camera + 1; camera")?;
    ensure_value(&value, &Value::Number(42.0))?;
    let declared_atoms = vm.resource_usage().atom_count;
    ensure_positive(declared_atoms, "atom count after declaration")?;

    let value = vm.context().eval("camera")?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, declared_atoms)
}

#[test]
fn tracks_atoms_for_object_property_keys_without_interning_missing_properties() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let initial_atoms = vm.resource_usage().atom_count;
    let value = vm.context().eval("let bag = { alpha: 1 }; bag.alpha")?;
    ensure_value(&value, &Value::Number(1.0))?;
    let object_atoms = vm.resource_usage().atom_count;
    ensure_greater_than(object_atoms, initial_atoms, "object property atoms")?;

    let value = vm.context().eval("bag.missing")?;
    ensure_value(&value, &Value::Undefined)?;
    ensure_usize(vm.resource_usage().atom_count, object_atoms)?;

    let value = vm.context().eval(
        r#"
        bag.beta = 2;
        bag["gamma"] = 3;
        let keys = "";
        for (let key in bag) {
            keys = keys + key + ":";
        }
        keys + (bag.beta + bag.gamma)
        "#,
    )?;
    ensure_value(&value, &Value::String("alpha:beta:gamma:5".to_owned()))?;
    ensure_greater_than(
        vm.resource_usage().atom_count,
        object_atoms,
        "mutated object property atoms",
    )
}

#[test]
fn preserves_binding_slot_updates_and_shadowing() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r"
        let camera = 1;
        {
            let camera = 10;
            camera = camera + 5;
        }
        camera = camera + 1;
        camera
        ",
    )?;

    ensure_value(&value, &Value::Number(2.0))?;
    ensure_optional_value(
        vm.context().get_global("camera").as_ref(),
        &Value::Number(2.0),
    )
}

#[test]
fn exposes_vm_level_embedding_helpers() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.register_host_function_typed("cameraLabel", |call| {
        let name: &str = call.argument(0, "name")?;
        Ok(format!("camera:{name}"))
    })?;
    vm.register_host_function("legacyAdd", |call| {
        let left = call.number(0, "left")?;
        let right = call.number(1, "right")?;
        Ok(Value::Number(left + right))
    })?;

    let camera = vm.eval(
        r#"
        let camera = cameraLabel("front");
        print(camera);
        camera
        "#,
    )?;
    ensure_value(&camera, &Value::String("camera:front".to_owned()))?;
    ensure_optional_value(
        vm.get_global("camera").as_ref(),
        &Value::String("camera:front".to_owned()),
    )?;

    let script = vm.compile("legacyAdd(20, 22)")?;
    let sum = vm.eval_compiled(&script)?;
    ensure_value(&sum, &Value::Number(42.0))?;
    ensure_output(vm.output(), &["camera:front".to_owned()])?;

    let output = vm.take_output();
    ensure_output(&output, &["camera:front".to_owned()])?;
    ensure_output(vm.output(), &[])
}

#[test]
fn evaluates_compiled_script_repeatedly_in_one_vm() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().eval("var counter = 0;")?;

    let script = vm.compile(COMPILED_COUNTER_SOURCE)?;
    ensure_usize(script.usage().source_len(), COMPILED_COUNTER_SOURCE.len())?;
    ensure_usize(script.usage().top_level_statement_count(), 2)?;
    ensure_positive(
        script.usage().max_expression_depth(),
        "compiled expression depth",
    )?;

    let first = vm.eval_compiled(&script)?;
    let second = vm.eval_compiled(&script)?;

    ensure_value(&first, &Value::Number(1.0))?;
    ensure_value(&second, &Value::Number(2.0))
}

#[test]
fn evaluates_one_compiled_script_in_isolated_vms() -> TestResult {
    let engine = Engine::new();
    let compile_vm = engine.create_vm();
    let script = compile_vm.compile(COMPILED_LABEL_SOURCE)?;
    let mut front_vm = engine.create_vm();
    let mut rear_vm = engine.create_vm();

    front_vm.context().eval(r#"let label = "front";"#)?;
    rear_vm.context().eval(r#"let label = "rear";"#)?;

    let front = front_vm.eval_compiled(&script)?;
    let rear = rear_vm.eval_compiled(&script)?;

    ensure_value(&front, &Value::String("front".to_owned()))?;
    ensure_value(&rear, &Value::String("rear".to_owned()))?;
    ensure_output(front_vm.context().output(), &["compiled front".to_owned()])?;
    ensure_output(rear_vm.context().output(), &["compiled rear".to_owned()])
}

#[test]
fn reports_compile_errors_before_evaluation() -> TestResult {
    let engine = Engine::new();
    let vm = engine.create_vm();

    let Err(error) = vm.compile("let value = ;") else {
        return Err("expected compiled script parsing to fail".into());
    };
    ensure_parse_error(&error)
}

#[test]
fn rejects_compiled_script_that_exceeds_target_vm_limits() -> TestResult {
    let runtime = rs_quickjs::Runtime::new();
    let statement_script = runtime.compile("1; 2;")?;
    ensure_compiled_script_rejected_by_limits(
        &statement_script,
        RuntimeLimits {
            max_statements: 1,
            ..RuntimeLimits::default()
        },
        "statement limit",
    )?;

    let source_script = runtime.compile("123;")?;
    ensure_compiled_script_rejected_by_limits(
        &source_script,
        RuntimeLimits {
            max_source_len: 1,
            ..RuntimeLimits::default()
        },
        "source length limit",
    )?;

    let expression_script = runtime.compile("((1));")?;
    ensure_compiled_script_rejected_by_limits(
        &expression_script,
        RuntimeLimits {
            max_expression_depth: 1,
            ..RuntimeLimits::default()
        },
        "expression depth limit",
    )
}

fn ensure_compiled_script_rejected_by_limits(
    script: &rs_quickjs::CompiledScript,
    limits: RuntimeLimits,
    label: &str,
) -> TestResult {
    let mut vm = Vm::with_config(VmConfig::with_limits(limits));

    let Err(error) = vm.eval_compiled(script) else {
        return Err(format!("expected compiled script to exceed target VM {label}").into());
    };
    ensure_resource_limit(&error)
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

fn ensure_parse_error(error: &Error) -> TestResult {
    if matches!(error, Error::Parse { .. }) {
        return Ok(());
    }
    Err(format!("expected parse error, got {error}").into())
}

fn ensure_runtime_error(error: &Error) -> TestResult {
    if matches!(error, Error::Runtime { .. }) {
        return Ok(());
    }
    Err(format!("expected runtime error, got {error}").into())
}

fn ensure_positive(actual: usize, label: &str) -> TestResult {
    if actual > 0 {
        return Ok(());
    }
    Err(format!("expected positive {label}, got {actual}").into())
}

fn ensure_greater_than(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual > minimum {
        return Ok(());
    }
    Err(format!("expected {label} greater than {minimum}, got {actual}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_usage(actual: &VmResourceUsage, expected: &VmResourceUsage) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected usage {expected:?}, got {actual:?}").into())
}
