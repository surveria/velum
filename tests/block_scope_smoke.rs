use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_block_and_for_lexical_scopes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let outer = 1;
        let total = 0;
        {
            let outer = 40;
            const delta = 2;
            total = outer + delta;
            print(total, typeof delta);
        }
        print(outer, typeof delta);

        let loopTotal = 0;
        for (let index = 0; index < 4; index = index + 1) {
            let record = { value: index + 1 };
            loopTotal = loopTotal + record.value;
        }
        print(loopTotal, typeof index, typeof record);

        let pair = 0;
        for (let left = 20, right = 22; left < 21; left = left + 1) {
            pair = left + right;
        }
        print(pair, typeof left, typeof right);

        {
            var hoisted = 42;
        }
        print(hoisted);

        total === 42 &&
            outer === 1 &&
            loopTotal === 10 &&
            pair === 42 &&
            hoisted === 42 &&
            typeof delta === "undefined" &&
            typeof index === "undefined" &&
            typeof record === "undefined" &&
            typeof left === "undefined" &&
            typeof right === "undefined"
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &[
            "42 number".to_owned(),
            "1 undefined".to_owned(),
            "10 undefined undefined".to_owned(),
            "42 undefined undefined".to_owned(),
            "42".to_owned(),
        ],
    )
}

#[test]
fn preserves_var_only_blocks_and_switch_lexical_scopes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let total = 0;
        {
            var hoisted = 20;
            total = total + hoisted;
        }
        switch (1) {
            case 1:
                let hidden = 22;
                total = total + hidden;
                break;
            default:
                total = 0;
        }
        total === 42 &&
            hoisted === 20 &&
            typeof hidden === "undefined"
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn scopes_try_catch_and_finally_blocks() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let status = "";
        try {
            let hidden = 1;
            throw "boom";
        } catch (error) {
            let caught = 40;
            status = error + " " + caught;
        } finally {
            let finalValue = 2;
            status = status + " " + finalValue;
        }
        print(status, typeof hidden, typeof error, typeof caught, typeof finalValue);
        status === "boom 40 2" &&
            typeof hidden === "undefined" &&
            typeof error === "undefined" &&
            typeof caught === "undefined" &&
            typeof finalValue === "undefined"
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))?;
    ensure_output(
        context.output(),
        &["boom 40 2 undefined undefined undefined undefined".to_owned()],
    )
}

#[test]
fn preserves_var_only_try_catch_finally_without_catch_parameter_leak() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        var status = 0;
        try {
            var fromTry = 20;
            throw 1;
        } catch (error) {
            var fromCatch = fromTry + error;
            status = fromCatch;
        } finally {
            var fromFinally = 21;
            status = status + fromFinally;
        }
        status === 42 &&
            fromTry === 20 &&
            fromCatch === 21 &&
            fromFinally === 21 &&
            typeof error === "undefined"
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn preserves_direct_throw_unreachable_tail() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        var value = 0;
        try {
            throw "caught";
            value = 100;
        } catch (error) {
            if (error === "caught") {
                value = value + 1;
            }
        }
        value === 1
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn preserves_direct_catch_fast_path_false_branch() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        var value = 0;
        try {
            throw "miss";
        } catch (error) {
            if (error === "caught") {
                value = value + 1;
            }
        }
        value === 0
        "#,
    )?;

    ensure_value(&value, &Value::Bool(true))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
