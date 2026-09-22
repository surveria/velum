use velum::{
    Error, OptimizationMode, Runtime, RuntimeLimits, SourceId, SourceSpan, Value, Vm, VmConfig,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const MODES: [OptimizationMode; 2] = [OptimizationMode::Enabled, OptimizationMode::Disabled];
const REPEATED_LOOP_SOURCE: &str = r"
    function accumulate(seed) {
        var total = seed;
        for (var index = 0; index < 12; index = index + 1) {
            if (index === 5) {
                continue;
            }
            total = total + index;
        }
        return total;
    }
    accumulate(input)
";

#[test]
fn linear_plans_keep_bindings_local_across_compiled_runs_and_vms() -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile(REPEATED_LOOP_SOURCE)?;

    for mode in MODES {
        for seed in [10_u32, 100_u32] {
            let mut vm = vm_with_mode(mode);
            ensure_value(
                &vm.eval(&format!("var input = {seed}; input"))?,
                &Value::Number(f64::from(seed)),
            )?;
            for repetition in 0_u32..3 {
                let input = f64::from(seed) + f64::from(repetition);
                ensure_value(&vm.eval_compiled(&script)?, &Value::Number(input + 61.0))?;
                ensure_value(&vm.eval("input = input + 1;")?, &Value::Number(input + 1.0))?;
            }
            ensure_direct_path_mode(&vm, mode)?;
        }
    }
    Ok(())
}

#[test]
fn nested_linear_loops_preserve_control_flow_and_fresh_locals() -> TestResult {
    let source = r"
        function nested(seed) {
            var total = seed;
            for (var outer = 0; outer < 4; outer = outer + 1) {
                for (var inner = 0; inner < 5; inner = inner + 1) {
                    if (inner === 1) {
                        continue;
                    }
                    if (inner === 4) {
                        break;
                    }
                    {
                        let delta = outer * 10 + inner;
                        total = total + delta;
                    }
                }
            }
            return total;
        }
        nested(3) + nested(10)
    ";
    ensure_modes_value(source, &Value::Number(413.0))
}

#[test]
fn lexical_loop_updates_preserve_escaped_iteration_cells() -> TestResult {
    let source = r"
        function capture(offset) {
            var callbacks = [];
            for (let index = 0; index < 4; index = index + 1) {
                callbacks.push(function () { return offset + index; });
            }
            return callbacks;
        }
        var first = capture(0);
        var second = capture(10);
        first[0]() === 0 && first[1]() === 1 &&
            first[2]() === 2 && first[3]() === 3 &&
            second[0]() === 10 && second[1]() === 11 &&
            second[2]() === 12 && second[3]() === 13
    ";
    ensure_modes_value(source, &Value::Bool(true))
}

#[test]
fn linear_loop_plans_decline_with_environment_bindings() -> TestResult {
    let source = r"
        var total = 100;
        var scope = { total: 1, index: 0 };
        with (scope) {
            for (index = 0; index < 4; index = index + 1) {
                total = total + index;
            }
        }
        total === 100 && scope.total === 7 && scope.index === 4
    ";
    ensure_modes_value(source, &Value::Bool(true))
}

#[test]
fn linear_loop_plans_observe_direct_eval_binding_mutations() -> TestResult {
    let source = r#"
        function accumulate(seed) {
            var total = seed;
            for (var index = 0; index < 4; index = index + 1) {
                eval("total = total + index;");
            }
            return total;
        }
        accumulate(1) * 100 + accumulate(10)
    "#;
    ensure_modes_value(source, &Value::Number(716.0))
}

#[test]
fn direct_loop_update_errors_preserve_named_source_spans() -> TestResult {
    const SOURCE_NAME: &str = "linear-update-error.js";
    let cases = [
        (
            "for (const index = 0; index < 2; index = index + 1) {}",
            "index = index + 1",
        ),
        ("for (const index = 0; index < 2; index++) {}", "index++"),
    ];
    let runtime = Runtime::new();
    for (source, marker) in cases {
        let script = runtime.compile_named(SOURCE_NAME, source)?;
        let expected_span = named_marker_span(SOURCE_NAME, source, marker)?;
        for mode in MODES {
            let mut vm = vm_with_mode(mode);
            let error = vm
                .eval_compiled(&script)
                .err()
                .ok_or("expected immutable loop update to fail")?;
            if error.javascript_error_name() != Some("TypeError") {
                return Err(format!("expected TypeError in {mode:?} mode, got {error}").into());
            }
            if error.source_span() != Some(expected_span) {
                return Err(format!(
                    "expected update span {expected_span:?} in {mode:?} mode, got {:?}",
                    error.source_span()
                )
                .into());
            }
            ensure_direct_path_mode(&vm, mode)?;
        }
    }
    Ok(())
}

#[test]
fn linear_loop_plans_preserve_runtime_limits_and_source_identity() -> TestResult {
    const SOURCE_NAME: &str = "linear-loop-limit.js";
    const SOURCE: &str = r"
        var index = 0;
        while (index < 1000) {
            index = index + 1;
        }
        index
    ";
    let runtime = Runtime::new();
    let script = runtime.compile_named(SOURCE_NAME, SOURCE)?;
    for mode in MODES {
        let limits = RuntimeLimits {
            max_runtime_steps: 64,
            ..RuntimeLimits::default()
        };
        let mut vm = Vm::with_config(VmConfig::with_limits(limits).with_optimization_mode(mode));
        let error = vm
            .eval_compiled(&script)
            .err()
            .ok_or("expected linear loop to exhaust runtime steps")?;
        if !matches!(error, Error::ResourceLimit { .. }) {
            return Err(format!("expected resource limit in {mode:?} mode, got {error}").into());
        }
        let span = error
            .source_span()
            .ok_or("expected runtime limit to retain the executing source span")?;
        if span.source_id() != script.source_id()
            || span.is_empty()
            || SOURCE.get(span.start()..span.end()).is_none()
        {
            return Err(format!("invalid runtime limit span in {mode:?} mode: {span:?}").into());
        }
        ensure_direct_path_mode(&vm, mode)?;
    }
    Ok(())
}

fn vm_with_mode(mode: OptimizationMode) -> Vm {
    Vm::with_config(VmConfig::default().with_optimization_mode(mode))
}

fn ensure_modes_value(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let script = runtime.compile(source)?;
    for mode in MODES {
        let mut vm = vm_with_mode(mode);
        for _ in 0..2 {
            let actual = vm.eval_compiled(&script)?;
            if &actual != expected {
                return Err(format!(
                    "expected {expected:?} in {mode:?} mode, got {actual:?}; source: {source}"
                )
                .into());
            }
        }
    }
    Ok(())
}

fn ensure_direct_path_mode(vm: &Vm, mode: OptimizationMode) -> TestResult {
    let runs = vm.optimization_snapshot().bytecode_linear_direct_runs();
    match mode {
        OptimizationMode::Enabled if runs > 0 => Ok(()),
        OptimizationMode::Disabled if runs == 0 => Ok(()),
        _ => Err(format!("unexpected linear direct runs in {mode:?} mode: {runs}").into()),
    }
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn named_marker_span(
    source_name: &str,
    source: &str,
    marker: &str,
) -> Result<SourceSpan, Box<dyn std::error::Error>> {
    let start = source
        .find(marker)
        .ok_or_else(|| format!("missing source marker {marker:?}"))?;
    let end = start
        .checked_add(marker.len())
        .ok_or("source marker range overflowed")?;
    SourceSpan::new(SourceId::for_named_source(source_name, source), start, end)
        .ok_or_else(|| "source marker range is reversed".into())
}
