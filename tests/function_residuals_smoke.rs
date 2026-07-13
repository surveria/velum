use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn strict_functions_inherit_restricted_accessors() -> TestResult {
    assert_script(
        r#"
        "use strict";
        function target() {}
        var callerThrows = false;
        var argumentsThrows = false;
        try { target.caller; } catch (error) { callerThrows = error instanceof TypeError; }
        try { target.arguments; } catch (error) { argumentsThrows = error instanceof TypeError; }
        callerThrows && argumentsThrows ? 42 : 0
        "#,
    )
}

#[test]
fn bound_functions_derive_metadata_and_has_instance() -> TestResult {
    assert_script(
        r#"
        function Target(first, second, third) {}
        Object.defineProperty(Target, "name", { value: "TargetName" });
        var bound = Target.bind(null, 1);
        var instance = new Target();
        bound.length === 2 &&
            bound.name === "bound TargetName" &&
            bound[Symbol.hasInstance](instance) === true ? 42 : 0
        "#,
    )
}

#[test]
fn function_prototype_is_the_callable_function_root() -> TestResult {
    assert_script(
        r#"
        var descriptor = Object.getOwnPropertyDescriptor(Function.prototype, "length");
        typeof Function.prototype === "function" &&
            Function.prototype() === undefined &&
            Object.getPrototypeOf(Function.prototype) === Object.prototype &&
            Object.prototype.toString.call(Function.prototype) === "[object Function]" &&
            Function.prototype.name === "" &&
            descriptor.value === 0 && descriptor.writable === false ? 42 : 0
        "#,
    )
}

#[test]
fn apply_accepts_callable_array_like_values_and_dynamic_html_comments() -> TestResult {
    assert_script(
        r#"
        "use strict";
        function receiver() { return this === "value" && arguments.length === 1; }
        var applyWorks = receiver.apply("value", Array);
        var commentsWork = Function("<!--")() === undefined &&
            Function("\n-->")() === undefined &&
            Function("\n-->", "")() === undefined;
        var invalidParameterThrows = false;
        try { Function("-->", ""); } catch (error) {
            invalidParameterThrows = error instanceof SyntaxError;
        }
        applyWorks && commentsWork && invalidParameterThrows ? 42 : 0
        "#,
    )
}

#[test]
fn module_code_rejects_html_comments() -> TestResult {
    let runtime = Runtime::new();
    let result = runtime.compile_module_named("html-comment.js", "/*\n*/-->");
    if result.is_err() {
        return Ok(());
    }
    Err("expected module HTML close comment to be rejected".into())
}

fn assert_script(source: &str) -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected Number(42), got {value:?}").into())
}
