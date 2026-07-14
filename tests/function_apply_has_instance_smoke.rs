use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn apply_forwards_this_and_array_like_arguments() -> TestResult {
    eval_is_42(
        r"
        function sum() {
            var total = 0;
            for (var i = 0; i < arguments.length; i++) {
                total += arguments[i];
            }
            return total;
        }
        sum.apply(null, [1, 2, 3]) === 6 &&
            sum.apply(null) === 0 &&
            sum.apply(null, null) === 0 &&
            sum.apply(null, undefined) === 0 &&
            sum.apply(null, { length: 3, 0: 10, 1: 20, 2: 30 }) === 60 &&
            (function () { return this.marker; }).apply({ marker: 7 }) === 7
            ? 42
            : 0
        ",
    )
}

#[test]
fn apply_rejects_bad_receivers_and_arguments() -> TestResult {
    eval_is_42(
        r"
        function f() { return 0; }
        var count = 0;
        try { f.apply(null, 5); } catch (e) { if (e instanceof TypeError) count += 1; }
        try { f.apply(null, true); } catch (e) { if (e instanceof TypeError) count += 1; }
        try { Function.prototype.apply.call(undefined, null, []); } catch (e) { if (e instanceof TypeError) count += 1; }
        try { Function.prototype.apply.call({}, null, []); } catch (e) { if (e instanceof TypeError) count += 1; }
        count === 4 ? 42 : 0
        ",
    )
}

#[test]
fn apply_composes_with_bound_functions() -> TestResult {
    eval_is_42(
        r"
        function add(a, b, c) { return a + b + c; }
        var bound = add.bind(null, 1, 2);
        bound.apply(null, [3]) === 6 ? 42 : 0
        ",
    )
}

#[test]
fn apply_preserves_observable_array_like_access() -> TestResult {
    eval_is_42(
        r#"
        function collect(a, b, c) { return a * 100 + b * 10 + c; }
        var getterCalls = 0;
        var accessorArray = [1, 0, 3];
        Object.defineProperty(accessorArray, "1", {
            get: function () { getterCalls += 1; return 2; },
            configurable: true
        });
        var inheritedArray = [4, 5, 6];
        delete inheritedArray[1];
        Array.prototype[1] = 2;
        var accessorResult = collect.apply(null, accessorArray);
        var inheritedResult = collect.apply(null, inheritedArray);
        delete Array.prototype[1];
        accessorResult === 123 &&
            inheritedResult === 426 &&
            getterCalls === 1
            ? 42
            : 0
        "#,
    )
}

#[test]
fn has_instance_matches_prototype_chain() -> TestResult {
    eval_is_42(
        r"
        function Animal() {}
        function Dog() {}
        Dog.prototype = Object.create(Animal.prototype);
        function Cat() {}
        var dog = new Dog();
        var hasInstance = Function.prototype[Symbol.hasInstance];
        dog instanceof Dog &&
            dog instanceof Animal &&
            (dog instanceof Cat) === false &&
            hasInstance.call(Dog, dog) === true &&
            hasInstance.call(Animal, dog) === true &&
            hasInstance.call(Cat, dog) === false
            ? 42
            : 0
        ",
    )
}

#[test]
fn has_instance_recursively_unwraps_bound_functions() -> TestResult {
    eval_is_42(
        r"
        function Constructor() {}
        let instance = new Constructor();
        let bound = Constructor.bind(null);
        let doubleBound = bound.bind(null);
        let tripleBound = doubleBound.bind(null);
        instance instanceof bound &&
            instance instanceof doubleBound &&
            instance instanceof tripleBound &&
            Function.prototype[Symbol.hasInstance].call(tripleBound, instance)
            ? 42
            : 0
        ",
    )
}

#[test]
fn has_instance_returns_false_for_non_objects_and_non_callables() -> TestResult {
    eval_is_42(
        r#"
        function Dog() {}
        var dog = new Dog();
        var hasInstance = Function.prototype[Symbol.hasInstance];
        hasInstance.call(Dog, 42) === false &&
            hasInstance.call(Dog, "s") === false &&
            hasInstance.call(Dog, null) === false &&
            hasInstance.call(Dog, undefined) === false &&
            hasInstance.call(undefined, dog) === false &&
            hasInstance.call(42, dog) === false
            ? 42
            : 0
        "#,
    )
}

#[test]
fn exposes_method_metadata() -> TestResult {
    eval_is_42(
        r#"
        var hasInstance = Function.prototype[Symbol.hasInstance];
        Function.prototype.apply.length === 2 &&
            Function.prototype.apply.name === "apply" &&
            hasInstance.length === 1 &&
            hasInstance.name === "[Symbol.hasInstance]"
            ? 42
            : 0
        "#,
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
