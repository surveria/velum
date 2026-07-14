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
fn parameter_expression_closures_keep_the_parameter_environment() -> TestResult {
    assert_script(
        r"
        function evaluate(value = 1, capture = () => value) {
            var value = 2;
            return capture() === 1 && value === 2;
        }
        evaluate() ? 42 : 0
        ",
    )
}

#[test]
fn annex_b_block_functions_preserve_parameter_bindings() -> TestResult {
    assert_script(
        r"
        var simpleBefore;
        var simpleAfter;
        (function (value) {
            simpleBefore = value;
            { function value() {} }
            simpleAfter = value;
        }(42));

        var defaultBefore;
        var defaultAfter;
        (function (value = 42) {
            defaultBefore = value;
            if (true) function value() {}
            defaultAfter = value;
        }());

        simpleBefore === 42 && simpleAfter === 42 &&
            defaultBefore === 42 && defaultAfter === 42 ? 42 : 0
        ",
    )
}

#[test]
fn direct_eval_in_a_function_body_uses_the_body_environment() -> TestResult {
    assert_script(
        r#"
        function bodyEval(value = 1) {
            eval("var local = 42");
            return local;
        }
        bodyEval() === 42 ? 42 : 0
        "#,
    )
}

#[test]
fn sloppy_parameter_eval_vars_are_visible_to_early_and_body_closures() -> TestResult {
    assert_script(
        r#"
        var x = "outside";
        var beforeEval;
        var afterEval;
        var fromBody;
        function evaluate(
            first = beforeEval = function() { return x; },
            second = (eval('var x = "inside";'), afterEval = function() { return x; })
        ) {
            fromBody = function() { return x; };
        }
        evaluate();
        beforeEval() === "inside" && afterEval() === "inside" &&
            fromBody() === "inside" && x === "outside" ? 42 : 0
        "#,
    )
}

#[test]
fn parameter_eval_var_environment_keeps_dynamic_scope_order() -> TestResult {
    assert_script(
        r#"
        var evalProbe;
        var innerWithProbe;
        var outerWith = { x: "outer-with" };
        with (outerWith) {
            (function(
                first = eval('var x = "eval";'),
                second = evalProbe = function() { return x; }
            ) {
                with ({ x: "inner-with" }) {
                    innerWithProbe = function() { return x; };
                }
            }());
        }
        evalProbe() === "eval" && innerWithProbe() === "inner-with" &&
            outerWith.x === "outer-with" ? 42 : 0
        "#,
    )
}

#[test]
fn parameter_eval_var_bindings_do_not_supply_an_implicit_this() -> TestResult {
    assert_script(
        r#"
        function evaluate(result = eval(`
            function strictReceiver() {
                "use strict";
                return this;
            }
            with ({}) {
                strictReceiver();
            }
        `)) {
            return result;
        }
        evaluate() === undefined ? 42 : 0
        "#,
    )
}

#[test]
fn parameter_eval_var_bindings_are_deletable() -> TestResult {
    assert_script(
        r#"
        function capture(remove = eval("var value; () => delete value")) {
            return remove;
        }
        var remove = capture();
        remove() && remove() ? 42 : 0
        "#,
    )
}

#[test]
fn direct_eval_parameter_arguments_conflicts_are_syntax_errors() -> TestResult {
    assert_script(
        r#"
        function evaluate(value = eval("var arguments")) {}
        var threw = false;
        try { evaluate(); } catch (error) { threw = error instanceof SyntaxError; }
        threw ? 42 : 0
        "#,
    )
}

#[test]
fn direct_eval_inherits_new_target_only_from_non_arrow_functions() -> TestResult {
    assert_script(
        r#"
        function observeNewTarget() { return eval("new.target"); }
        var arrowError;
        try { (() => eval("new.target"))(); } catch (error) { arrowError = error; }
        observeNewTarget() === undefined &&
            new observeNewTarget() === observeNewTarget &&
            arrowError instanceof SyntaxError ? 42 : 0
        "#,
    )
}

#[test]
fn rejects_parameter_and_body_lexical_name_conflicts_during_parsing() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let result = vm.eval("async function evaluate(value = 1) { let value; }");
    if matches!(result, Err(rs_quickjs::Error::Parse { .. })) {
        return Ok(());
    }
    Err(format!("expected parse error, got {result:?}").into())
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
