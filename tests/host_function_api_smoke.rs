use velum::{Engine, Error, HostCall, Value, VmStorageKind};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const HOST_ADD_NAME: &str = "hostAdd";
const HOST_ECHO_NAME: &str = "hostEcho";
const HOST_FAIL_NAME: &str = "hostFail";
const HOST_FORMAT_NAME: &str = "hostFormat";
const HOST_LABEL_NAME: &str = "hostLabel";
const HOST_LEAK_NAME: &str = "hostLeak";
const HOST_LOCAL_STRING_NAME: &str = "hostLocalString";
const HOST_LOCAL_SYMBOL_NAME: &str = "hostLocalSymbol";
const HOST_NOOP_NAME: &str = "hostNoop";
const HOST_OWNED_NAME: &str = "hostOwned";
const HOST_SCORE_NAME: &str = "hostScore";
const HOST_READY_NAME: &str = "hostReady";
const CAMERA_LABEL: &str = "camera";

fn string_payload_bytes(value: &str) -> usize {
    value
        .encode_utf16()
        .count()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(value.len())
}

#[test]
fn registers_typed_host_functions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function(HOST_ADD_NAME, host_add)?;

    let value = vm.context().eval("hostAdd(40, 2)")?;
    ensure_value(&value, &Value::Number(42.0))?;

    let type_name = vm.context().eval("typeof hostAdd")?;
    ensure_value(&type_name, &Value::from("function"))
}

#[test]
fn host_functions_are_valid_weak_collection_keys() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function(HOST_ADD_NAME, host_add)?;

    vm.context().eval(
        r#"
        var hostWeakValue = { kind: "host" };
        var hostWeakMap = new WeakMap([[hostAdd, hostWeakValue]]);
        var hostWeakSet = new WeakSet([hostAdd]);
        "#,
    )?;
    vm.collect_garbage()?;
    let actual = vm.context().eval(
        r#"
        "" + (hostWeakMap.get(hostAdd) === hostWeakValue)
            + ":" + hostWeakSet.has(hostAdd)
            + ":" + hostWeakMap.delete(hostAdd)
            + ":" + hostWeakSet.delete(hostAdd)
        "#,
    )?;
    ensure_value(&actual, &Value::from("true:true:true:true"))
}

#[test]
fn host_functions_expose_ordinary_function_object_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    for name in ["hostObject", "hostSealed", "hostFrozen"] {
        vm.context()
            .register_host_function_typed(name, |_call| Ok(()))?;
    }

    vm.context().eval(
        r#"
        var hostSymbol = Symbol("host");
        var inheritedHostState = { inherited: 40 };
        var hostSetterValue = 0;
        Object.defineProperty(hostObject, "answer", {
            get: function () { return 42; },
            set: function (value) { hostSetterValue = value; },
            enumerable: true,
            configurable: true
        });
        hostObject.extra = 2;
        hostObject[hostSymbol] = 3;
        Object.defineProperties(hostObject, {
            batch: {
                value: 4,
                writable: true,
                enumerable: true,
                configurable: true
            }
        });
        Object.assign(hostObject, { assigned: 5 });
        (function attachOnlyHostOwnedState() {
            const metadata = { answer: 42 };
            hostObject.metadata = metadata;
            Object.defineProperty(hostObject, "computed", {
                get: function () { return metadata.answer; },
                configurable: true
            });
        })();
        hostSealed.value = 1;
        hostFrozen.value = 1;
        var copiedHostState = Object.assign({}, hostObject);
        Object.seal(hostSealed);
        Object.freeze(hostFrozen);
        "#,
    )?;
    vm.collect_garbage()?;

    let actual = vm.context().eval(
        r#"
        const nameDescriptor = Object.getOwnPropertyDescriptor(hostObject, "name");
        const lengthDescriptor = Object.getOwnPropertyDescriptor(hostObject, "length");
        const keys = Reflect.ownKeys(hostObject);
        hostObject.answer = 41;
        const initial = Object.getPrototypeOf(hostObject) === Function.prototype
            && hostObject instanceof Function
            && hostObject.call(undefined) === undefined
            && hostObject.name === "hostObject"
            && hostObject.length === 0
            && nameDescriptor.value === "hostObject"
            && nameDescriptor.writable === false
            && nameDescriptor.enumerable === false
            && nameDescriptor.configurable === true
            && lengthDescriptor.value === 0
            && lengthDescriptor.writable === false
            && lengthDescriptor.enumerable === false
            && lengthDescriptor.configurable === true
            && Object.hasOwn(hostObject, "answer")
            && "answer" in hostObject
            && hostObject.answer === 42
            && hostSetterValue === 41
            && Object.keys(hostObject).join(",") === "answer,extra,batch,assigned,metadata"
            && keys[0] === "length"
            && keys[1] === "name"
            && keys.indexOf("answer") > 1
            && keys[keys.length - 1] === hostSymbol
            && Object.getOwnPropertySymbols(hostObject)[0] === hostSymbol
            && hostObject.metadata.answer === 42
            && hostObject.computed === 42
            && copiedHostState.answer === 42
            && copiedHostState.batch + copiedHostState.assigned === 9
            && Reflect.setPrototypeOf(hostObject, hostObject) === false;
        const changed = Object.setPrototypeOf(hostObject, inheritedHostState) === hostObject
            && Object.getPrototypeOf(hostObject) === inheritedHostState
            && hostObject.inherited + hostObject.extra === 42;
        const deleted = delete hostObject.extra
            && !Object.hasOwn(hostObject, "extra");
        Object.preventExtensions(hostObject);
        initial && changed && deleted
            && Object.isExtensible(hostObject) === false
            && Object.isSealed(hostSealed)
            && Object.isFrozen(hostFrozen) ? 42 : 0
        "#,
    )?;
    vm.storage_snapshot()?;
    ensure_value(&actual, &Value::Number(42.0))
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
    ensure_value(&formatted, &Value::from("front:7:true"))?;

    let score = vm.context().eval("hostScore()")?;
    ensure_value(&score, &Value::Number(42.0))?;

    let ready = vm.context().eval("hostReady()")?;
    ensure_value(&ready, &Value::Bool(true))?;

    let noop_type = vm.context().eval("typeof hostNoop()")?;
    ensure_value(&noop_type, &Value::from("undefined"))
}

#[test]
fn interns_host_returned_strings_in_vm_heap() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function_typed(HOST_LABEL_NAME, |_call| -> velum::Result<&'static str> {
            Ok(CAMERA_LABEL)
        })?;
    vm.context()
        .register_host_function_typed(HOST_OWNED_NAME, |_call| -> velum::Result<String> {
            Ok(CAMERA_LABEL.to_owned())
        })?;
    vm.context()
        .register_host_function(HOST_ECHO_NAME, |_call| Ok(Value::from(CAMERA_LABEL)))?;

    let registration_baseline = vm.resource_usage();

    let static_label = vm.context().eval("hostLabel()")?;
    ensure_value(&static_label, &Value::from(CAMERA_LABEL))?;
    let after_static_label = vm.resource_usage();
    let expected_count = registration_baseline
        .string_count
        .checked_add(1)
        .ok_or("host string count overflowed")?;
    ensure_usize(after_static_label.string_count, expected_count)?;
    let expected_bytes = registration_baseline
        .string_bytes
        .checked_add(string_payload_bytes(CAMERA_LABEL))
        .ok_or("host string byte count overflowed")?;
    ensure_usize(after_static_label.string_bytes, expected_bytes)?;

    let owned_label = vm.context().eval("hostOwned()")?;
    ensure_value(&owned_label, &Value::from(CAMERA_LABEL))?;
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
    ensure_value(&legacy_label, &Value::from(CAMERA_LABEL))?;
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
    ensure_value(&value, &Value::from("front"))?;

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
    let before_duplicate = vm.storage_snapshot()?;

    let Err(error) = vm
        .context()
        .register_host_function(HOST_ECHO_NAME, host_echo)
    else {
        return Err("expected duplicate host function registration to fail".into());
    };
    ensure_error_contains(&error, "'hostEcho' has already been declared")?;
    let after_duplicate = vm.storage_snapshot()?;
    ensure_usize(
        after_duplicate.count(VmStorageKind::ObjectProperty),
        before_duplicate.count(VmStorageKind::ObjectProperty),
    )?;
    ensure_usize(
        after_duplicate.count(VmStorageKind::CacheEntry),
        before_duplicate.count(VmStorageKind::CacheEntry),
    )
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

#[test]
fn permits_same_vm_heap_string_and_symbol_returns() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let string_value = vm.context().eval(r#""camera""#)?;
    let symbol_value = vm
        .context()
        .eval(r#"let localSymbol = Symbol("camera"); localSymbol"#)?;

    vm.context()
        .register_host_function(
            HOST_LOCAL_STRING_NAME,
            move |_call| Ok(string_value.clone()),
        )?;
    vm.context()
        .register_host_function(
            HOST_LOCAL_SYMBOL_NAME,
            move |_call| Ok(symbol_value.clone()),
        )?;

    let actual = vm
        .context()
        .eval(r#"hostLocalString() === "camera" && hostLocalSymbol() === localSymbol"#)?;
    ensure_value(&actual, &Value::Bool(true))
}

#[test]
fn rejects_foreign_heap_strings_even_when_slots_collide() -> TestResult {
    let engine = Engine::new();
    let mut first_vm = engine.create_vm();
    let mut second_vm = engine.create_vm();
    let Value::String(foreign) = first_vm.context().eval(r#""camera""#)? else {
        return Err("expected first VM to return a heap string".into());
    };
    let Value::String(local) = second_vm.context().eval(r#""camera""#)? else {
        return Err("expected second VM to return a heap string".into());
    };
    if foreign.id() != local.id() {
        return Err("test setup did not create colliding string slots".into());
    }
    if foreign.identity() == local.identity() {
        return Err("independent string heaps shared an owner identity".into());
    }

    second_vm
        .context()
        .register_host_function(HOST_LOCAL_STRING_NAME, move |_call| {
            Ok(Value::String(foreign.clone()))
        })?;
    let Err(error) = second_vm.context().eval("hostLocalString()") else {
        return Err("expected a foreign heap string return to fail".into());
    };
    ensure_error_contains(
        &error,
        "host function 'hostLocalString': value belongs to another VM",
    )
}

#[test]
fn rejects_foreign_symbols_even_when_slots_collide() -> TestResult {
    let engine = Engine::new();
    let mut first_vm = engine.create_vm();
    let mut second_vm = engine.create_vm();
    let Value::Symbol(foreign) = first_vm.context().eval(r#"Symbol("camera")"#)? else {
        return Err("expected first VM to return a Symbol".into());
    };
    let Value::Symbol(local) = second_vm.context().eval(r#"Symbol("camera")"#)? else {
        return Err("expected second VM to return a Symbol".into());
    };
    if foreign.id() != local.id() {
        return Err("test setup did not create colliding Symbol slots".into());
    }
    if foreign == local || foreign.identity() == local.identity() {
        return Err("independent Symbols shared VM-local identity".into());
    }

    second_vm
        .context()
        .register_host_function(HOST_LOCAL_SYMBOL_NAME, move |_call| {
            Ok(Value::Symbol(foreign.clone()))
        })?;
    let Err(error) = second_vm.context().eval("hostLocalSymbol()") else {
        return Err("expected a foreign Symbol return to fail".into());
    };
    ensure_error_contains(
        &error,
        "host function 'hostLocalSymbol': value belongs to another VM",
    )
}

fn host_add(call: HostCall<'_>) -> velum::Result<Value> {
    let left = call.number(0, "left")?;
    let right = call.number(1, "right")?;
    Ok(Value::Number(left + right))
}

fn host_echo(call: HostCall<'_>) -> velum::Result<Value> {
    let value = call.string(0, "value")?;
    Ok(Value::from(value))
}

fn host_format(call: HostCall<'_>) -> velum::Result<String> {
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
