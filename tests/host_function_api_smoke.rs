use rs_quickjs::{Engine, Error, HostCall, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const HOST_ADD_NAME: &str = "hostAdd";
const HOST_ECHO_NAME: &str = "hostEcho";
const HOST_FAIL_NAME: &str = "hostFail";
const HOST_FORMAT_NAME: &str = "hostFormat";
const HOST_LABEL_NAME: &str = "hostLabel";
const HOST_LEAK_NAME: &str = "hostLeak";
const HOST_NOOP_NAME: &str = "hostNoop";
const HOST_OWNED_NAME: &str = "hostOwned";
const HOST_SCORE_NAME: &str = "hostScore";
const HOST_READY_NAME: &str = "hostReady";
const CAMERA_LABEL: &str = "camera";

#[test]
fn registers_typed_host_functions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function(HOST_ADD_NAME, host_add)?;

    let value = vm.context().eval("hostAdd(40, 2)")?;
    ensure_value(&value, &Value::Number(42.0))?;

    let type_name = vm.context().eval("typeof hostAdd")?;
    ensure_value(&type_name, &Value::String("function".to_owned()))
}

#[test]
fn reports_contextual_host_argument_errors() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function(HOST_ADD_NAME, host_add)?;

    let Err(error) = vm.context().eval(r#"hostAdd("left", 2)"#) else {
        return Err("expected host argument type error".into());
    };
    ensure_error_contains(
        &error,
        "host function 'hostAdd': argument 'left' at index 0 expected number, got string",
    )
}

#[test]
fn supports_host_value_conversion_helpers() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function_typed(HOST_FORMAT_NAME, host_format)?;
    vm.context()
        .register_host_function_typed(HOST_SCORE_NAME, |_call| Ok(42.0))?;
    vm.context()
        .register_host_function_typed(HOST_READY_NAME, |_call| Ok(true))?;
    vm.context()
        .register_host_function_typed(HOST_NOOP_NAME, |_call| Ok(()))?;

    let formatted = vm.context().eval(r#"hostFormat(true, "front", 7)"#)?;
    ensure_value(&formatted, &Value::String("front:7:true".to_owned()))?;

    let score = vm.context().eval("hostScore()")?;
    ensure_value(&score, &Value::Number(42.0))?;

    let ready = vm.context().eval("hostReady()")?;
    ensure_value(&ready, &Value::Bool(true))?;

    let noop_type = vm.context().eval("typeof hostNoop()")?;
    ensure_value(&noop_type, &Value::String("undefined".to_owned()))
}

#[test]
fn interns_host_returned_strings_in_vm_heap() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context().register_host_function_typed(
        HOST_LABEL_NAME,
        |_call| -> rs_quickjs::Result<&'static str> { Ok(CAMERA_LABEL) },
    )?;
    vm.context()
        .register_host_function_typed(HOST_OWNED_NAME, |_call| -> rs_quickjs::Result<String> {
            Ok(CAMERA_LABEL.to_owned())
        })?;
    vm.context()
        .register_host_function(HOST_ECHO_NAME, |_call| {
            Ok(Value::String(CAMERA_LABEL.to_owned()))
        })?;

    ensure_usize(vm.resource_usage().string_count, 0)?;
    ensure_usize(vm.resource_usage().string_bytes, 0)?;

    let static_label = vm.context().eval("hostLabel()")?;
    ensure_value(&static_label, &Value::String(CAMERA_LABEL.to_owned()))?;
    let after_static_label = vm.resource_usage();
    ensure_usize(after_static_label.string_count, 1)?;
    ensure_usize(after_static_label.string_bytes, CAMERA_LABEL.len())?;

    let owned_label = vm.context().eval("hostOwned()")?;
    ensure_value(&owned_label, &Value::String(CAMERA_LABEL.to_owned()))?;
    let after_owned_label = vm.resource_usage();
    ensure_usize(
        after_owned_label.string_count,
        after_static_label.string_count,
    )?;
    ensure_usize(
        after_owned_label.string_bytes,
        after_static_label.string_bytes,
    )?;

    let legacy_label = vm.context().eval("hostEcho()")?;
    ensure_value(&legacy_label, &Value::String(CAMERA_LABEL.to_owned()))?;
    let after_legacy_label = vm.resource_usage();
    ensure_usize(
        after_legacy_label.string_count,
        after_static_label.string_count,
    )?;
    ensure_usize(
        after_legacy_label.string_bytes,
        after_static_label.string_bytes,
    )
}

#[test]
fn reports_contextual_generic_host_argument_errors() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function_typed(HOST_FORMAT_NAME, host_format)?;

    let Err(error) = vm.context().eval(r"hostFormat(true, 7, 2)") else {
        return Err("expected generic host argument type error".into());
    };
    ensure_error_contains(
        &error,
        "host function 'hostFormat': argument 'label' at index 1 expected string, got number",
    )
}

#[test]
fn keeps_host_functions_vm_local() -> TestResult {
    let engine = Engine::new();
    let mut first_vm = engine.create_vm();
    let mut second_vm = engine.create_vm();
    first_vm
        .context()
        .register_host_function(HOST_ECHO_NAME, host_echo)?;

    let value = first_vm.context().eval(r#"hostEcho("front")"#)?;
    ensure_value(&value, &Value::String("front".to_owned()))?;

    let Err(error) = second_vm.context().eval(r#"hostEcho("rear")"#) else {
        return Err("expected missing host function in second VM".into());
    };
    ensure_error_contains(&error, "ReferenceError: 'hostEcho' is not defined")
}

#[test]
fn rejects_duplicate_host_function_bindings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function(HOST_ECHO_NAME, host_echo)?;

    let Err(error) = vm
        .context()
        .register_host_function(HOST_ECHO_NAME, host_echo)
    else {
        return Err("expected duplicate host function registration to fail".into());
    };
    ensure_error_contains(&error, "'hostEcho' has already been declared")
}

#[test]
fn wraps_host_callback_errors_with_function_context() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function(HOST_FAIL_NAME, |_call| {
            Err(Error::runtime("camera offline"))
        })?;

    let Err(error) = vm.context().eval("hostFail()") else {
        return Err("expected host callback error".into());
    };
    ensure_error_contains(&error, "host function 'hostFail': camera offline")
}

#[test]
fn rejects_vm_owned_handles_returned_from_host_functions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let object = vm.context().eval("({ camera: 42 })")?;
    vm.context()
        .register_host_function(HOST_LEAK_NAME, move |_call| Ok(object.clone()))?;

    let Err(error) = vm.context().eval("hostLeak()") else {
        return Err("expected VM-owned host return value to fail".into());
    };
    ensure_error_contains(
        &error,
        "host functions cannot return VM-owned handles in the skeleton API",
    )
}

fn host_add(call: HostCall<'_>) -> rs_quickjs::Result<Value> {
    let left = call.number(0, "left")?;
    let right = call.number(1, "right")?;
    Ok(Value::Number(left + right))
}

fn host_echo(call: HostCall<'_>) -> rs_quickjs::Result<Value> {
    let value = call.string(0, "value")?;
    Ok(Value::String(value.to_owned()))
}

fn host_format(call: HostCall<'_>) -> rs_quickjs::Result<String> {
    let enabled: bool = call.argument(0, "enabled")?;
    let label: &str = call.argument(1, "label")?;
    let count: f64 = call.argument(2, "count")?;
    Ok(format!("{label}:{count:.0}:{enabled}"))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &Error, expected: &str) -> TestResult {
    let actual = error.to_string();
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error containing {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
