use rs_quickjs::{Runtime, Value};

mod support;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_function_constructor_metadata() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        typeof Function === "function" &&
            Function.name === "Function" &&
            Function.length === 1 &&
            Function.prototype.constructor === Function ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn function_constructor_creates_callable_functions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let add = Function("left", "right", "return left + right;");
        let fromCommaList = Function("left, right", "return left + right;");
        add(20, 22) + fromCommaList(10, 32)
        "#,
    )?;

    ensure_value(&value, &Value::Number(84.0))
}

#[test]
fn new_function_returns_created_function() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let created = new Function("value", "return value + 1;");
        typeof created === "function" &&
            created.name === "anonymous" &&
            created.length === 1 &&
            created(41) === 42 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn function_constructor_does_not_capture_caller_locals() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let make = function() {
            let hidden = 42;
            return Function("return typeof hidden;");
        };
        make()()
        "#,
    )?;

    ensure_value(&value, &Value::from("undefined"))
}

#[test]
fn function_constructor_name_is_metadata_not_a_lexical_binding() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let created = Function(
            "return typeof anonymous + ':' + function() { return typeof anonymous; }();"
        );
        created.name + ":" + created()
        "#,
    )?;

    ensure_value(&value, &Value::from("anonymous:undefined:undefined"))
}

#[test]
fn function_constructor_handles_parameter_line_comments() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let AsyncFunction = async function() {}.constructor;
        let sync = Function("a", " /* a */ b, c /* b */ //", "/* c */ ; /* d */ //");
        let asyncCreated = AsyncFunction("a", " /* a */ b, c /* b */ //", "/* c */ ; /* d */ //");
        let syncSource =
            "function anonymous(a, /* a */ b, c /* b */ //\n) {\n/* c */ ; /* d */ //\n}";
        let asyncSource =
            "async function anonymous(a, /* a */ b, c /* b */ //\n) {\n/* c */ ; /* d */ //\n}";
        sync(1, 2, 3) === undefined &&
            typeof asyncCreated === "function" &&
            "" + sync === syncSource &&
            "" + asyncCreated === asyncSource ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn function_constructor_rejects_invalid_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let result = context.eval(r#"Function(")")"#);
    if result.is_err() {
        return Ok(());
    }

    Err("expected invalid Function body to fail".into())
}

#[test]
fn function_constructor_rejects_parameter_text_that_escapes_the_parameter_list() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let rejected = 0;
        let cases = [
            ["/*", "*/) {"],
            ["//", ") {"],
            ["a = `", "` ) {"],
            [") { var x = function (", "} "],
            ["x = function (", "}) {"]
        ];
        for (let entry of cases) {
            try {
                Function(entry[0], entry[1]);
            } catch (error) {
                if (error instanceof SyntaxError) rejected = rejected + 1;
            }
        }
        rejected
        "#,
    )?;

    ensure_value(&value, &Value::Number(5.0))
}

#[test]
fn function_constructor_throws_catchable_syntax_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    support::install_assert(&mut context)?;

    let value = context.eval(
        r#"
        let AsyncFunction = async function() {}.constructor;
        let caughtBody = false;
        let caughtParam = false;
        let caughtNew = false;
        let caughtAsync = false;

        try {
            Function(")");
        } catch (error) {
            caughtBody = error.name === "SyntaxError";
        }

        try {
            Function("left right", "return 1;");
        } catch (error) {
            caughtParam = error.name === "SyntaxError";
        }

        try {
            new Function(")");
        } catch (error) {
            caughtNew = error.name === "SyntaxError";
        }

        try {
            AsyncFunction(")");
        } catch (error) {
            caughtAsync = error.name === "SyntaxError";
        }

        assert.throws(SyntaxError, function() {
            Function(")");
        });

        caughtBody && caughtParam && caughtNew && caughtAsync ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
