use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    task::{Context as TaskContext, Poll, Waker},
};

use velum::{
    DataPropertyDefinition, Engine, HostFutureError, JsValueRef, OwnedValue, PropertyDefinition,
    PropertyDescriptor, PropertyKeyRef, QueuedCallRequest, QueuedCallResult, Vm,
};

const MAX_PUMP_TURNS: usize = 32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = Engine::new().create_vm();
    vm.eval(
        r"
        class Device {
            constructor(name) {
                this.name = name;
                this.count = 1;
            }

            describe(prefix) {
                return `${prefix}:${this.name}:${this.count}`;
            }

            async refresh(delta) {
                await Promise.resolve();
                this.count += delta;
                return this.count;
            }
        }
        ",
    )?;
    let constructor = vm
        .get_global_retained("Device")?
        .ok_or("Device was not defined")?;
    let device = vm.construct_retained(&constructor, &[JsValueRef::String("sensor")])?;
    constructor.release()?;

    let before = vm.call_method_owned(
        (&device).into(),
        PropertyKeyRef::Name("describe"),
        &[JsValueRef::String("before")],
    )?;
    println!("Initial method result: {before:?}");

    vm.set_property_or_throw(
        (&device).into(),
        PropertyKeyRef::Name("count"),
        JsValueRef::Number(10.0),
    )?;
    vm.define_property_or_throw(
        (&device).into(),
        PropertyKeyRef::Name("owner"),
        PropertyDefinition::Data(
            DataPropertyDefinition::new(JsValueRef::String("Rust"))
                .with_writable(true)
                .with_enumerable(true)
                .with_configurable(true),
        ),
    )?;

    let refresh = vm.get_property_retained((&device).into(), PropertyKeyRef::Name("refresh"))?;
    let request =
        vm.enqueue_call_with_receiver(&refresh, (&device).into(), &[JsValueRef::Number(5.0)])?;
    refresh.release()?;
    let refreshed = drive_request(&mut vm, request)?;
    if !matches!(
        refreshed,
        QueuedCallResult::Owned(OwnedValue::Number(value)) if (value - 15.0).abs() <= f64::EPSILON
    ) {
        return Err(format!("unexpected async refresh result: {refreshed:?}").into());
    }

    let receiver_calls = Rc::new(Cell::new(0_u32));
    let captured_calls = Rc::clone(&receiver_calls);
    let replacement = vm.create_host_function_typed("rustDescribe", move |call| {
        if call.receiver().as_value().type_name() != "object" {
            return Err(velum::Error::runtime(
                "replacement method lost its receiver",
            ));
        }
        let prefix = call.string(0, "prefix")?;
        captured_calls.set(captured_calls.get().saturating_add(1));
        Ok(format!("{prefix}:replacement-from-rust"))
    })?;
    vm.define_property_or_throw(
        (&device).into(),
        PropertyKeyRef::Name("describe"),
        PropertyDefinition::Data(
            DataPropertyDefinition::new((&replacement).into())
                .with_writable(true)
                .with_enumerable(false)
                .with_configurable(true),
        ),
    )?;
    replacement.release()?;

    let after = vm.call_method_owned(
        (&device).into(),
        PropertyKeyRef::Name("describe"),
        &[JsValueRef::String("after")],
    )?;
    let owner = vm.get_property_owned((&device).into(), PropertyKeyRef::Name("owner"))?;
    let count = vm.get_property_owned((&device).into(), PropertyKeyRef::Name("count"))?;
    println!("Replacement method result: {after:?}");
    println!("Fields controlled from Rust: owner={owner:?}, count={count:?}");

    let descriptor = vm
        .get_own_property_descriptor((&device).into(), PropertyKeyRef::Name("owner"))?
        .ok_or("owner descriptor was not defined")?;
    release_descriptor(descriptor)?;
    if receiver_calls.get() != 1 {
        return Err("replacement Rust method was not invoked exactly once".into());
    }
    device.release()?;
    Ok(())
}

fn drive_request(
    vm: &mut Vm,
    request: QueuedCallRequest,
) -> Result<QueuedCallResult, Box<dyn std::error::Error>> {
    let mut request = Box::pin(request);
    let mut context = TaskContext::from_waker(Waker::noop());
    for _ in 0..MAX_PUMP_TURNS {
        if let Poll::Ready(result) = request.as_mut().poll(&mut context) {
            return result.map_err(host_error);
        }
        vm.poll_host_futures(&mut context)?;
        vm.run_host_commands()?;
        vm.run_jobs()?;
    }
    Err("async Device.refresh did not settle".into())
}

fn release_descriptor(descriptor: PropertyDescriptor) -> Result<(), velum::Error> {
    match descriptor {
        PropertyDescriptor::Data { value, .. } => value.release(),
        PropertyDescriptor::Accessor { getter, setter, .. } => {
            if let Some(getter) = getter {
                getter.release()?;
            }
            if let Some(setter) = setter {
                setter.release()?;
            }
            Ok(())
        }
    }
}

fn host_error(error: HostFutureError) -> Box<dyn std::error::Error> {
    Box::new(error)
}
