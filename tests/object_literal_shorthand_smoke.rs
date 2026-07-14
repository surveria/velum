use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

#[test]
fn supports_object_literal_shorthand_and_methods() -> TestResult {
    let value = eval(
        r#"
        let name = "front-door";
        let count = 40;
        let camera = {
            name,
            count,
            default: 1,
            7: 2,
            add(extra) {
                return this.count + extra;
            },
            nested() {
                return this.add(this[7]);
            },
        };
        ("prototype" in camera.add) ? 0 : camera.nested()
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn validates_contextual_object_shorthand_identifiers() -> TestResult {
    let value = eval("var let = 42; ({ let }).let")?;
    ensure_value(&value, &Value::Number(42.0))?;

    let Err(error) = eval(
        r#"
        var implements = 1;
        (function() { "use strict"; ({ implements }); });
        "#,
    ) else {
        return Err("expected strict reserved shorthand to fail".into());
    };
    if error.to_string().contains("reserved word") {
        return Ok(());
    }
    Err(format!("expected strict shorthand parse error, got {error}").into())
}

#[test]
fn supports_computed_object_literal_property_names() -> TestResult {
    let value = eval(
        r#"
        let order = "";
        function mark(name, value) {
            order = order + name;
            return value;
        }
        let object = {
            [mark("k", "front")]: mark("v", 40),
            [mark("n", "door")]: mark("w", 2),
        };
        order === "kvnw" && object.front + object.door === 42 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn converts_computed_property_keys_before_evaluating_values() -> TestResult {
    let value = eval(
        r#"
        let name = "first";
        let key = { toString() { return name; } };
        let object = {
            [key]: (name = "second", 40),
            [key]: 2,
        };
        object.first + object.second
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn computed_proto_object_literal_property_is_data_property() -> TestResult {
    let value = eval(
        r#"
        let object = { ["__proto__"]: 42, marker: 1 };
        object.__proto__ === 42 && !("inherited" in object) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn proto_shorthand_is_data_and_duplicate_proto_setters_are_rejected() -> TestResult {
    let value = eval(
        r#"
        let __proto__ = 42;
        let object = { __proto__, __proto__ };
        Object.hasOwn(object, "__proto__") && object.__proto__
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))?;

    let Err(error) = eval("({ __proto__: null, '__proto__': null })") else {
        return Err("expected duplicate __proto__ setters to fail".into());
    };
    if error.to_string().contains("duplicate __proto__") {
        return Ok(());
    }
    Err(format!("expected duplicate __proto__ parse error, got {error}").into())
}

#[test]
fn object_literal_data_properties_replace_prior_accessor_descriptors() -> TestResult {
    let value = eval(
        r#"
        let object = {
            get slot() { return 1; },
            set slot(value) {},
            slot: 42
        };
        let descriptor = Object.getOwnPropertyDescriptor(object, "slot");
        descriptor.value === 42 && descriptor.writable && descriptor.enumerable &&
            descriptor.configurable && descriptor.get === undefined &&
            descriptor.set === undefined ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_symbol_object_literal_property_names() -> TestResult {
    let value = eval(
        r#"
        let key = Symbol("camera");
        let object = { [key]: 42 };
        object[key]
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn infers_anonymous_function_names_from_static_and_computed_property_keys() -> TestResult {
    let value = eval(
        r#"
        var described = Symbol("camera");
        var empty = Symbol("");
        var anonymous = Symbol();
        var object = {
            prop: function() {},
            "": function() {},
            5: function() {},
            [described]: function() {},
            [empty]: function() {},
            [anonymous]: function() {},
            [/a/]: function() {},
        };
        [
            object.prop.name,
            object[""].name,
            object[5].name,
            object[described].name,
            object[empty].name,
            object[anonymous].name,
            object[/a/].name,
        ].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("prop||5|[camera]|[]||/a/"))
}

#[test]
fn proto_setter_does_not_infer_an_anonymous_function_name() -> TestResult {
    let value = eval(
        r#"
        var object = { __proto__: function() {} };
        Object.getPrototypeOf(object).name + ":" + object.__proto__.name
        "#,
    )?;
    ensure_value(&value, &Value::from(":"))
}

#[test]
fn supports_computed_object_literal_methods() -> TestResult {
    let value = eval(
        r#"
        let order = "";
        function mark(name, value) {
            order = order + name;
            return value;
        }
        let object = {
            value: 40,
            [mark("k", "read")](extra) {
                order = order + "m";
                return this.value + extra;
            },
            after: mark("a", 1),
        };
        order === "ka" &&
            object.read(2) === 42 &&
            order === "kam" &&
            object.read.name === "read" &&
            !("prototype" in object.read) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn object_method_parameter_defaults_can_access_super() -> TestResult {
    let value = eval(
        r"
        let base = { value: 40 };
        let object = {
            answer(value = super.value) { return value + 2; }
        };
        Object.setPrototypeOf(object, base);
        object.answer()
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_symbol_object_literal_methods() -> TestResult {
    let value = eval(
        r#"
        let key = Symbol("camera");
        let object = {
            value: 40,
            [key](extra) {
                return this.value + extra;
            },
        };
        object[key](2)
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_async_object_literal_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r"
        let value = 0;
        let object = {
            base: 40,
            async answer(extra) {
                return await Promise.resolve(this.base + extra);
            },
        };
        object.answer(2).then(function(resolved) {
            value = resolved;
        });
        ",
    )?;

    let value = context.eval(
        r#"
        let AsyncFunction = async function() {}.constructor;
        value === 42 &&
            object.answer.name === "answer" &&
            object.answer.constructor === AsyncFunction &&
            !("prototype" in object.answer) ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_computed_async_object_literal_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let key = "answer";
        let value = 0;
        let object = {
            base: 40,
            async [key](extra) {
                return await Promise.resolve(this.base + extra);
            },
        };
        object.answer(2).then(function(resolved) {
            value = resolved;
        });
        "#,
    )?;

    let value = context.eval(
        r#"
        let AsyncFunction = async function() {}.constructor;
        value === 42 &&
            object.answer.name === "answer" &&
            object.answer.constructor === AsyncFunction ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn preserves_async_named_sync_object_literal_method() -> TestResult {
    let value = eval(
        r#"
        let object = {
            async() {
                return 42;
            },
        };
        object.async() === 42 &&
            object.async.name === "async" &&
            object.async.constructor === Function ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_async_object_method_default_parameter_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    context.eval(
        r#"
        let bodyRun = false;
        let rejected = "";
        function fail() {
            throw new TypeError("bad default");
        }
        let object = {
            async answer(value = fail()) {
                bodyRun = true;
                return value;
            },
        };
        object.answer().then(function() {
            rejected = "resolved";
        }, function(error) {
            rejected = error.name + ":" + error.message;
        });
        "#,
    )?;

    let value = context.eval(
        r#"
        !bodyRun && rejected === "TypeError:bad default" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_missing_shorthand_bindings() -> TestResult {
    let Err(error) = eval("let camera = { missing }; camera.missing") else {
        return Err("expected missing shorthand binding to fail".into());
    };
    let message = error.to_string();
    if message.contains("ReferenceError: 'missing' is not defined") {
        return Ok(());
    }
    Err(format!("expected ReferenceError, got '{message}'").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
