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

#[test]
fn generator_declaration_yields_and_returns() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            yield 40;
            return 42;
        }
        const iterator = values();
        const first = iterator.next();
        const second = iterator.next();
        first.value + ":" + first.done + ":" + second.value + ":" + second.done
        "#,
    )?;
    ensure_value(&value, &Value::String("40:false:42:true".to_owned()))
}

#[test]
fn next_value_becomes_yield_expression_result() -> TestResult {
    let value = eval(
        r#"
        const iterator = (function* () {
            const received = yield 1;
            return received + 2;
        })();
        iterator.next();
        iterator.next(40).value
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn generator_return_runs_finally_and_can_yield_again() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            try {
                yield 1;
            } finally {
                yield 2;
            }
        }
        const iterator = values();
        iterator.next();
        const cleanup = iterator.return(40);
        const completed = iterator.next();
        cleanup.value + ":" + cleanup.done + ":" + completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::String("2:false:40:true".to_owned()))
}

#[test]
fn generator_throw_enters_catch() -> TestResult {
    let value = eval(
        r#"
        function* values() {
            try {
                yield 1;
            } catch (error) {
                return error + 2;
            }
        }
        const iterator = values();
        iterator.next();
        const completed = iterator.throw(40);
        completed.value + ":" + completed.done
        "#,
    )?;
    ensure_value(&value, &Value::String("42:true".to_owned()))
}

#[test]
fn generator_object_method_is_iterable() -> TestResult {
    let value = eval(
        r#"
        const holder = {
            *values() {
                yield 20;
                yield 22;
            }
        };
        let total = 0;
        for (const value of holder.values()) {
            total = total + value;
        }
        total
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn evaluates_parameters_on_call_but_defers_the_body() -> TestResult {
    let value = eval(
        r#"
        let bodyCalls = 0;
        let parameterThrew = false;
        function* values(value = missing) {
            bodyCalls = bodyCalls + 1;
            yield value;
        }
        try {
            values();
        } catch (error) {
            parameterThrew = error instanceof ReferenceError;
        }
        const iterator = values(42);
        parameterThrew + ":" + bodyCalls + ":" + iterator.next().value + ":" + bodyCalls
        "#,
    )?;
    ensure_value(&value, &Value::String("true:0:42:1".to_owned()))
}

#[test]
fn generator_destructuring_parameter_errors_are_catchable_on_call() -> TestResult {
    let value = eval(
        r#"
        function* values({}) {}
        let caught = false;
        try {
            values(null);
        } catch (error) {
            caught = error instanceof TypeError;
        }
        caught
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}
