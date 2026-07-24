use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use velum::{
    DataPropertyDefinition, Engine, Error, JsValueRef, OwnedValue, PropertyDefinition,
    PropertyDescriptor, PropertyKeyRef, QueuedCallResult,
};
use velum_tokio::{RuntimeError, VmHandle, VmRuntime};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime.spawn_vm().await?;
    let before = create_device(&vm).await?;
    println!("Initial method result: {before}");
    let refreshed = refresh_device(&vm).await?;
    if (refreshed - 15.0).abs() > f64::EPSILON {
        return Err(format!("unexpected async refresh result: {refreshed}").into());
    }
    let receiver_calls = Arc::new(AtomicU32::new(0));
    let (after, owner, count) = replace_device_method(&vm, Arc::clone(&receiver_calls)).await?;
    if receiver_calls.load(Ordering::Relaxed) != 1 {
        return Err("replacement Rust method was not invoked exactly once".into());
    }
    println!("Replacement method result: {after}");
    println!("Fields controlled from Rust: owner={owner}, count={count}");
    Ok(())
}

async fn create_device(vm: &VmHandle) -> Result<String, RuntimeError> {
    let before = vm
        .run(|vm| {
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
                .ok_or_else(|| Error::runtime("Device was not defined"))?;
            let device = vm.construct_retained(&constructor, &[JsValueRef::String("sensor")])?;
            constructor.release()?;
            let before = vm.call_method_owned(
                (&device).into(),
                PropertyKeyRef::Name("describe"),
                &[JsValueRef::String("before")],
            )?;
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
            let global = vm.eval_retained("globalThis")?;
            vm.set_property_or_throw(
                (&global).into(),
                PropertyKeyRef::Name("device"),
                JsValueRef::Retained(&device),
            )?;
            global.release()?;
            device.release()?;
            Ok(format!("{before:?}"))
        })
        .await?;
    Ok(before)
}

async fn refresh_device(vm: &VmHandle) -> Result<f64, RuntimeError> {
    vm.run_local(|vm| {
        let device = vm.eval_retained("globalThis.device")?;
        let refresh =
            vm.get_property_retained((&device).into(), PropertyKeyRef::Name("refresh"))?;
        let request =
            vm.enqueue_call_with_receiver(&refresh, (&device).into(), &[JsValueRef::Number(5.0)])?;
        refresh.release()?;
        device.release()?;
        Ok(async move {
            match request.await.map_err(RuntimeError::from)? {
                QueuedCallResult::Owned(OwnedValue::Number(value)) => Ok(value),
                QueuedCallResult::Owned(other) => Err(RuntimeError::Engine(format!(
                    "Device.refresh returned {other:?}"
                ))),
                QueuedCallResult::Retained(value) => {
                    value.release().map_err(|error| {
                        RuntimeError::Engine(format!(
                            "unexpected refresh result could not be released: {error}"
                        ))
                    })?;
                    Err(RuntimeError::Engine(
                        "Device.refresh returned a VM-local value".to_owned(),
                    ))
                }
            }
        })
    })
    .await
}

async fn replace_device_method(
    vm: &VmHandle,
    captured_calls: Arc<AtomicU32>,
) -> Result<(String, String, String), RuntimeError> {
    vm.run(move |vm| {
        let device = vm.eval_retained("globalThis.device")?;
        let replacement = vm.create_host_function_typed("rustDescribe", move |call| {
            if call.receiver().as_value().type_name() != "object" {
                return Err(Error::runtime("replacement method lost its receiver"));
            }
            let prefix = call.string(0, "prefix")?;
            let previous = captured_calls
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .map_err(|_value| Error::limit("replacement call count overflowed"))?;
            let call_index = previous
                .checked_add(1)
                .ok_or_else(|| Error::limit("replacement call count overflowed"))?;
            Ok(format!("{prefix}:replacement-from-rust:{call_index}"))
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
        let descriptor = vm
            .get_own_property_descriptor((&device).into(), PropertyKeyRef::Name("owner"))?
            .ok_or_else(|| Error::runtime("owner descriptor was not defined"))?;
        release_descriptor(descriptor)?;
        device.release()?;
        Ok((
            format!("{after:?}"),
            format!("{owner:?}"),
            format!("{count:?}"),
        ))
    })
    .await
}

fn release_descriptor(descriptor: PropertyDescriptor) -> velum::Result<()> {
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
