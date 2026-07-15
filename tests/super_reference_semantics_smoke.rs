use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn object_methods_use_the_home_object_and_actual_receiver() -> TestResult {
    expect_true(
        r#"
        var proto = {
            get value() { return this.own; },
            method() { return this.own + 1; }
        };
        var object = {
            __proto__: proto,
            own: 7,
            read() { return super.value; },
            call() { return super.method(); },
            computed(key) { return super[key]; }
        };
        object.read() === 7 && object.call() === 8 && object.computed("value") === 7
        "#,
    )
}

#[test]
fn super_writes_updates_and_compound_assignments_target_the_receiver() -> TestResult {
    expect_true(
        r"
        var proto = { count: 1 };
        var object = {
            __proto__: proto,
            update() {
                super.count = 4;
                super.count += 2;
                return ++super.count;
            }
        };
        object.update() === 2 && object.count === 2 && proto.count === 1
        ",
    )
}

#[test]
fn super_assignment_evaluates_the_rhs_before_rejecting_a_null_base() -> TestResult {
    expect_true(
        r#"
        var count = 0;
        class Camera {
            static assign() { super.value = count += 1; }
            static computed() { super["value"] = count += 1; }
        }
        Object.setPrototypeOf(Camera, null);
        try { Camera.assign(); } catch (error) {}
        try { Camera.computed(); } catch (error) {}
        count === 2
        "#,
    )
}

#[test]
fn direct_eval_inherits_the_active_super_context() -> TestResult {
    expect_true(
        r#"
        var proto = { value: 11 };
        var object = {
            __proto__: proto,
            method() { return eval("super.value"); }
        };
        object.method() === 11
        "#,
    )
}

#[test]
fn derived_constructors_bind_the_object_returned_by_super() -> TestResult {
    expect_true(
        r"
        class Base { constructor() { return { value: 12 }; } }
        class Derived extends Base {
            constructor() {
                super();
                this.value += 1;
            }
        }
        new Derived().value === 13
        ",
    )
}

#[test]
fn derived_this_is_uninitialized_before_super() -> TestResult {
    expect_true(
        r"
        var caught = false;
        class Base {}
        class Derived extends Base {
            constructor() {
                try { this.value; } catch (error) { caught = error.constructor === ReferenceError; }
                super();
            }
        }
        new Derived();
        caught
        ",
    )
}

#[test]
fn super_constructor_is_resolved_after_argument_evaluation() -> TestResult {
    expect_true(
        r"
        var evaluated = false;
        var caught;
        class Derived extends Object {
            constructor() {
                try { super(evaluated = true); } catch (error) { caught = error; }
            }
        }
        Object.setPrototypeOf(Derived, parseInt);
        try { new Derived(); } catch (error) {}
        evaluated && caught.constructor === TypeError
        ",
    )
}

#[test]
fn super_constructor_reference_is_prepared_before_arguments() -> TestResult {
    expect_true(
        r"
        class Base { constructor(value) { this.value = value; } }
        class Derived extends Base {
            constructor() {
                super((Object.setPrototypeOf(Derived, null), 42));
            }
        }
        new Derived().value === 42
        ",
    )
}

#[test]
fn constructor_arrows_resolve_super_properties_from_the_instance_prototype() -> TestResult {
    expect_true(
        r"
        var calls = 0;
        class Base {
            method() { calls += 1; }
        }
        class Derived extends Base {
            constructor() {
                super();
                (() => super.method())();
            }
        }
        new Derived();
        calls === 1
        ",
    )
}

#[test]
fn derived_native_construction_uses_the_new_target_prototype() -> TestResult {
    expect_true(
        r"
        class DerivedArray extends Array {}
        class DerivedFunction extends Function {}
        var array = new DerivedArray();
        var callable = new DerivedFunction();
        array instanceof DerivedArray && array instanceof Array &&
            callable instanceof DerivedFunction && callable instanceof Function
        ",
    )
}

#[test]
fn deleting_computed_super_properties_evaluates_the_key_before_throwing() -> TestResult {
    expect_true(
        r#"
        var keyEvaluations = 0;
        var object = {
            method() {
                try {
                    delete super[(keyEvaluations += 1, "value")];
                } catch (error) {
                    return error instanceof ReferenceError && keyEvaluations === 1;
                }
                return false;
            }
        };
        Object.setPrototypeOf(object, null);
        object.method()
        "#,
    )
}

#[test]
fn deleting_super_properties_checks_this_before_the_computed_key() -> TestResult {
    expect_true(
        r"
        var baseCalls = 0;
        class Base { constructor() { baseCalls = baseCalls + 1; } }
        class Derived extends Base {
            constructor() {
                delete super[(super(), 0)];
            }
        }
        try {
            new Derived();
        } catch (error) {
            error instanceof ReferenceError && baseCalls === 0
        }
        ",
    )
}

#[test]
fn destructuring_and_for_of_assign_through_super_references() -> TestResult {
    expect_true(
        r#"
        var writes = [];
        var base = {
            set first(value) { writes.push("first:" + value); },
            set second(value) { writes.push("second:" + value); }
        };
        var object = {
            __proto__: base,
            assign() {
                [super.first, super["second"]] = [1, 2];
                for (super.first of [3, 4]) {}
            }
        };
        object.assign();
        writes.join(",") === "first:1,second:2,first:3,first:4"
        "#,
    )
}

#[test]
fn class_heritage_accepts_new_expressions() -> TestResult {
    expect_true(
        r"
        class Base {}
        class Derived extends new Proxy(Base, {}) {}
        new Derived() instanceof Base
        ",
    )
}

fn expect_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}
