use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn public_eval_exposes_the_original_thrown_value() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval("throw 42") else {
        return Err("expected an uncaught JavaScript value".into());
    };
    if error.javascript_value() == Some(&Value::Number(42.0)) {
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
        let value = call.required_value(0, "value")?.clone();
        Err(Error::javascript(value))
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
