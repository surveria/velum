use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn expect_string(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if actual == Value::from(expected) {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

#[test]
fn infers_names_for_declarations_and_simple_assignments() -> TestResult {
    expect_string(
        r#"
        let declared = () => {};
        let assigned;
        assigned = function() {};
        let Klass = class {};
        let parenthesized = (function() {});
        declared.name + "|" + assigned.name + "|" + Klass.name + "|"
            + parenthesized.name
        "#,
        "declared|assigned|Klass|parenthesized",
    )
}

#[test]
fn leaves_non_identifier_assignment_names_empty() -> TestResult {
    expect_string(
        r#"
        let covered;
        let object = {};
        (covered) = function() {};
        object.value = () => {};
        covered.name + "|" + object.value.name
        "#,
        "|",
    )
}

#[test]
fn infers_names_for_parameter_and_destructuring_defaults() -> TestResult {
    expect_string(
        r#"
        function plain(callback = () => {}) {
            return callback.name;
        }
        let [element = function() {}] = [];
        let {value: property = class {}} = {};
        plain() + "|" + element.name + "|" + property.name
        "#,
        "callback|element|property",
    )
}

#[test]
fn infers_names_for_logical_assignment_values() -> TestResult {
    expect_string(
        r#"
        let andValue = true;
        let orValue = false;
        let nullishValue = null;
        andValue &&= () => {};
        orValue ||= function() {};
        nullishValue ??= class {};
        andValue.name + "|" + orValue.name + "|" + nullishValue.name
        "#,
        "andValue|orValue|nullishValue",
    )
}

#[test]
fn names_object_methods_accessors_and_computed_definitions() -> TestResult {
    expect_string(
        r#"
        let key = "computed";
        let named = Symbol("named");
        let anonymous = Symbol();
        let object = {
            staticValue: () => {},
            [key]: function() {},
            [named]() {},
            get value() {},
            set value(next) {},
            get [anonymous]() {}
        };
        let valueDescriptor = Object.getOwnPropertyDescriptor(object, "value");
        let anonymousDescriptor =
            Object.getOwnPropertyDescriptor(object, anonymous);
        object.staticValue.name + "|" + object[key].name + "|"
            + object[named].name + "|" + valueDescriptor.get.name + "|"
            + valueDescriptor.set.name + "|" + anonymousDescriptor.get.name
        "#,
        "staticValue|computed|[named]|get value|set value|get ",
    )
}

#[test]
fn names_class_methods_and_accessors_from_resolved_keys() -> TestResult {
    expect_string(
        r#"
        let named = Symbol("named");
        let anonymous = Symbol();
        class Example {
            method() {}
            [named]() {}
            get value() {}
            set value(next) {}
            static get [anonymous]() {}
        }
        let valueDescriptor =
            Object.getOwnPropertyDescriptor(Example.prototype, "value");
        let anonymousDescriptor =
            Object.getOwnPropertyDescriptor(Example, anonymous);
        Example.prototype.method.name + "|" + Example.prototype[named].name
            + "|" + valueDescriptor.get.name + "|" + valueDescriptor.set.name
            + "|" + anonymousDescriptor.get.name
        "#,
        "method|[named]|get value|set value|get ",
    )
}

#[test]
fn infers_names_for_computed_class_field_initializers() -> TestResult {
    expect_string(
        r#"
        let arrow = "arrow";
        class Example {
            [arrow] = () => {};
            [5] = async () => {};
            static ["factory"] = function () {};
        }
        let instance = new Example();
        instance.arrow.name + "|" + instance[5].name + "|" + Example.factory.name
        "#,
        "arrow|5|factory",
    )
}

#[test]
fn named_async_functions_keep_their_private_self_binding() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let observed = "missing";
        let reference = async function BindingIdentifier() {
            BindingIdentifier = 1;
            return BindingIdentifier;
        };
        async function observe() {
            let value = await reference();
            observed = value === reference ? "same" : "changed";
        }
        observe();
        "#,
    )?;
    let actual = context.eval("observed")?;
    if actual == Value::from("same") {
        return Ok(());
    }
    Err(format!("expected async self binding to resolve itself, got {actual:?}").into())
}

#[test]
fn named_async_generators_keep_captured_private_self_binding() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let observed = "missing";
        let callCount = 0;
        let reference = async function * BindingIdentifier() {
            callCount++;
            (() => {
                BindingIdentifier = 1;
            })();
            return BindingIdentifier;
        };
        async function observe() {
            let generator = await reference();
            let result = await generator.next();
            observed = result.value === reference
                ? "same|" + callCount
                : "changed|" + callCount;
        }
        observe();
        "#,
    )?;
    let actual = context.eval("observed")?;
    if actual == Value::from("same|1") {
        return Ok(());
    }
    Err(format!("expected async generator self binding to resolve itself, got {actual:?}").into())
}

#[test]
fn named_async_generator_completes_test262_async_wrapper() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let Test262Error = function Test262Error(message) {
            this.message = message || "";
        };
        let assert = function assert(condition) {
            if (condition !== true) {
                throw new Test262Error("Expected true");
            }
        };
        assert.sameValue = function (actual, expected, message) {
            if (actual === expected) {
                return;
            }
            throw new Test262Error(message || "Expected SameValue");
        };
        function $DONE(error) {
            print(error ? "failed:" + error.message : "complete");
        }
        function asyncTest(callback) {
            if (!Object.prototype.hasOwnProperty.call(globalThis, "$DONE")) {
                throw new Test262Error("asyncTest called without async flag");
            }
            callback().then(() => $DONE(), (error) => $DONE(error));
        }
        "#,
    )?;
    context.eval(
        r#"
        let callCount = 0;
        let ref = async function * BindingIdentifier() {
            callCount++;
            BindingIdentifier = 1;
            return BindingIdentifier;
        };
        asyncTest(async () => {
            assert.sameValue((await (await ref()).next()).value, ref);
            assert.sameValue(callCount, 1, "function invoked exactly once");
        });
        "#,
    )?;
    let output = context.take_output();
    if output == ["complete"] {
        return Ok(());
    }
    Err(format!("expected async wrapper completion, got {output:?}").into())
}
