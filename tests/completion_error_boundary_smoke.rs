use std::rc::Rc;

use parking_lot::Mutex;
use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn public_eval_exposes_the_original_thrown_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval("throw 42") else {
        return Err("expected an uncaught JavaScript value".into());
    };
    if error.javascript_value() == Some(&Value::Number(42.0))
        && error.javascript_identity() == Some(context.identity())
    {
        return Ok(());
    }
    Err(format!("expected thrown number 42, got {error:?}").into())
}

#[test]
fn native_runtime_frames_preserve_thrown_identity() -> TestResult {
    eval_is_42(
        r#"
        let objectMarker = { marker: "object" };
        let symbolMarker = Symbol("symbol");
        let score = 0;

        function observe(run, marker) {
            try {
                run();
            } catch (error) {
                if (error === marker) score = score + 1;
            }
        }

        observe(function () {
            ({ get value() { throw objectMarker; } }).value;
        }, objectMarker);
        observe(function () {
            [1].map(function () { throw objectMarker; });
        }, objectMarker);
        observe(function () {
            JSON.parse("1", function () { throw symbolMarker; });
        }, symbolMarker);
        observe(function () {
            eval("throw objectMarker");
        }, objectMarker);

        score === 4 ? 42 : score
        "#,
    )
}

#[test]
fn reference_errors_no_longer_depend_on_message_prefixes() -> TestResult {
    eval_is_42(
        r#"
        let score = 40;
        try {
            missingBinding;
        } catch (error) {
            if (error.name === "ReferenceError" &&
                error.message === "'missingBinding' is not defined") {
                score = score + 1;
            }
        }

        function readTdz(tdzValue = tdzValue) {}
        try {
            readTdz();
        } catch (error) {
            if (error.name === "ReferenceError" &&
                error.message === "'tdzValue' is not initialized") {
                score = score + 1;
            }
        }
        score
        "#,
    )
}

#[test]
fn host_functions_can_throw_an_explicit_javascript_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_function("hostThrow", |call| -> rs_quickjs::Result<Value> {
        Err(call.required_value(0, "value")?.javascript_error())
    })?;

    let value = context.eval(
        r#"
        let marker = { source: "host" };
        try {
            hostThrow(marker);
            0;
        } catch (error) {
            error === marker ? 42 : 1;
        }
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_foreign_host_errors_even_when_object_slots_collide() -> TestResult {
    let runtime = Runtime::new();
    let mut first_context = runtime.context();
    let captured = Rc::new(Mutex::new(None));
    let callback_capture = Rc::clone(&captured);
    first_context.register_host_function("captureError", move |call| {
        let error = call.required_value(0, "value")?.javascript_error();
        let mut slot = callback_capture.lock();
        if slot.is_some() {
            return Err(Error::runtime("captured error slot is already initialized"));
        }
        *slot = Some(error);
        drop(slot);
        Ok(Value::Undefined)
    })?;
    first_context.eval("captureError({ source: 'first' })")?;

    let foreign_error = captured
        .lock()
        .as_ref()
        .cloned()
        .ok_or("host callback did not capture a local JavaScript error")?;
    let Some(Value::Object(foreign_id)) = foreign_error.javascript_value() else {
        return Err("captured JavaScript error did not retain its object value".into());
    };

    let mut second_context = runtime.context();
    let mut has_local_collision = false;
    for _ in 0..64 {
        let Value::Object(local_id) =
            second_context.eval("var localMarker = { source: 'second' }; localMarker")?
        else {
            return Err("second VM did not create a local object marker".into());
        };
        if foreign_id == &local_id {
            has_local_collision = true;
            break;
        }
    }
    if !has_local_collision {
        return Err("test setup did not reach a colliding object slot".into());
    }
    second_context
        .register_host_function("throwForeign", move |_call| Err(foreign_error.clone()))?;

    let Err(error) = second_context
        .eval("try { throwForeign(); } catch (value) { value === localMarker ? 42 : 1; }")
    else {
        return Err("expected a foreign host JavaScript error to stay non-catchable".into());
    };
    if !matches!(error, Error::Runtime { .. })
        || !error
            .to_string()
            .contains("JavaScript thrown value belongs to another VM")
    {
        return Err(format!("expected a foreign-owner runtime error, got {error:?}").into());
    }
    Ok(())
}

#[test]
fn runtime_text_cannot_forge_a_catchable_reference_error() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_function("hostFailure", |_call| -> rs_quickjs::Result<Value> {
        Err(Error::runtime("ReferenceError: forged"))
    })?;

    let Err(error) = context.eval(
        r"
        try {
            hostFailure();
        } catch (error) {
            42;
        }
        ",
    ) else {
        return Err("expected a non-catchable host failure".into());
    };
    if matches!(error, Error::Runtime { .. }) {
        return Ok(());
    }
    Err(format!("expected runtime failure, got {error:?}").into())
}

#[test]
fn resource_limits_remain_outside_javascript_catch() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_function("hostLimit", |_call| -> rs_quickjs::Result<Value> {
        Err(Error::limit("host budget exhausted"))
    })?;

    for source in [
        "try { hostLimit(); } catch (error) { 42; }",
        "try { new Promise(hostLimit); } catch (error) { 42; }",
    ] {
        let Err(error) = context.eval(source) else {
            return Err("expected a non-catchable resource limit".into());
        };
        if !matches!(error, Error::ResourceLimit { .. }) {
            return Err(format!("expected resource limit, got {error:?}").into());
        }
    }
    Ok(())
}

#[test]
fn builtin_argument_validation_uses_catchable_error_classes() -> TestResult {
    eval_is_42(
        r#"
        let score = 0;
        for (const length of [-1, 1.5, NaN, Infinity, 4294967296]) {
            try {
                new Array(length);
            } catch (error) {
                if (error instanceof RangeError) score = score + 1;
            }
        }

        const maximum = new Array(4294967295);
        if (maximum.length === 4294967295) score = score + 1;

        const marker = Symbol("marker");
        try {
            Symbol(marker);
        } catch (error) {
            if (error instanceof TypeError) score = score + 1;
        }
        try {
            Symbol.for(marker);
        } catch (error) {
            if (error instanceof TypeError) score = score + 1;
        }

        score === 8 ? 42 : score
        "#,
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
