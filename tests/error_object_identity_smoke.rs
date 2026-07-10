use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn error_instances_have_ordinary_object_identity() -> TestResult {
    eval_is_true(
        r#"
        let first = new TypeError("same");
        let second = new TypeError("same");
        let caught = undefined;
        try {
            throw first;
        } catch (value) {
            caught = value;
        }
        first !== second && caught === first &&
            Object.prototype.toString.call(first) === "[object Error]"
        "#,
    )
}

#[test]
fn error_properties_use_ordinary_descriptors_and_mutation() -> TestResult {
    eval_is_true(
        r#"
        let error = new RangeError("initial");
        let descriptor = Object.getOwnPropertyDescriptor(error, "message");
        let hadOwnName = Object.hasOwn(error, "name");
        error.message = "changed";
        error.extra = 42;
        let keys = Object.keys(error);
        let deleted = delete error.message;
        descriptor.value === "initial" &&
            descriptor.writable === true &&
            descriptor.enumerable === false &&
            descriptor.configurable === true &&
            hadOwnName === false &&
            keys.length === 1 && keys[0] === "extra" &&
            deleted === true && error.message === ""
        "#,
    )
}

#[test]
fn error_objects_share_the_object_prototype_and_integrity_paths() -> TestResult {
    eval_is_true(
        r#"
        let error = new SyntaxError("bad");
        let prototype = { marker: 7 };
        let changed = Object.setPrototypeOf(error, prototype) === error;
        let inherited = error.marker === 7;
        let defined = Object.defineProperty(error, "code", {
            value: 9,
            enumerable: true,
            writable: false,
            configurable: false
        }) === error;
        let frozen = Object.freeze(error) === error && Object.isFrozen(error);
        changed && inherited && defined && frozen && error.code === 9
        "#,
    )
}

#[test]
fn error_construction_honors_new_target_prototype() -> TestResult {
    eval_is_true(
        r#"
        function CustomErrorTarget() {}
        CustomErrorTarget.prototype = { marker: 42 };
        let error = Reflect.construct(TypeError, ["typed"], CustomErrorTarget);
        Object.getPrototypeOf(error) === CustomErrorTarget.prototype &&
            error.marker === 42 && error.message === "typed" &&
            Object.prototype.toString.call(error) === "[object Error]"
        "#,
    )
}

#[test]
fn public_errors_keep_typed_metadata_for_object_backed_instances() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval("missingErrorObject") else {
        return Err("expected a ReferenceError".into());
    };
    let Some(Value::Object(_)) = error.javascript_value() else {
        return Err(format!("expected an object-backed JavaScript error, got {error:?}").into());
    };
    ensure_metadata(
        &error,
        "ReferenceError",
        "'missingErrorObject' is not defined",
    )
}

fn eval_is_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}

fn ensure_metadata(error: &Error, name: &str, message: &str) -> TestResult {
    if error.javascript_error_name() == Some(name)
        && error.javascript_error_message() == Some(message)
    {
        return Ok(());
    }
    Err(format!("unexpected JavaScript error metadata: {error:?}").into())
}
