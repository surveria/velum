use rs_quickjs::{Engine, Error, Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn hoists_top_level_function_declarations() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let first = add(20, 22);
        function add(left, right) {
            return left + right;
        }
        first === 42 && add.length === 2 && add.name === 'add' ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_recursive_function_declarations() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        function factorial(value) {
            if (value <= 1) {
                return 1;
            }
            return value * factorial(value - 1);
        }
        factorial(5)
        ",
    )?;

    ensure_value(&value, &Value::Number(120.0))
}

#[test]
fn async_function_expressions_inherit_object_prototype_helpers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let task = async function task() {};
        task.prototype === undefined &&
            task.hasOwnProperty("prototype") === false &&
            task.propertyIsEnumerable("name") === false
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn nested_function_declarations_capture_function_scope() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        function makeAdder(base) {
            return add(2);
            function add(value) {
                return base + value;
            }
        }
        makeAdder(40)
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_usage_tracks_function_declaration_hoists() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        function answer() {
            return 42;
        }
        answer()
        ",
    )?;
    let usage = script.usage();

    ensure_usize(usage.bytecode_hoisted_function_count(), 1)?;
    ensure_usize(usage.bytecode_hoisted_var_count(), 1)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_excessively_nested_function_declarations_with_limit_error() -> TestResult {
    let limits = RuntimeLimits {
        max_expression_depth: 3,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        function one() {
            function two() {
                function three() {
                    function four() {
                        return 42;
                    }
                    return four();
                }
                return three();
            }
            return two();
        }
        one()
        ",
    ) else {
        return Err(
            "expected nested function declarations to hit the statement depth limit".into(),
        );
    };

    ensure_limit_error(&error, "statement nesting exceeded 3")
}

#[test]
fn rejects_recursive_function_declarations_with_call_depth_limit() -> TestResult {
    let limits = RuntimeLimits {
        max_expression_depth: 8,
        ..RuntimeLimits::default()
    };
    let runtime = Runtime::with_limits(limits);
    let mut context = runtime.context();

    let Err(error) = context.eval(
        r"
        function recurse() {
            return recurse();
        }
        recurse()
        ",
    ) else {
        return Err("expected recursive function declaration to hit the call depth limit".into());
    };

    ensure_limit_error(&error, "call stack depth exceeded 8")
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_limit_error(error: &Error, expected: &str) -> TestResult {
    match error {
        Error::ResourceLimit { message } if message.contains(expected) => Ok(()),
        actual => Err(format!("expected limit error containing {expected:?}, got {actual}").into()),
    }
}
