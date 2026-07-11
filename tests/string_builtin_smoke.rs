use rs_quickjs::{Runtime, Value};

mod support;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_string_constructor_and_wrapper_properties() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let stringConstructor = String;
        let constructed = new String("camera");
        let emptyObject = new String();
        let originalPrototype = String.prototype;
        String.prototype = null;
        let prototypeStayed = String.prototype === originalPrototype &&
            (new String("x")).__proto__ === originalPrototype;

        let constructorKeys = "";
        for (let key in String) {
            constructorKeys = constructorKeys + key + ";";
        }

        let prototypeKeys = "";
        for (let key in String.prototype) {
            prototypeKeys = prototypeKeys + key + ";";
        }

        let boxedKeys = "";
        for (let key in constructed) {
            boxedKeys = boxedKeys + key + ";";
        }

        let primitiveKeys = "";
        for (let key in "go") {
            primitiveKeys = primitiveKeys + key + ";";
        }

        print(
            typeof String,
            String.name,
            String.length,
            String.prototype.constructor === String
        );
        print(
            String(),
            String(null),
            String(undefined),
            String(true),
            String(false),
            String(42),
            String(Object())
        );
        print(
            constructed.length,
            constructed[0],
            constructed[1],
            emptyObject.length,
            String("front").length,
            String("front")[1]
        );
        print("keys:" + constructorKeys + "|" + prototypeKeys + "|" + boxedKeys + "|" + primitiveKeys);

        stringConstructor === String &&
            typeof String === "function" &&
            String.name === "String" &&
            String.length === 1 &&
            String.prototype.__proto__ === Object.prototype &&
            String.prototype.constructor.prototype === String.prototype &&
            constructed.__proto__ === String.prototype &&
            constructed.constructor === String &&
            typeof constructed === "object" &&
            prototypeStayed &&
            constructorKeys === "" &&
            prototypeKeys === "" &&
            boxedKeys === "0;1;2;3;4;5;" &&
            primitiveKeys === "0;1;" &&
            String() === "" &&
            String(null) === "null" &&
            String(undefined) === "undefined" &&
            String(true) === "true" &&
            String(false) === "false" &&
            String(42) === "42" &&
            String(Object()) === "[object Object]" &&
            constructed.length === 6 &&
            constructed[0] === "c" &&
            constructed[5] === "a" &&
            constructed[6] === undefined &&
            emptyObject.length === 0 &&
            String("front").length === 5 &&
            String("front")[1] === "r" &&
            "1" in "go" &&
            !("2" in "go") ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function String 1 true",
            " null undefined true false 42 [object Object]",
            "6 c a 0 5 r",
            "keys:||0;1;2;3;4;5;|0;1;",
        ],
    )
}

#[test]
fn supports_legacy_string_primitive_comparison_and_constructor_lookup() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let empty = "";
        let text = "x\0a";
        let next = "x\0b";
        let boxed = new String("ABC");
        let emptyBoxed = new String("");
        text < next &&
            next > text &&
            text <= "x\0a" &&
            next >= "x\0b" &&
            empty == 0 &&
            empty == false &&
            empty != undefined &&
            empty != null &&
            empty !== 0 &&
            "rock'n'roll".constructor === String &&
            "ABC".constructor === boxed.constructor &&
            "ABC" == boxed &&
            "" == emptyBoxed &&
            emptyBoxed == false &&
            emptyBoxed !== "" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_legacy_bare_string_constructor_call() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let str = "";
        let strObj = new String;
        str.constructor === strObj.constructor &&
            str == strObj &&
            str !== strObj &&
            typeof str !== typeof strObj ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_adjacent_string_literals_after_var_initializer() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let Err(error) = context.eval("var str = '''';") else {
        return Err("expected adjacent single-quote string literals to fail parsing".into());
    };

    let message = error.to_string();
    if !message.contains("expected statement terminator after variable declaration") {
        return Err(format!("expected statement terminator parse error, got {message}").into());
    }

    let Err(error) = context.eval(r#"var str = """";"#) else {
        return Err("expected adjacent double-quote string literals to fail parsing".into());
    };

    let message = error.to_string();
    if message.contains("expected statement terminator after variable declaration") {
        return Ok(());
    }

    Err(format!("expected statement terminator parse error, got {message}").into())
}

#[test]
fn assert_throws_observes_reference_error_from_eval_line_terminator_source() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    support::install_assert(&mut context)?;

    let value = context.eval(
        r#"
        assert.throws(ReferenceError, function() {
            eval("var x = asdf\u000Aghjk");
        });
        assert.throws(ReferenceError, function() {
            eval("var x = asdf\u2028ghjk");
        });
        42
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

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
