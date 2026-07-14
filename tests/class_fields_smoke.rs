use rs_quickjs::{Runtime, Value};

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

fn ensure_error_contains(source: &str, expected: &str) -> TestResult {
    let Err(error) = eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    let message = error.to_string();
    if message.contains(expected) {
        return Ok(());
    }
    Err(format!("expected error '{message}' to contain '{expected}'").into())
}

#[test]
fn initializes_instance_fields_in_order() -> TestResult {
    ensure_string(
        r#"
        class Point {
            x = 1;
            y = this.x + 1;
            bare;
        }
        const p = new Point();
        "" + p.x + ":" + p.y + ":" + (p.bare === undefined)
        "#,
        "1:2:true",
    )
}

#[test]
fn initializes_static_fields_at_class_creation() -> TestResult {
    ensure_string(
        r#"
        class Registry {
            static count = 40 + 2;
            static label = "reg";
            static kind = typeof this;
        }
        "" + Registry.count + ":" + Registry.label + ":" + Registry.kind
        "#,
        "42:reg:function",
    )
}

#[test]
fn executes_class_static_blocks_once_with_scoped_bindings() -> TestResult {
    ensure_string(
        r#"
        let calls = 0;
        class Registry {
            static {
                let local = 40;
                calls = calls + 1;
                this.value = local + 2;
            }
        }
        "" + Registry.value + ":" + calls + ":" + typeof local
        "#,
        "42:1:undefined",
    )
}

#[test]
fn isolates_var_bindings_between_class_static_blocks() -> TestResult {
    ensure_string(
        r#"
        var value = "outer";
        var first;
        var second;
        class Registry {
            static {
                var value = "first block";
                first = value;
            }
            static {
                second = value;
                var value = "second block";
            }
        }
        value + ":" + first + ":" + second
        "#,
        "outer:first block:undefined",
    )
}

#[test]
fn interleaves_static_fields_and_blocks_in_source_order() -> TestResult {
    ensure_string(
        r#"
        let sequence = "";
        class Ordered {
            static first = (sequence = sequence + "f1");
            static { sequence = sequence + "b1"; }
            static second = (sequence = sequence + "f2");
            static { sequence = sequence + "b2"; }
        }
        sequence
        "#,
        "f1b1f2b2",
    )
}

#[test]
fn rejects_class_static_block_early_errors() -> TestResult {
    for source in [
        "class Sample { static { await; } }",
        "class Sample { static { arguments; } }",
        "while (false) { class Sample { static { break; } } }",
        "class Sample { static { let value; var value; } }",
    ] {
        ensure_error_contains(source, "parser error")?;
    }
    Ok(())
}

#[test]
fn supports_computed_string_and_numeric_field_keys() -> TestResult {
    ensure_string(
        r#"
        const suffix = "puted";
        class Keys {
            ["com" + suffix] = "c";
            "quoted" = "q";
            42 = "n";
        }
        const k = new Keys();
        k.computed + ":" + k.quoted + ":" + k[42]
        "#,
        "c:q:n",
    )
}

#[test]
fn derived_fields_initialize_after_parent_fields() -> TestResult {
    ensure_string(
        r#"
        class Base {
            v = "base";
        }
        class Derived extends Base {
            w = this.v + "+derived";
        }
        class Third extends Derived {
            z = this.w + "+third";
        }
        new Third().z
        "#,
        "base+derived+third",
    )
}

#[test]
fn fields_are_visible_to_constructors_and_methods() -> TestResult {
    ensure_string(
        r#"
        class Mixed {
            f = 1;
            constructor() {
                this.g = this.f + 1;
            }
            sum() {
                return this.f + this.g;
            }
        }
        "" + new Mixed().sum()
        "#,
        "3",
    )
}

#[test]
fn arrow_function_fields_capture_this() -> TestResult {
    ensure_string(
        r#"
        class Handler {
            tag = "captured";
            read = () => this.tag;
        }
        new Handler().read()
        "#,
        "captured",
    )
}

#[test]
fn field_initializers_capture_the_inner_class_name() -> TestResult {
    ensure_string(
        r#"
        let Direct = class InnerDirect {
            value = InnerDirect;
            deferred = () => InnerDirect;
        };
        const direct = new Direct();
        const result = direct.value === Direct && direct.deferred() === Direct;
        Direct = null;
        "" + (result && direct.deferred() === direct.value)
        "#,
        "true",
    )
}

#[test]
fn field_initializer_eval_observes_the_inner_class_name() -> TestResult {
    ensure_string(
        r#"
        class Direct {
            value = eval("Direct");
            deferred = () => eval("Direct");
            static value = eval("Direct");
            static deferred = () => eval("Direct");
        }
        let Expression = class Inner {
            value = eval("Inner");
            deferred = () => eval("Inner");
            static value = eval("Inner");
            static deferred = () => eval("Inner");
        };
        const Renamed = Expression;
        Expression = null;
        const DirectAlias = Direct;
        Direct = null;
        const direct = new DirectAlias();
        const expression = new Renamed();
        "" + (
            direct.value === DirectAlias && direct.deferred() === DirectAlias &&
            DirectAlias.value === DirectAlias && DirectAlias.deferred() === DirectAlias &&
            expression.value === Renamed && expression.deferred() === Renamed &&
            Renamed.value === Renamed && Renamed.deferred() === Renamed
        )
        "#,
        "true",
    )
}

#[test]
fn block_scoped_instance_field_eval_retains_the_class_environment() -> TestResult {
    ensure_string(
        r#"
        let valid = true;
        {
            class C { field = eval("C"); }
            valid = valid && new C().field === C;
        }
        {
            let C = class Inner { field = eval("Inner"); };
            valid = valid && new C().field === C;
        }
        {
            class C { field = () => eval("C"); }
            valid = valid && new C().field() === C;
            const D = C;
            C = null;
            valid = valid && new D().field() === D;
        }
        {
            let C = class Inner { field = () => eval("Inner"); };
            valid = valid && new C().field() === C;
            const D = C;
            C = null;
            valid = valid && new D().field() === D;
        }
        "" + valid;
        "#,
        "true",
    )
}

#[test]
fn fields_are_enumerable_own_properties() -> TestResult {
    ensure_string(
        r#"
        class Shape {
            a = 1;
            method() {}
            static s = 2;
        }
        const shape = new Shape();
        let seen = "";
        for (const key in shape) {
            seen = seen + key;
        }
        seen + ":" + Object.keys(shape).length
        "#,
        "a:1",
    )
}

#[test]
fn field_initializers_run_per_instance() -> TestResult {
    ensure_string(
        r#"
        let counter = 0;
        function next() {
            counter = counter + 1;
            return counter;
        }
        class Counted {
            id = next();
        }
        "" + new Counted().id + new Counted().id + new Counted().id + ":" + counter
        "#,
        "123:3",
    )
}

#[test]
fn keywords_and_contextual_names_work_as_field_names() -> TestResult {
    ensure_string(
        r#"
        class Named {
            static = 1;
            get = 2;
            set = 3;
        }
        const n = new Named();
        "" + n.static + n.get + n.set
        "#,
        "123",
    )
}

#[test]
fn rejects_invalid_field_names() -> TestResult {
    ensure_error_contains(
        "class Bad { constructor = 1 }",
        "class field cannot be named 'constructor'",
    )?;
    ensure_error_contains(
        "class Bad2 { static prototype = 1 }",
        "class static member cannot be named 'prototype'",
    )
}
