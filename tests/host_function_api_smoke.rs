use rs_quickjs::{Engine, Error, HostCall, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const HOST_ADD_NAME: &str = "hostAdd";
const HOST_ECHO_NAME: &str = "hostEcho";
const HOST_FAIL_NAME: &str = "hostFail";
const HOST_LEAK_NAME: &str = "hostLeak";

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
