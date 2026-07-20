use velum::{Engine, OptimizationMode, Value, Vm, VmConfig};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn numeric_property_read_assignments_use_linear_peepholes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        var total = 0;
        var record = { alpha: 3, beta: 5, gamma: 7 };

        for (var index = 0; index < 64; index++) {
            total = total + record.alpha;
            total = total + record.beta;
            total = total + record.gamma;
        }

        total
        ",
    )?;
    ensure_at_least(
        script.usage().bytecode_linear_peephole_candidate_count(),
        3,
        "compiled linear peephole candidates",
    )?;

    let before = vm.resource_usage();
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(960.0))?;
    let segment_delta = vm
        .resource_usage()
        .bytecode_linear_segment_runs
        .checked_sub(before.bytecode_linear_segment_runs)
        .ok_or("bytecode linear segment counter moved backwards")?;
    let direct_delta = vm
        .resource_usage()
        .bytecode_linear_direct_runs
        .checked_sub(before.bytecode_linear_direct_runs)
        .ok_or("bytecode linear direct counter moved backwards")?;
    let optimized_runs = segment_delta
        .checked_add(direct_delta)
        .ok_or("bytecode linear optimized run count overflowed")?;
    ensure_at_least(optimized_runs, 64, "bytecode linear optimized runs")
}

#[test]
fn numeric_property_read_assignments_observe_each_getter_once() -> TestResult {
    let source = r#"
        var reads = 0;
        var observed = "unset";
        var total = 0;
        var record = {
            get first() { reads++; return reads; },
            get second() { reads++; return "x"; },
            get third() { reads++; observed = total; return reads; }
        };

        total = total + record.first;
        total = total + record.second;
        total = total + record.third;
        total + ":" + reads + ":" + observed
    "#;

    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
        let value = vm.eval(source)?;
        ensure_value(&value, &Value::String("1x3:3:1x".into()))?;
    }
    Ok(())
}

#[test]
fn numeric_property_read_assignments_observe_each_proxy_trap_once() -> TestResult {
    let source = r#"
        var reads = 0;
        var total = 0;
        var record = new Proxy({ first: 1, second: "x" }, {
            get: function (target, property) {
                reads++;
                return target[property];
            }
        });

        total = total + record.first;
        total = total + record.second;
        total + ":" + reads
    "#;

    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
        let value = vm.eval(source)?;
        ensure_value(&value, &Value::String("1x:2".into()))?;
    }
    Ok(())
}

#[test]
fn numeric_property_read_assignments_follow_shape_changes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval("var total = 0; var record = { value: 1 };")?;
    let script = vm.compile("total = total + record.value; total")?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(1.0))?;

    vm.eval("record.extra = 0; record.value = 41; total = 0;")?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(41.0))?;

    vm.eval("delete record.value; record.__proto__ = { value: 42 }; total = 0;")?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} >= {minimum}, got {actual}").into())
}
