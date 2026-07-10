use rs_quickjs::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn evaluates_instanceof_for_constructed_objects() -> TestResult {
    let value = eval(
        r"
        function Box(value) {
            this.value = value;
        }
        let object = new Box(7);
        object instanceof Box
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn evaluates_instanceof_for_function_objects() -> TestResult {
    let value = eval(
        r"
        function Box() {}
        Box instanceof Function && Object instanceof Function
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn evaluates_instanceof_for_current_error_objects() -> TestResult {
    let value = eval(
        r#"
        let typed = new TypeError("typed");
        let ranged = new RangeError("ranged");
        typed instanceof TypeError &&
            typed instanceof Error &&
            !(ranged instanceof TypeError) &&
            ranged instanceof Error
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn returns_false_for_primitive_left_operand() -> TestResult {
    let value = eval("1 instanceof Number")?;
    ensure_value(&value, &Value::Bool(false))
}

#[test]
fn supports_function_prototype_call_and_bind() -> TestResult {
    let value = eval(
        r#"
        function read(prefix, suffix) {
            return this.name + ":" + prefix + ":" + suffix;
        }
        let receiver = { name: "box" };
        let direct = read.call(receiver, "a", "b");
        let bound = read.bind(receiver, "a");
        direct + "|" + bound("b")
        "#,
    )?;
    ensure_value(&value, &Value::String("box:a:b|box:a:b".to_owned()))
}

#[test]
fn supports_property_helper_bound_object_prototype_methods() -> TestResult {
    let value = eval(
        r#"
        let hasOwnProperty = Function.prototype.call.bind(Object.prototype.hasOwnProperty);
        let propertyIsEnumerable =
            Function.prototype.call.bind(Object.prototype.propertyIsEnumerable);
        let propertyNames = Object.getOwnPropertyNames({
            value: 1,
            writable: true,
            enumerable: false,
            configurable: true
        }).join(",");
        let object = {};
        Object.defineProperty(object, "hidden", {
            value: 1,
            enumerable: false,
            configurable: true,
            writable: true
        });
        object.visible = 2;
        hasOwnProperty(object, "hidden") &&
            hasOwnProperty(object, "visible") &&
            !hasOwnProperty(object, "missing") &&
            !propertyIsEnumerable(object, "hidden") &&
            propertyIsEnumerable(object, "visible") &&
            propertyNames === "value,writable,enumerable,configurable"
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn exposes_async_function_to_string_tag_descriptor() -> TestResult {
    let value = eval(
        r"
        let AsyncFunction = async function foo() {}.constructor;
        let descriptor =
            Object.getOwnPropertyDescriptor(AsyncFunction.prototype, Symbol.toStringTag);
        descriptor.value === 'AsyncFunction' &&
            descriptor.writable === false &&
            descriptor.enumerable === false &&
            descriptor.configurable === true
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn rejects_non_callable_right_operand() -> TestResult {
    let Err(error) = eval("({}) instanceof ({})") else {
        return Err("expected non-callable right operand to fail".into());
    };
    ensure_error_kind(&error, "javascript")
}

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_error_kind(error: &Error, expected: &str) -> TestResult {
    let matches = matches!(
        (error, expected),
        (Error::Runtime { .. }, "runtime")
            | (Error::JavaScript { .. }, "javascript")
            | (Error::ResourceLimit { .. }, "resource limit")
    );
    if matches {
        return Ok(());
    }
    Err(format!("expected {expected} error, got {error:?}").into())
}
