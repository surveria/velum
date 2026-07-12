use rs_quickjs::{Runtime, Value};

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

fn expect_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}
