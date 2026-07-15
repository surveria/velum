use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_constructor_this_and_prototype_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let Camera = function Camera(name) {
            this.name = name;
            this.count = 40;
        };
        Camera.prototype.kind = "camera";
        Camera.prototype.read = function(delta) {
            return this.count + delta;
        };

        let front = new Camera("front");
        let side = new Camera("side");
        front.count = 41;

        let keys = "";
        for (let key in front) {
            keys = keys + key + ";";
        }

        print(front.name, side.name, front.kind, front.read(1), side.read(2));
        print("read" in front, "kind" in front, front.__proto__ === Camera.prototype);
        print(keys);

        front.name === "front" &&
            side.name === "side" &&
            front.kind === "camera" &&
            front.read(1) === 42 &&
            side.read(2) === 42 &&
            ("read" in front) &&
            ("kind" in front) &&
            front.__proto__ === Camera.prototype &&
            keys === "name;count;kind;read;" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "front side camera 42 42".to_owned(),
            "true true true".to_owned(),
            "name;count;kind;read;".to_owned(),
        ],
    )
}

#[test]
fn supports_constructor_return_object_and_primitive_rules() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let Replace = function Replace() {
            this.value = 1;
            return { value: 42 };
        };
        let Keep = function Keep() {
            this.value = 42;
            return 7;
        };

        let replaced = new Replace();
        let kept = new Keep();
        print(replaced.value, kept.value);

        replaced.value === 42 && kept.value === 42 ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(context.output(), &["42 42".to_owned()])
}

#[test]
fn supports_callable_constructor_prototypes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function CallablePrototype() {}
        CallablePrototype.kind = "callable";
        Object.defineProperty(CallablePrototype, "answer", {
            set: function(value) { this.stored = value; }
        });
        function Factory() {}
        Factory.prototype = CallablePrototype;

        let instance = new Factory();
        instance.answer = 42;
        let literal = { __proto__: CallablePrototype };

        Object.getPrototypeOf(instance) === CallablePrototype &&
            CallablePrototype.isPrototypeOf(instance) &&
            instance.kind === "callable" &&
            instance.stored === 42 &&
            typeof instance.call === "function" &&
            Object.getPrototypeOf(literal) === CallablePrototype &&
            literal.kind === "callable" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_new_on_non_function_bindings() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let Err(error) = context.eval("let Camera = {}; new Camera();") else {
        return Err("expected new on non-function binding to fail".into());
    };
    ensure_error_contains(&error, "not a constructor")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &velum::Error, expected: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }

    Err(format!("expected error containing '{expected}', got '{message}'").into())
}
