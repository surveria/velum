use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    ensure_value(&eval(source)?, &Value::from(expected))
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

#[test]
fn exposes_indexed_arguments_and_length() -> TestResult {
    ensure_string(
        r#"
        function probe(a, b) {
            return arguments.length + ":" + arguments[0] + ":" + arguments[2]
                + ":" + (arguments[9] === undefined);
        }
        probe(10, 20, 30)
        "#,
        "3:10:30:true",
    )
}

#[test]
fn keeps_arguments_on_the_object_prototype_and_outside_is_array() -> TestResult {
    ensure_string(
        r#"
        function probe() {
            return (Object.getPrototypeOf(arguments) === Object.prototype)
                + ":" + Array.isArray(arguments)
                + ":" + String.prototype.trim.call(arguments);
        }
        probe(1, 2, true)
        "#,
        "true:false:[object Arguments]",
    )
}

#[test]
fn reflects_actual_call_arity() -> TestResult {
    ensure_string(
        r#"
        function count() {
            return arguments.length;
        }
        "" + count() + count(1) + count(1, 2, 3)
        "#,
        "013",
    )
}

#[test]
fn indexed_writes_do_not_alias_parameters() -> TestResult {
    ensure_string(
        r#"
        function unmapped(a) {
            arguments[0] = 99;
            return a + ":" + arguments[0];
        }
        unmapped(1)
        "#,
        "1:99",
    )
}

#[test]
fn parameters_and_vars_named_arguments_shadow_the_object() -> TestResult {
    ensure_string(
        r#"
        function byParam(arguments) {
            return arguments;
        }
        function byVar() {
            var arguments = "var";
            return arguments;
        }
        byParam("param") + ":" + byVar(1, 2)
        "#,
        "param:var",
    )
}

#[test]
fn arrow_functions_do_not_bind_their_own_arguments() -> TestResult {
    ensure_string(
        r"
        var probe = () => typeof arguments;
        function host() {
            return probe();
        }
        host(1, 2)
        ",
        "undefined",
    )
}

#[test]
fn escaped_arrows_capture_the_parent_arguments_binding() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        function ordinary(value) {
            return () => arguments[0];
        }
        const holder = {
            async method(value) {
                return async () => arguments[0];
            }
        };
        class Cls {
            static async method(value) {
                return async () => arguments[0];
            }
        }
        let trace = ordinary(40)();
        Promise.all([holder.method(1), Cls.method(2)]).then(function(functions) {
            return Promise.all([functions[0](), functions[1]()]);
        }).then(function(values) {
            trace = trace + ":" + values[0] + ":" + values[1];
        });
        "#,
    )?;
    ensure_value(&context.eval("trace")?, &Value::from("40:1:2"))
}

#[test]
fn arguments_helpers_preserve_cross_script_closure_bindings() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        function probe(object, name) {
            if (arguments.length < 2) {
                return false;
            }
            object[name] = "ignored";
            return object[name];
        }
        "#,
    )?;
    let value = context.eval(
        r#"
        var captured = "data";
        var object = {};
        Object.defineProperty(object, "value", {
            get: function() { return captured; }
        });
        captured + ":" + probe(object, "value");
        "#,
    )?;
    ensure_value(&value, &Value::from("data:data"))
}

#[test]
fn strict_functions_and_methods_bind_arguments() -> TestResult {
    ensure_string(
        r#"
        function strictProbe() {
            "use strict";
            return arguments.length;
        }
        const holder = {
            m() {
                return arguments.length;
            }
        };
        class Cls {
            m() {
                return arguments.length;
            }
        }
        "" + strictProbe(1, 2) + holder.m(1) + new Cls().m(7, 8, 9)
        "#,
        "213",
    )
}

#[test]
fn arguments_iterate_and_spread() -> TestResult {
    ensure_string(
        r#"
        function total() {
            let sum = 0;
            for (const value of arguments) {
                sum = sum + value;
            }
            return sum + ":" + Math.max(...arguments);
        }
        total(3, 9, 4)
        "#,
        "16:9",
    )
}

#[test]
fn defaults_and_rest_see_the_full_argument_list() -> TestResult {
    ensure_string(
        r#"
        function withDefault(a = arguments.length) {
            return a;
        }
        function withRest(first, ...rest) {
            return arguments.length + ":" + rest.length;
        }
        "" + withDefault() + ":" + withDefault(5) + ":" + withRest(1, 2, 3)
        "#,
        "0:5:3:2",
    )
}

#[test]
fn functions_without_arguments_references_skip_the_binding() -> TestResult {
    ensure_string(
        r#"
        function plain(a, b) {
            return a + b;
        }
        "" + plain(40, 2)
        "#,
        "42",
    )
}

#[test]
fn nested_functions_own_their_arguments_bindings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let baseline = vm.compile(
        r"
        function outer(value) {
            function inner() {
                return 0;
            }
            return value + inner();
        }
        outer(1);
        ",
    )?;
    let referenced = vm.compile(
        r"
        function outer(value) {
            function inner() {
                return arguments.length;
            }
            return value + inner();
        }
        outer(1);
        ",
    )?;
    let expected = baseline
        .usage()
        .static_binding_count()
        .checked_add(2)
        .ok_or("expected binding count overflowed")?;
    ensure_usize(referenced.usage().static_binding_count(), expected)?;
    ensure_value(&vm.eval_compiled(&referenced)?, &Value::Number(1.0))
}

#[test]
fn arrows_charge_arguments_to_the_nearest_ordinary_function() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let baseline = vm.compile(
        r"
        function outer(value) {
            const inner = () => 0;
            return value + inner();
        }
        outer(1);
        ",
    )?;
    let referenced = vm.compile(
        r"
        function outer(value) {
            const inner = () => arguments.length;
            return value + inner();
        }
        outer(1);
        ",
    )?;
    let expected = baseline
        .usage()
        .static_binding_count()
        .checked_add(2)
        .ok_or("expected binding count overflowed")?;
    ensure_usize(referenced.usage().static_binding_count(), expected)?;
    ensure_value(&vm.eval_compiled(&referenced)?, &Value::Number(2.0))
}

#[test]
fn distinguishes_mapped_and_unmapped_callee_properties() -> TestResult {
    ensure_string(
        r#"
        function sloppy() {
            var original = arguments.callee === sloppy;
            arguments.callee = 7;
            var assigned = arguments.callee === 7;
            var deleted = delete arguments.callee;
            return original && assigned && deleted && arguments.callee === undefined;
        }
        function strict() {
            "use strict";
            return Object.getOwnPropertyDescriptor(arguments, "callee");
        }
        function nonSimple(value = 0) {
            return Object.getOwnPropertyDescriptor(arguments, "callee");
        }
        var strictDescriptor = strict();
        var nonSimpleDescriptor = nonSimple();
        sloppy() + ":" +
            (strictDescriptor.get === strictDescriptor.set) + ":" +
            (strictDescriptor.get === nonSimpleDescriptor.get);
        "#,
        "true:true:true",
    )
}
