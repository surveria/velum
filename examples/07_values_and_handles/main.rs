use std::{cell::RefCell, rc::Rc};

use velum::{Engine, JsValueRef, OwnedValue, PropertyKeyRef, RetainedValue};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let engine = Engine::new();
    let mut first = engine.create_vm();
    let mut second = engine.create_vm();

    let portable = first.eval_owned(r#""portable between VMs""#)?;
    second.eval("function echo(value) { return value; }")?;
    let echo = second
        .get_global_retained("echo")?
        .ok_or("echo was not defined")?;
    let echoed = second.call_owned(&echo, &[JsValueRef::Owned(&portable)])?;
    echo.release()?;
    if echoed != portable {
        return Err(format!("portable value changed: {echoed:?}").into());
    }
    println!("OwnedValue crossed VM boundary: {echoed:?}");

    let captured = Rc::new(RefCell::new(None::<RetainedValue>));
    let captured_value = Rc::clone(&captured);
    first.register_host_function_typed("retainObject", move |call| {
        let object = call.required_value(0, "object")?.retain()?;
        captured_value.replace(Some(object));
        Ok(())
    })?;
    first.eval("retainObject({ answer: 42 });")?;
    first.collect_garbage()?;
    let object = captured
        .borrow_mut()
        .take()
        .ok_or("the callback did not retain its local object")?;
    let answer = first.get_property_owned(
        JsValueRef::Retained(&object),
        PropertyKeyRef::Name("answer"),
    )?;
    if answer != OwnedValue::Number(42.0) {
        return Err(format!("retained object changed: {answer:?}").into());
    }
    println!(
        "RetainedValue type={}, roots={}",
        first.retained_type_name(&object)?,
        first.root_snapshot()?.total()
    );

    tokio::task::yield_now().await;
    let foreign = second.get_property(
        JsValueRef::Retained(&object),
        PropertyKeyRef::Name("answer"),
    );
    if foreign.is_ok() {
        return Err("a VM accepted another VM's retained handle".into());
    }
    println!(
        "Cross-VM RetainedValue was rejected: {}",
        foreign.err().ok_or("missing error")?
    );
    object.release()?;
    Ok(())
}
