use std::{
    cell::Cell,
    rc::Rc,
    task::{Context as TaskContext, Waker},
};

use parking_lot::Mutex;
use velum::{
    Engine, EngineConfig, Error, HostClass, HostInstance, HostMethodResult, JsValueRef, OwnedValue,
    PropertyKeyRef, RuntimeLimits, Value, Vm, VmStorageKind, VmStorageLimits,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[derive(Debug)]
struct MockSocket {
    url: String,
    ready_state: Cell<f64>,
    sent: Mutex<Vec<String>>,
}

#[derive(Debug)]
struct DropState {
    drops: Rc<Cell<usize>>,
}

impl Drop for DropState {
    fn drop(&mut self) {
        self.drops.set(self.drops.get().saturating_add(1));
    }
}

#[test]
fn registers_constructable_typed_class_with_ordinary_descriptors() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_mock_socket(&mut vm)?;

    let result = vm.eval_owned(
        r#"
        const socket = new WebSocket("ws://camera");
        socket.send("first");
        socket.readyState = 2;
        const clone = socket.cloneHandle();
        clone.send("second");
        socket.ownMarker = 7;

        const method = Object.getOwnPropertyDescriptor(WebSocket.prototype, "send");
        const accessor = Object.getOwnPropertyDescriptor(WebSocket.prototype, "readyState");
        const constructor = Object.getOwnPropertyDescriptor(WebSocket.prototype, "constructor");
        const prototype = Object.getOwnPropertyDescriptor(WebSocket, "prototype");
        [
            socket.url,
            socket.readyState,
            socket.sentCount,
            clone.sentCount,
            clone.ownMarker,
            clone instanceof WebSocket,
            WebSocket.isMock(socket),
            method.value.name,
            method.value.length,
            method.writable,
            method.enumerable,
            method.configurable,
            accessor.get.name,
            accessor.set.name,
            accessor.enumerable,
            accessor.configurable,
            constructor.value === WebSocket,
            constructor.enumerable,
            prototype.writable,
            prototype.enumerable,
            prototype.configurable,
            Object.keys(WebSocket.prototype).length
        ].join("|")
        "#,
    )?;

    ensure_owned(
        &result,
        &OwnedValue::String(
            "ws://camera|2|2|2||true|true|send|1|true|false|true|get readyState|set readyState|false|true|true|false|false|false|false|0"
                .to_owned(),
        ),
    )?;
    let socket = required_global(&vm, "socket")?;
    ensure_usize(
        vm.host_payload::<MockSocket>(&socket)?.sent.lock().len(),
        2,
        "shared mock send history",
    )?;
    socket.release()?;
    Ok(())
}

#[test]
fn host_construction_honors_subclass_new_target_and_rejects_plain_calls() -> TestResult {
    let mut vm = Engine::new().create_vm();
    let constructor_calls = Rc::new(Cell::new(0_usize));
    let constructor_calls_for_callback = Rc::clone(&constructor_calls);
    let class = mock_socket_class(move |call| {
        ensure_true(call.new_target().is_some(), "host constructor new.target")
            .map_err(|error| Error::runtime(error.to_string()))?;
        constructor_calls_for_callback.set(constructor_calls_for_callback.get().saturating_add(1));
        create_mock_socket(call.string(0, "url")?)
    });
    vm.register_host_class(class)?;

    let result = vm.eval_owned(
        r#"
        class DerivedSocket extends WebSocket {}
        DerivedSocket.prototype.kind = "derived";
        const derived = new DerivedSocket("ws://derived");
        const plainCall = (() => {
            try { WebSocket("ws://plain"); } catch (error) {
                return error instanceof TypeError && error.message.includes("without 'new'");
            }
            return false;
        })();
        [
            derived instanceof DerivedSocket,
            derived instanceof WebSocket,
            Object.getPrototypeOf(derived) === DerivedSocket.prototype,
            derived.kind,
            derived.url,
            plainCall
        ].join("|")
        "#,
    )?;
    ensure_owned(
        &result,
        &OwnedValue::String("true|true|true|derived|ws://derived|true".to_owned()),
    )?;
    ensure_usize(
        constructor_calls.get(),
        1,
        "successful host constructor calls",
    )
}

#[test]
fn rust_constructs_and_calls_host_instances_through_the_public_facade() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_mock_socket(&mut vm)?;
    let constructor = required_global(&vm, "WebSocket")?;
    ensure_true(
        vm.is_constructor(&constructor)?,
        "host class constructability",
    )?;
    let instance = vm.construct_retained(&constructor, &[JsValueRef::String("ws://rust")])?;

    ensure_owned(
        &vm.call_method_owned(
            JsValueRef::Retained(&instance),
            PropertyKeyRef::Name("send"),
            &[JsValueRef::String("from-rust")],
        )?,
        &OwnedValue::Undefined,
    )?;
    ensure_owned(
        &vm.get_property_owned(
            JsValueRef::Retained(&instance),
            PropertyKeyRef::Name("sentCount"),
        )?,
        &OwnedValue::Number(1.0),
    )?;
    let returned = vm.call_method_retained(
        JsValueRef::Retained(&instance),
        PropertyKeyRef::Name("selfHandle"),
        &[],
    )?;
    ensure_true(
        std::ptr::eq(
            vm.host_payload::<MockSocket>(&instance)?,
            vm.host_payload::<MockSocket>(&returned)?,
        ),
        "retained host method result identity",
    )?;

    returned.release()?;
    instance.release()?;
    constructor.release()?;
    Ok(())
}

#[test]
fn async_methods_copy_state_before_the_future_and_use_promise_jobs() -> TestResult {
    let mut vm = Engine::new().create_vm();
    register_mock_socket(&mut vm)?;
    vm.eval(
        r#"
        var asyncSocket = new WebSocket("ws://async");
        var asyncStatus = "pending";
        asyncSocket.describeLater("ready").then(function (value) {
            asyncStatus = value;
        });
        "#,
    )?;

    ensure_usize(
        vm.pending_host_future_count(),
        1,
        "pending class method future",
    )?;
    let mut task_context = TaskContext::from_waker(Waker::noop());
    let poll = vm.poll_host_futures(&mut task_context)?;
    ensure_usize(poll.completed(), 1, "completed class method future")?;
    ensure_usize(vm.run_jobs()?, 1, "class method Promise reactions")?;
    ensure_value(
        vm.get_global("asyncStatus").as_ref(),
        &Value::from("ws://async:ready"),
        "async class method result",
    )
}

#[test]
fn methods_reject_incompatible_receivers_before_entering_user_logic() -> TestResult {
    let mut vm = Engine::new().create_vm();
    let entered = Rc::new(Cell::new(false));
    let entered_for_callback = Rc::clone(&entered);
    let class = HostClass::new("Checked", |_call| Ok(HostInstance::new((), 0))).method(
        "touch",
        move |_payload, _call| {
            entered_for_callback.set(true);
            Ok(42.0)
        },
    );
    vm.register_host_class(class)?;

    let Err(error) = vm.eval("Checked.prototype.touch.call({})") else {
        return Err("incompatible host method receiver was accepted".into());
    };
    ensure_true(
        error.to_string().contains("incompatible Rust payload"),
        "wrong-receiver TypeError context",
    )?;
    ensure_true(!entered.get(), "wrong receiver entered user callback")
}

#[test]
fn invalid_or_duplicate_class_definitions_fail_before_vm_allocation() -> TestResult {
    let mut vm = Engine::new().create_vm();
    vm.register_host_class(HostClass::new("Unique", |_call| {
        Ok(HostInstance::new((), 0))
    }))?;
    let before = vm.storage_snapshot()?;
    let Err(error) = vm.register_host_class(HostClass::new("Unique", |_call| {
        Ok(HostInstance::new((), 0))
    })) else {
        return Err("duplicate host class binding was accepted".into());
    };
    ensure_true(
        error.to_string().contains("already been declared"),
        "duplicate class binding error",
    )?;
    let after_duplicate = vm.storage_snapshot()?;
    for kind in [
        VmStorageKind::Object,
        VmStorageKind::ObjectProperty,
        VmStorageKind::HostCallback,
        VmStorageKind::RetainedHandle,
    ] {
        ensure_usize(
            after_duplicate.count(kind),
            before.count(kind),
            "duplicate class allocation rollback",
        )?;
    }

    let invalid_before = vm.storage_snapshot()?;
    let invalid = HostClass::new("Invalid", |_call| Ok(HostInstance::new((), 0)))
        .method("same", |_payload, _call| Ok(()))
        .getter("same", |_payload, _call| Ok(42.0));
    let Err(error) = vm.register_host_class(invalid) else {
        return Err("conflicting host class members were accepted".into());
    };
    ensure_true(
        error.to_string().contains("mixes an accessor and method"),
        "conflicting class member error",
    )?;
    let invalid_after = vm.storage_snapshot()?;
    ensure_usize(
        invalid_after.count(VmStorageKind::HostCallback),
        invalid_before.count(VmStorageKind::HostCallback),
        "invalid class callback allocation",
    )?;
    ensure_usize(
        invalid_after.count(VmStorageKind::Object),
        invalid_before.count(VmStorageKind::Object),
        "invalid class object allocation",
    )?;

    let storage = VmStorageLimits::unlimited().with_max_count(VmStorageKind::HostCallback, 1);
    let mut limited = vm_with_storage_limits(storage);
    limited.eval("Function.prototype; Object.prototype")?;
    let limited_before = limited.storage_snapshot()?;
    let class = HostClass::new("CallbackLimited", |_call| Ok(HostInstance::new((), 0)))
        .method("method", |_payload, _call| Ok(()));
    let Err(error) = limited.register_host_class(class) else {
        return Err("host callback limit unexpectedly accepted a class".into());
    };
    ensure_true(
        error.to_string().contains("HostCallback"),
        "class callback limit error",
    )?;
    let limited_after = limited.storage_snapshot()?;
    for kind in [VmStorageKind::HostCallback, VmStorageKind::RetainedHandle] {
        let label = format!("limited class immediate {kind:?} rollback");
        ensure_usize(
            limited_after.count(kind),
            limited_before.count(kind),
            label.as_str(),
        )?;
    }
    limited.collect_garbage()?;
    let collected = limited.storage_snapshot()?;
    for kind in [
        VmStorageKind::Object,
        VmStorageKind::ObjectProperty,
        VmStorageKind::HostCallback,
        VmStorageKind::RetainedHandle,
    ] {
        let label = format!("limited class collected {kind:?} rollback");
        ensure_usize(
            collected.count(kind),
            limited_before.count(kind),
            label.as_str(),
        )?;
    }
    limited.register_host_class(HostClass::new("Recovered", |_call| {
        Ok(HostInstance::new((), 0))
    }))?;
    ensure_usize(
        limited
            .storage_snapshot()?
            .count(VmStorageKind::HostCallback),
        1,
        "callback capacity after class rollback",
    )?;
    Ok(())
}

#[test]
fn traced_instance_cycles_collect_and_payload_limits_roll_back() -> TestResult {
    let drops = Rc::new(Cell::new(0_usize));
    let drops_for_constructor = Rc::clone(&drops);
    let mut vm = Engine::new().create_vm();
    vm.register_host_class(HostClass::new("Traced", move |call| {
        let traced = call.required_value(0, "traced")?.retain()?;
        Ok(HostInstance::new(
            DropState {
                drops: Rc::clone(&drops_for_constructor),
            },
            8,
        )
        .with_traced_values(vec![traced]))
    }))?;
    vm.eval(
        r"
        var tracedChild = {};
        var tracedHost = new Traced(tracedChild);
        tracedChild.host = tracedHost;
        tracedChild = null;
        tracedHost = null;
        ",
    )?;
    vm.collect_garbage()?;
    ensure_usize(drops.get(), 1, "collected class payload cycle")?;
    ensure_storage(
        &vm,
        VmStorageKind::HostInstance,
        0,
        "collected class instances",
    )?;
    ensure_storage(
        &vm,
        VmStorageKind::HostPayload,
        0,
        "collected class payloads",
    )?;

    let limited_drops = Rc::new(Cell::new(0_usize));
    let limited_drops_for_constructor = Rc::clone(&limited_drops);
    let storage =
        VmStorageLimits::unlimited().with_max_payload_bytes(VmStorageKind::HostPayload, 3);
    let mut limited = vm_with_storage_limits(storage);
    limited.register_host_class(HostClass::new("Limited", move |_call| {
        Ok(HostInstance::new(
            DropState {
                drops: Rc::clone(&limited_drops_for_constructor),
            },
            4,
        ))
    }))?;
    let Err(error) = limited.eval("new Limited()") else {
        return Err("host payload limit unexpectedly accepted a class instance".into());
    };
    ensure_true(
        error.to_string().contains("HostPayload"),
        "payload limit error",
    )?;
    ensure_usize(limited_drops.get(), 1, "rejected class payload drops")?;
    ensure_storage(
        &limited,
        VmStorageKind::HostInstance,
        0,
        "rolled back class instances",
    )?;
    ensure_storage(
        &limited,
        VmStorageKind::HostPayload,
        0,
        "rolled back class payloads",
    )
}

fn register_mock_socket(vm: &mut Vm) -> velum::Result<()> {
    vm.register_host_class(mock_socket_class(|call| {
        create_mock_socket(call.string(0, "url")?)
    }))
}

fn mock_socket_class<F>(constructor: F) -> HostClass<MockSocket>
where
    F: for<'call> Fn(velum::HostCall<'call>) -> velum::Result<HostInstance<MockSocket>> + 'static,
{
    HostClass::new("WebSocket", constructor)
        .with_constructor_length(1)
        .method_with_length("send", 1, |socket, call| {
            let message = call.string(0, "message")?.to_owned();
            let mut sent = socket.sent.lock();
            sent.try_reserve(1)
                .map_err(|_| Error::limit("mock send history capacity exceeded"))?;
            sent.push(message);
            drop(sent);
            Ok(())
        })
        .method("close", |socket, _call| {
            socket.ready_state.set(3.0);
            Ok(())
        })
        .method_with_result("cloneHandle", 0, |_socket, _call| {
            Ok(HostMethodResult::shared_receiver())
        })
        .method_with_result("selfHandle", 0, |_socket, call| {
            Ok(HostMethodResult::retained(call.receiver().retain()?))
        })
        .async_method("describeLater", 1, |socket, call| {
            let url = socket.url.clone();
            let suffix = call.string(0, "suffix")?.to_owned();
            Ok(async move { Ok(format!("{url}:{suffix}")) })
        })
        .getter("url", |socket, _call| Ok(socket.url.clone()))
        .getter("readyState", |socket, _call| Ok(socket.ready_state.get()))
        .setter("readyState", |socket, call| {
            socket.ready_state.set(call.number(0, "readyState")?);
            Ok(())
        })
        .getter("sentCount", |socket, _call| {
            let count = u32::try_from(socket.sent.lock().len())
                .map_err(|_| Error::limit("mock send history length exceeded u32"))?;
            Ok(f64::from(count))
        })
        .static_method("isMock", 1, |call| {
            Ok(call
                .value(0)
                .is_some_and(|value| value.host_payload::<MockSocket>().is_ok()))
        })
}

fn create_mock_socket(url: &str) -> velum::Result<HostInstance<MockSocket>> {
    let logical_payload_bytes = url
        .len()
        .checked_add(std::mem::size_of::<MockSocket>())
        .ok_or_else(|| Error::limit("mock socket payload bytes overflowed"))?;
    Ok(HostInstance::new(
        MockSocket {
            url: url.to_owned(),
            ready_state: Cell::new(1.0),
            sent: Mutex::new(Vec::new()),
        },
        logical_payload_bytes,
    ))
}

fn vm_with_storage_limits(storage: VmStorageLimits) -> Vm {
    let limits = RuntimeLimits {
        storage,
        ..RuntimeLimits::default()
    };
    Engine::with_config(EngineConfig::with_default_vm_config(
        velum::VmConfig::with_limits(limits),
    ))
    .create_vm()
}

fn required_global(vm: &Vm, name: &str) -> velum::Result<velum::RetainedValue> {
    vm.get_global_retained(name)?
        .ok_or_else(|| Error::runtime(format!("global '{name}' is missing")))
}

fn ensure_storage(vm: &Vm, kind: VmStorageKind, expected: usize, label: &str) -> TestResult {
    ensure_usize(vm.storage_snapshot()?.count(kind), expected, label)
}

fn ensure_owned(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_value(actual: Option<&Value>, expected: &Value, label: &str) -> TestResult {
    if actual == Some(expected) {
        return Ok(());
    }
    Err(format!("{label}: expected {expected:?}, got {actual:?}").into())
}

fn ensure_true(value: bool, label: &str) -> TestResult {
    if value {
        return Ok(());
    }
    Err(format!("{label}: expected true").into())
}

fn ensure_usize(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("{label}: expected {expected}, got {actual}").into())
}
