use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let actual = eval(source)?;
    if actual == Value::from(expected) {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

#[test]
fn evaluates_and_applies_decorators_in_defined_order() -> TestResult {
    ensure_string(
        r#"
        let log = "";
        function decorator(label) {
            log = log + "e" + label + ";";
            return function (value, context) {
                log = log + "a" + label + ":" + context.kind + ":" + context.name
                    + ":" + context.static + ":" + context.private + ";";
            };
        }
        @decorator("c1")
        @decorator("c2")
        class Example {
            @decorator("m1")
            @decorator("m2")
            method() { return 42; }
        }
        log + new Example().method()
        "#,
        "ec1;ec2;em1;em2;am2:method:method:false:false;am1:method:method:false:false;ac2:class:Example:undefined:undefined;ac1:class:Example:undefined:undefined;42",
    )
}

#[test]
fn decorator_replacements_flow_into_class_and_method_bindings() -> TestResult {
    ensure_string(
        r#"
        function replaceClass(value, context) {
            return function Replacement() { this.label = context.name; };
        }
        function replaceMethod(value, context) {
            return function () { return context.kind + ":" + context.name; };
        }
        @replaceClass
        class Example {}
        class Holder {
            @replaceMethod
            method() { return "old"; }
        }
        new Example().label + ":" + new Holder().method()
        "#,
        "Example:method:method",
    )
}

#[test]
fn field_decorator_initializers_transform_instance_and_static_values() -> TestResult {
    ensure_string(
        r#"
        function increment(value, context) {
            return function (initial) {
                return initial + (context.static ? 2 : 1);
            };
        }
        class Example {
            @increment
            value = 41;
            @increment
            static total = 40;
        }
        "" + new Example().value + ":" + Example.total
        "#,
        "42:42",
    )
}

#[test]
fn rejects_non_callable_decorator_replacements() -> TestResult {
    let Err(error) = eval(
        r"
        function invalid() { return 42; }
        @invalid class Example {}
        ",
    ) else {
        return Err("expected invalid decorator replacement to fail".into());
    };
    if error.to_string().contains("must return a callable value") {
        return Ok(());
    }
    Err(format!("unexpected decorator error: {error}").into())
}

#[test]
fn public_auto_accessors_use_hidden_storage_and_one_computed_key() -> TestResult {
    ensure_string(
        r#"
        let keyCalls = 0;
        function key() { keyCalls = keyCalls + 1; return "value"; }
        class Example {
            accessor [key()] = 41;
            static accessor total = 40;
        }
        const instance = new Example();
        const descriptor = Object.getOwnPropertyDescriptor(Example.prototype, "value");
        const staticDescriptor = Object.getOwnPropertyDescriptor(Example, "total");
        descriptor.set.call(instance, descriptor.get.call(instance) + 1);
        staticDescriptor.set.call(Example, staticDescriptor.get.call(Example) + 2);
        "" + keyCalls + ":" + instance.value + ":" + Example.total + ":"
            + Object.prototype.hasOwnProperty.call(instance, "value")
        "#,
        "1:42:42:false",
    )
}

#[test]
fn derived_auto_accessors_keep_independent_backing_slots() -> TestResult {
    ensure_string(
        r#"
        class Base {
            accessor value = 1;
            accessor inherited = 2;
        }
        class Derived extends Base {
            accessor value = 3;
        }
        const base = new Base();
        const derived = new Derived();
        derived.value = 4;
        "" + base.value + ":" + derived.value + ":" + derived.inherited
        "#,
        "1:4:2",
    )
}

#[test]
fn auto_accessor_descriptors_follow_class_element_source_order() -> TestResult {
    ensure_string(
        r#"
        class GetterWins {
            accessor value = 1;
            get value() { return 2; }
        }
        class AccessorWins {
            get value() { return 3; }
            accessor value = 4;
        }
        "" + new GetterWins().value + ":" + new AccessorWins().value
        "#,
        "2:4",
    )
}
