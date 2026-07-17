use velum::{
    AccessorPropertyDefinition, DataPropertyDefinition, Engine, Error, JsString, JsValueRef,
    OwnedValue, PropertyDefinition, PropertyDescriptor, PropertyKeyRef, RetainedValue,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const PROXY_FIXTURE: &str = r#"
var events = [];
var token = Symbol("token");
const target = {
    count: 4,
    get doubled() {
        events.push("get:doubled");
        return this.count * 2;
    }
};
var proxy = new Proxy(target, {
    get(target, property, receiver) {
        events.push(`proxy:get:${String(property)}`);
        return Reflect.get(target, property, receiver);
    },
    set(target, property, value, receiver) {
        events.push(`proxy:set:${String(property)}:${value}`);
        return Reflect.set(target, property, value, receiver);
    },
    defineProperty(target, property, descriptor) {
        events.push(`proxy:define:${String(property)}:${descriptor.value}`);
        return Reflect.defineProperty(target, property, descriptor);
    },
    deleteProperty(target, property) {
        events.push(`proxy:delete:${String(property)}`);
        return Reflect.deleteProperty(target, property);
    },
    getOwnPropertyDescriptor(target, property) {
        events.push(`proxy:descriptor:${String(property)}`);
        return Reflect.getOwnPropertyDescriptor(target, property);
    }
});
Object.defineProperty(proxy, "boom", {
    configurable: true,
    get() { throw new RangeError("access denied"); }
});
"#;

#[test]
fn calls_with_checked_values_receivers_and_explicit_result_ownership() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r#"
        var callCount = 0;
        var combine = function (left, right, exact) {
            callCount += 1;
            return `${this.prefix}:${left + right}:${exact.length}:${exact.charCodeAt(0)}`;
        };
        var receiver = { prefix: "device" };
        "#,
    )?;
    let function = required_global(&vm, "combine")?;
    let receiver = required_global(&vm, "receiver")?;
    let exact = JsString::from_utf16(vec![0xd800]);
    let args = [
        JsValueRef::Number(2.0),
        JsValueRef::Number(3.0),
        JsValueRef::ExactString(&exact),
    ];

    ensure_true(vm.is_callable(&function)?, "function should be callable")?;
    ensure_false(
        vm.is_constructor(&receiver)?,
        "ordinary object should not be a constructor",
    )?;
    ensure_owned(
        &vm.call_with_receiver_owned(&function, (&receiver).into(), &args)?,
        &OwnedValue::String("device:5:1:55296".to_owned()),
    )?;

    let result = vm.call_with_receiver_retained(
        &function,
        (&receiver).into(),
        &[
            JsValueRef::String("retained"),
            JsValueRef::Number(1.0),
            JsValueRef::String("abc"),
        ],
    )?;
    ensure_owned(
        &vm.retained_to_owned(&result)?,
        &OwnedValue::String("device:retained1:3:97".to_owned()),
    )?;
    result.release()?;

    function.release()?;
    receiver.release()?;
    Ok(())
}

#[test]
fn rejects_foreign_handles_before_javascript_dispatch() -> TestResult {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    first.eval(
        r"
        var callCount = 0;
        var record = function (value) {
            callCount += 1;
            return value;
        };
        ",
    )?;
    let function = required_global(&first, "record")?;

    let mut second = engine.create_vm();
    let foreign = second.eval_retained("({ owner: 'second' })")?;
    let Err(error) = first.call(&function, &[(&foreign).into()]) else {
        return Err("foreign argument entered a JavaScript call".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;
    ensure_owned(&first.eval_owned("callCount")?, &OwnedValue::Number(0.0))?;

    let Err(error) = second.call(&function, &[]) else {
        return Err("foreign callable entered a JavaScript call".into());
    };
    ensure_runtime_error(&error, "retained value belongs to another VM")?;

    function.release()?;
    foreign.release()?;
    Ok(())
}

#[test]
fn constructs_and_controls_a_javascript_class_without_eval_bridges() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(
        r"
        class Device {
            constructor(name) {
                this.name = name;
                this.count = 1;
            }

            bump(amount) {
                this.count += amount;
                return `${this.name}:${this.count}`;
            }
        }
        globalThis.Device = Device;
        ",
    )?;
    let constructor = required_global(&vm, "Device")?;
    ensure_true(
        vm.is_constructor(&constructor)?,
        "class should be a constructor",
    )?;
    let device = vm.construct_retained(&constructor, &[JsValueRef::String("camera")])?;

    ensure_owned(
        &vm.call_method_owned(
            (&device).into(),
            PropertyKeyRef::Name("bump"),
            &[JsValueRef::Number(2.0)],
        )?,
        &OwnedValue::String("camera:3".to_owned()),
    )?;
    ensure_true(
        vm.set_property(
            (&device).into(),
            PropertyKeyRef::Name("count"),
            JsValueRef::Number(10.0),
        )?,
        "count assignment should succeed",
    )?;
    ensure_owned(
        &vm.get_property_owned((&device).into(), PropertyKeyRef::Name("count"))?,
        &OwnedValue::Number(10.0),
    )?;

    let definition = DataPropertyDefinition::new(JsValueRef::String("A-17"))
        .with_writable(true)
        .with_enumerable(false)
        .with_configurable(true);
    vm.define_property_or_throw(
        (&device).into(),
        PropertyKeyRef::Name("serial"),
        definition.into(),
    )?;
    let Some(descriptor) =
        vm.get_own_property_descriptor((&device).into(), PropertyKeyRef::Name("serial"))?
    else {
        return Err("defined serial property has no descriptor".into());
    };
    let PropertyDescriptor::Data {
        value,
        writable,
        enumerable,
        configurable,
    } = descriptor
    else {
        return Err("serial property unexpectedly became an accessor".into());
    };
    ensure_owned(
        &vm.retained_to_owned(&value)?,
        &OwnedValue::String("A-17".to_owned()),
    )?;
    ensure_true(writable, "serial should be writable")?;
    ensure_false(enumerable, "serial should not be enumerable")?;
    ensure_true(configurable, "serial should be configurable")?;
    value.release()?;

    vm.collect_garbage()?;
    ensure_owned(
        &vm.call_method_owned(
            (&device).into(),
            PropertyKeyRef::Name("bump"),
            &[JsValueRef::Number(1.0)],
        )?,
        &OwnedValue::String("camera:11".to_owned()),
    )?;
    ensure_true(
        vm.delete_property((&device).into(), PropertyKeyRef::Name("serial"))?,
        "serial deletion should succeed",
    )?;
    if vm
        .get_own_property_descriptor((&device).into(), PropertyKeyRef::Name("serial"))?
        .is_some()
    {
        return Err("deleted serial property still has a descriptor".into());
    }

    constructor.release()?;
    device.release()?;
    Ok(())
}

#[test]
fn observes_accessors_proxies_symbols_and_javascript_exceptions() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.eval(PROXY_FIXTURE)?;
    let proxy = required_global(&vm, "proxy")?;
    let token = required_global(&vm, "token")?;

    ensure_owned(
        &vm.get_property_owned((&proxy).into(), PropertyKeyRef::Name("doubled"))?,
        &OwnedValue::Number(8.0),
    )?;
    ensure_true(
        vm.set_property(
            (&proxy).into(),
            PropertyKeyRef::Symbol(&token),
            JsValueRef::Number(41.0),
        )?,
        "symbol assignment through Proxy should succeed",
    )?;
    ensure_owned(
        &vm.get_property_owned((&proxy).into(), PropertyKeyRef::Symbol(&token))?,
        &OwnedValue::Number(41.0),
    )?;

    let getter = vm.eval_retained("(function () { return this.count + 1; })")?;
    let accessor = AccessorPropertyDefinition::new()
        .with_getter((&getter).into())
        .with_enumerable(true)
        .with_configurable(true);
    vm.define_property_or_throw(
        (&proxy).into(),
        PropertyKeyRef::Name("next"),
        PropertyDefinition::Accessor(accessor),
    )?;
    ensure_owned(
        &vm.get_property_owned((&proxy).into(), PropertyKeyRef::Name("next"))?,
        &OwnedValue::Number(5.0),
    )?;
    let Some(PropertyDescriptor::Accessor {
        getter: descriptor_getter,
        setter,
        enumerable,
        configurable,
    }) = vm.get_own_property_descriptor((&proxy).into(), PropertyKeyRef::Name("next"))?
    else {
        return Err("next property did not return an accessor descriptor".into());
    };
    let Some(descriptor_getter) = descriptor_getter else {
        return Err("next descriptor lost its getter".into());
    };
    ensure_true(
        vm.is_callable(&descriptor_getter)?,
        "descriptor getter should remain callable",
    )?;
    ensure_true(setter.is_none(), "next should not have a setter")?;
    ensure_true(enumerable, "next should be enumerable")?;
    ensure_true(configurable, "next should be configurable")?;
    descriptor_getter.release()?;

    let Err(error) = vm.get_property((&proxy).into(), PropertyKeyRef::Name("boom")) else {
        return Err("throwing getter unexpectedly returned a value".into());
    };
    ensure_javascript_error(&vm, &error, "RangeError", "access denied")?;

    ensure_true(
        vm.delete_property((&proxy).into(), PropertyKeyRef::Name("next"))?,
        "Proxy deletion should succeed",
    )?;
    let output = vm.eval_owned("events.join('|')")?;
    let OwnedValue::String(output) = output else {
        return Err("event log was not a string".into());
    };
    for expected in [
        "proxy:get:doubled",
        "get:doubled",
        "proxy:set:Symbol(token):41",
        "proxy:define:next:undefined",
        "proxy:descriptor:next",
        "proxy:delete:next",
    ] {
        if !output.contains(expected) {
            return Err(format!("event log did not contain {expected:?}: {output}").into());
        }
    }

    getter.release()?;
    proxy.release()?;
    token.release()?;
    Ok(())
}

#[test]
fn exposes_boolean_and_throwing_property_failure_modes() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let frozen = vm.eval_retained("Object.freeze({ fixed: 1 })")?;

    ensure_false(
        vm.set_property(
            (&frozen).into(),
            PropertyKeyRef::Name("fixed"),
            JsValueRef::Number(2.0),
        )?,
        "Reflect-style assignment should report false",
    )?;
    let Err(error) = vm.set_property_or_throw(
        (&frozen).into(),
        PropertyKeyRef::Name("fixed"),
        JsValueRef::Number(2.0),
    ) else {
        return Err("strict assignment unexpectedly succeeded".into());
    };
    ensure_javascript_error(&vm, &error, "TypeError", "Cannot assign")?;

    ensure_false(
        vm.delete_property((&frozen).into(), PropertyKeyRef::Name("fixed"))?,
        "Reflect-style deletion should report false",
    )?;
    let Err(error) = vm.delete_property_or_throw((&frozen).into(), PropertyKeyRef::Name("fixed"))
    else {
        return Err("strict deletion unexpectedly succeeded".into());
    };
    ensure_javascript_error(&vm, &error, "TypeError", "Cannot delete")?;

    let definition = DataPropertyDefinition::new(JsValueRef::Number(3.0));
    ensure_false(
        vm.define_property(
            (&frozen).into(),
            PropertyKeyRef::Name("added"),
            definition.into(),
        )?,
        "Reflect-style definition should report false",
    )?;
    let Err(error) = vm.define_property_or_throw(
        (&frozen).into(),
        PropertyKeyRef::Name("added"),
        definition.into(),
    ) else {
        return Err("throwing property definition unexpectedly succeeded".into());
    };
    ensure_javascript_error(&vm, &error, "TypeError", "Cannot define")?;

    frozen.release()?;
    Ok(())
}

fn required_global(
    vm: &velum::Vm,
    name: &str,
) -> Result<RetainedValue, Box<dyn std::error::Error>> {
    let Some(value) = vm.get_global_retained(name)? else {
        return Err(format!("missing retained global {name:?}").into());
    };
    Ok(value)
}

fn ensure_owned(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_true(value: bool, message: &str) -> TestResult {
    if value {
        return Ok(());
    }
    Err(message.into())
}

fn ensure_false(value: bool, message: &str) -> TestResult {
    if !value {
        return Ok(());
    }
    Err(message.into())
}

fn ensure_runtime_error(error: &Error, expected: &str) -> TestResult {
    if matches!(error, Error::Runtime { .. }) && error.to_string().contains(expected) {
        return Ok(());
    }
    Err(format!("expected runtime error containing {expected:?}, got {error:?}").into())
}

fn ensure_javascript_error(
    vm: &velum::Vm,
    error: &Error,
    expected_name: &str,
    expected_message: &str,
) -> TestResult {
    if error.javascript_identity() != Some(vm.identity()) {
        return Err(format!("JavaScript error lost VM identity: {error:?}").into());
    }
    if error.javascript_error_name() != Some(expected_name) {
        return Err(format!("expected {expected_name}, got {error:?}").into());
    }
    if error
        .javascript_error_message()
        .is_some_and(|message| message.contains(expected_message))
    {
        return Ok(());
    }
    Err(format!("expected message containing {expected_message:?}, got {error:?}").into())
}
