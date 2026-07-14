use rs_quickjs::{Engine, HostOperation, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn eval_lexical_declarations_are_isolated_per_call() -> TestResult {
    expect_true(
        r#"
        eval("let value = 1; class C {}; value") === 1 &&
        eval("let value = 2; class C {}; value") === 2 &&
        typeof value === "undefined" &&
        typeof C === "undefined"
        "#,
    )
}

#[test]
fn sloppy_eval_vars_update_the_outer_environment() -> TestResult {
    expect_true(
        r#"
        var value = 1;
        eval("var value = 42; var created = 7");
        value === 42 && created === 7
        "#,
    )
}

#[test]
fn sloppy_eval_extends_shared_parameter_indexes_per_call() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.eval(
        r#"
        function readCamera(lens) {
            eval("var observed = lens + 1");
            return observed;
        }
        readCamera(40) === 41 && readCamera(1) === 2 &&
        typeof observed === "undefined"
        "#,
    )?;
    if value != Value::Bool(true) {
        return Err(format!("expected shared-index copy-on-write result, got {value:?}").into());
    }
    vm.storage_snapshot()?;
    Ok(())
}

#[test]
fn strict_eval_keeps_vars_local_and_preserves_captured_lexicals() -> TestResult {
    expect_true(
        r#"
        var closure = eval('"use strict"; var hidden = 1; let value = 40; () => ++value');
        closure() === 41 && closure() === 42 &&
        typeof hidden === "undefined" && typeof value === "undefined"
        "#,
    )
}

#[test]
fn sloppy_eval_functions_capture_eval_lexical_and_variable_bindings() -> TestResult {
    expect_true(
        r"
        function createReader() {
            var value = 2;
            return eval(`
                let offset = 40;
                var value = 4;
                function read() { return offset + value; }
                read
            `);
        }
        var read = createReader();
        read() === 44
        ",
    )
}

#[test]
fn sloppy_eval_var_declarations_target_the_function_variable_environment() -> TestResult {
    expect_true(
        r#"
        function createValue() {
            {
                let blockOnly = 1;
                eval("var createdByEval = 41");
                if (blockOnly !== 1) return -1;
            }
            return createdByEval;
        }
        createValue() === 41 && typeof createdByEval === "undefined"
        "#,
    )
}

#[test]
fn eval_created_vars_shadow_globals_only_inside_the_owning_function() -> TestResult {
    expect_true(
        r"
        var value = 42;
        function readGlobal() { return value; }
        function updateLocal() {
            var readLocal = eval(`
                var value = 5;
                function read() { return value; }
                read
            `);
            if (readLocal() !== 5 || value !== 5 || readGlobal() !== 42) return false;
            value = 8;
            return readLocal() === 8 && value === 8 && readGlobal() === 42;
        }
        updateLocal() && value === 42
        ",
    )
}

#[test]
fn catch_var_redeclarations_use_the_current_catch_binding() -> TestResult {
    expect_true(
        r#"
        var value = "global";
        var before;
        var after;
        try {
            throw "caught";
        } catch (value) {
            before = value;
            var value = "updated";
            after = value;
        }
        before === "caught" && after === "updated" && value === "global"
        "#,
    )
}

#[test]
fn declarations_preserve_the_previous_completion_value() -> TestResult {
    expect_true(
        r#"
        eval("1; var first") === 1 &&
        eval("2; let second = 0") === 2 &&
        eval("3; const third = 0") === 3 &&
        eval("4; class Fourth {}") === 4
        "#,
    )
}

#[test]
fn indirect_eval_uses_the_global_variable_environment() -> TestResult {
    expect_true(
        r#"
        function run() {
            let local = 1;
            var indirect = eval;
            indirect("var indirectGlobal = 42; let indirectLexical = 7");
            return local;
        }
        run() === 1 && indirectGlobal === 42 &&
        typeof indirectLexical === "undefined"
        "#,
    )
}

#[test]
fn indirect_eval_var_declarations_preserve_builtin_bindings() -> TestResult {
    expect_true(
        r#"
        var indirectEval = eval;
        indirectEval("var eval;");
        eval === indirectEval
        "#,
    )
}

#[test]
fn global_eval_declarations_create_configurable_bindings() -> TestResult {
    expect_true(
        r#"
        eval("var evalVar = 1; function evalFunction() { return 2; }");
        var varDescriptor = Object.getOwnPropertyDescriptor(globalThis, "evalVar");
        var functionDescriptor = Object.getOwnPropertyDescriptor(globalThis, "evalFunction");
        evalVar === 1 && evalFunction() === 2 &&
        varDescriptor.writable && varDescriptor.enumerable && varDescriptor.configurable &&
        functionDescriptor.writable && functionDescriptor.enumerable &&
        functionDescriptor.configurable
        "#,
    )
}

#[test]
fn global_eval_bindings_follow_configurable_property_deletion() -> TestResult {
    expect_true(
        r#"
        eval("var evalCreated = 1");
        var deleted = eval("delete evalCreated");
        var missing = typeof evalCreated === "undefined";
        evalCreated = 2;
        deleted && missing && evalCreated === 2 && globalThis.evalCreated === 2
        "#,
    )
}

#[test]
fn global_eval_rejects_non_definable_functions_before_mutation() -> TestResult {
    expect_true(
        r#"
        var error;
        try {
            eval("var untouched; function NaN() {}");
        } catch (caught) {
            error = caught;
        }
        error instanceof TypeError && typeof untouched === "undefined"
        "#,
    )
}

#[test]
fn annex_b_eval_block_functions_use_configurable_global_vars() -> TestResult {
    expect_true(
        r#"
        eval("var initial = blockFunction; { function blockFunction() { return 42; } }");
        var descriptor = Object.getOwnPropertyDescriptor(globalThis, "blockFunction");
        initial === undefined && blockFunction() === 42 &&
        descriptor.writable && descriptor.enumerable && descriptor.configurable
        "#,
    )
}

#[test]
fn annex_b_eval_preserves_existing_globals_after_separate_scripts() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    for name in [
        "hostAgentStart",
        "hostAgentBroadcast",
        "hostAgentReport",
        "hostAgentSleep",
    ] {
        context.register_host_function(name, |_call| Ok(Value::Undefined))?;
    }
    context.register_host_operation("hostDetachBuffer", HostOperation::DetachArrayBuffer)?;
    context.register_host_operation("hostCreateRealm", HostOperation::CreateRealm)?;
    context.eval(
        r"
        var harnessHost = {
            global: globalThis,
            detach: hostDetachBuffer,
            createRealm: hostCreateRealm,
            agent: {
                start: hostAgentStart,
                broadcast: hostAgentBroadcast,
                report: hostAgentReport,
                sleep: hostAgentSleep
            }
        };
        ",
    )?;
    context.eval("let harnessLexical = 1; var harnessGlobal = 2;")?;
    context.eval(
        r#"
        var savedGlobalObject = Function("return this;")();
        function globalObject() { return savedGlobalObject; }
        "#,
    )?;
    let value = context.eval(
        r#"
        Object.defineProperty(globalObject(), "existingEvalFunction", {
            value: "initial",
            enumerable: true,
            writable: true,
            configurable: false
        });
        eval("var initial = existingEvalFunction; { function existingEvalFunction() { return 42; } }");
        initial === "initial" && existingEvalFunction() === 42
        "#,
    )?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected preserved eval global, got {value:?}").into())
}

#[test]
fn nested_eval_closures_observe_deletable_var_bindings() -> TestResult {
    expect_true(
        r#"
        function captureVarDelete() {
            return eval("var value; () => delete value");
        }
        function captureFunctionDelete() {
            return eval("function value() {} (() => delete value)");
        }
        function captureLetDelete() {
            return eval("let value; () => delete value");
        }
        var deleteVar = captureVarDelete();
        var deleteFunction = captureFunctionDelete();
        deleteVar() && deleteVar() && deleteFunction() && deleteFunction() &&
            captureLetDelete()() === false
        "#,
    )
}

#[test]
fn configurable_eval_vars_do_not_block_later_global_lexicals() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval("eval('if (true) { function configurableEvalBinding() {} }');")?;
    context.eval("let configurableEvalBinding = 42;")?;
    let value = context.eval(
        r#"
        configurableEvalBinding === 42 &&
        typeof globalThis.configurableEvalBinding === "function"
        "#,
    )?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected lexical binding to shadow eval property, got {value:?}").into())
}

#[test]
fn direct_eval_spread_calls_keep_the_caller_environment() -> TestResult {
    expect_true(
        r#"
        function sloppy() {
            let value = 0;
            eval(...[], "value = 1");
            eval("value = 2", ...[]);
            eval(...["value = 3"]);
            return value;
        }
        function strict() {
            "use strict";
            let value = 0;
            eval(...["value = 4"]);
            return value;
        }
        sloppy() === 3 && strict() === 4
        "#,
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
