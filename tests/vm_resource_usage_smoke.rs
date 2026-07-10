use rs_quickjs::{Engine, Value, VmResourceUsage};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

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

    let report = vm.teardown_report()?;
    ensure_positive(report.resources.runtime_steps, "runtime steps")?;
    ensure_usage(
        &report.resources,
        &VmResourceUsage {
            runtime_steps: report.resources.runtime_steps,
            bytecode_linear_segment_runs: report.resources.bytecode_linear_segment_runs,
            bytecode_linear_direct_runs: report.resources.bytecode_linear_direct_runs,
            output_entries: 1,
            global_bindings: 1,
            atom_count: report.resources.atom_count,
            string_count: report.resources.string_count,
            string_bytes: report.resources.string_bytes,
            shape_count: report.resources.shape_count,
            native_function_count: report.resources.native_function_count,
            prototype_lookup_version: report.resources.prototype_lookup_version,
            upvalue_cell_count: report.resources.upvalue_cell_count,
            native_call_cache_hits: report.resources.native_call_cache_hits,
            native_call_cache_misses: report.resources.native_call_cache_misses,
            native_call_cache_slow_paths: report.resources.native_call_cache_slow_paths,
            call_value_cache_hits: report.resources.call_value_cache_hits,
            call_value_cache_misses: report.resources.call_value_cache_misses,
            call_value_cache_slow_paths: report.resources.call_value_cache_slow_paths,
        },
    )?;

    let finished = vm.finish()?;
    ensure_usage(&finished.resources, &report.resources)?;
    if finished.storage == report.storage {
        return Ok(());
    }
    Err(format!(
        "expected storage snapshot {:?}, got {:?}",
        report.storage, finished.storage
    )
    .into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
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
