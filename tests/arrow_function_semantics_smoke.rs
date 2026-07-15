use velum::{Error, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn arrow_functions_capture_lexical_this() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        function Holder() {
            this.arrow = () => this;
        }
        const holder = new Holder();
        const usurper = {};
        holder.arrow() === holder &&
            holder.arrow.call(usurper) === holder &&
            holder.arrow.apply(usurper) === holder &&
            holder.arrow.bind(usurper)() === holder
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn arrows_created_by_direct_eval_capture_the_enclosing_this() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        function fromEval() { return eval("() => this"); }
        function throughEval() { return () => eval("this"); }
        "#,
    )?;
    ensure_value(&context.eval("fromEval()() === this")?, &Value::Bool(true))?;
    ensure_value(
        &context.eval("throughEval()() === this")?,
        &Value::Bool(true),
    )
}

#[test]
fn arrow_functions_inherit_restricted_function_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const arrow = () => {};
        let callerThrows = false;
        let argumentsThrows = false;
        try { arrow.caller; } catch (error) { callerThrows = error instanceof TypeError; }
        try { arrow.arguments; } catch (error) { argumentsThrows = error instanceof TypeError; }
        !arrow.hasOwnProperty("caller") &&
            !arrow.hasOwnProperty("arguments") &&
            callerThrows && argumentsThrows
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn arrow_super_call_reuses_the_constructor_environment() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        let calls = 0;
        class Base { constructor() { calls++; } }
        class Derived extends Base {
            constructor() {
                super();
                this.callSuper = () => super();
            }
        }
        const instance = new Derived();
        let threw = false;
        try { instance.callSuper(); } catch (error) {
            threw = error instanceof ReferenceError;
        }
        threw && calls === 2
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn arrow_parameters_reject_duplicate_pattern_bindings() -> TestResult {
    for source in [
        "const arrow = (x, [x]) => 1;",
        "const arrow = (x, {x}) => 1;",
        "const arrow = ([x], {value: x}) => 1;",
    ] {
        ensure_parse_error(source)?;
    }
    Ok(())
}

#[test]
fn strict_arrow_parameters_reject_future_reserved_words() -> TestResult {
    for source in [
        r#""use strict"; const arrow = package => 1;"#,
        r#""use strict"; const arrow = ({ \u0069mplements }) => 1;"#,
    ] {
        ensure_parse_error(source)?;
    }
    Ok(())
}

#[test]
fn arrow_lookahead_balances_mixed_nested_delimiters() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        const arrow = ({ outer: [first, { value: second }] }, [third]) =>
            first + second + third;
        arrow({ outer: [10, { value: 12 }] }, [20]) === 42;
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))?;

    for source in [
        "const arrow = ([value}) => value;",
        "const arrow = ({value]) => value;",
    ] {
        ensure_parse_error(source)?;
    }
    Ok(())
}

fn ensure_parse_error(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval(source) else {
        return Err(format!("expected '{source}' to fail").into());
    };
    if matches!(error, Error::Parse { .. }) {
        return Ok(());
    }
    Err(format!("expected parse error for '{source}', got {error:?}").into())
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
