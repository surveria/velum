use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn routes_string_consumers_through_to_string() -> TestResult {
    eval_is_42(
        r#"
        let hints = "";
        let value = {};
        value[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint + ";";
            return "42";
        };
        let valid = String(value) === "42"
            && "x" + value === "x42"
            && `${value}` === "42"
            && [value].join() === "42"
            && "x".concat(value) === "x42";
        valid && hints === "string;default;string;string;string;" ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_ordinary_string_conversion_order() -> TestResult {
    eval_is_42(
        r#"
        let order = "";
        let value = {};
        value.toString = function () {
            order = order + "s";
            return {};
        };
        value.valueOf = function () {
            order = order + "v";
            return 42;
        };
        String(value) === "42" && order === "sv" ? 42 : 0
        "#,
    )
}

#[test]
fn converts_function_constructor_arguments_left_to_right() -> TestResult {
    eval_is_42(
        r#"
        let order = "";
        let parameter = {};
        parameter.toString = function () {
            order = order + "p";
            return "value";
        };
        let body = {};
        body.toString = function () {
            order = order + "b";
            return "return value + 1";
        };
        let generated = Function(parameter, body);
        generated(41) === 42 && order === "pb" ? 42 : 0
        "#,
    )
}

#[test]
fn keeps_to_boolean_free_of_user_code() -> TestResult {
    eval_is_42(
        r"
        let calls = 0;
        let value = {};
        value[Symbol.toPrimitive] = function () {
            calls = calls + 1;
            return 0;
        };
        let selected = value ? true : false;
        let filtered = [1, 2].filter(function () { return value; });
        Boolean(value) && !(!value) && selected && filtered.length === 2 && calls === 0 ? 42 : 0
        ",
    )
}

#[test]
fn keeps_string_constructor_symbol_exception_local() -> TestResult {
    eval_is_42(
        r#"
        let symbol = Symbol("name");
        let score = String(symbol) === "Symbol(name)" ? 40 : 0;
        try {
            new String(symbol);
        } catch (error) {
            score = score + 1;
        }
        try {
            "" + symbol;
        } catch (error) {
            score = score + 1;
        }
        score
        "#,
    )
}

#[test]
fn converts_error_messages_and_properties_observably() -> TestResult {
    eval_is_42(
        r#"
        let calls = "";
        let message = {};
        message.toString = function () {
            calls = calls + "m";
            return "boom";
        };
        let error = Error(message);
        let receiver = { name: {}, message: {} };
        receiver.name.toString = function () {
            calls = calls + "n";
            return "Custom";
        };
        receiver.message.toString = function () {
            calls = calls + "d";
            return "detail";
        };
        let text = Error.prototype.toString.call(receiver);
        error.message === "boom" && text === "Custom: detail" && calls === "mnd" ? 42 : 0
        "#,
    )
}

#[test]
fn reads_error_new_target_prototype_before_message_conversion() -> TestResult {
    eval_is_42(
        r#"
        let order = 0;
        function assertOrdering(expected) {
            if (order !== expected) {
                throw "expected " + expected + " got " + order;
            }
            order = order + 1;
        }
        let handler = { get() {
            assertOrdering(0);
            return Error.prototype;
        } };
        let constructor = new Proxy(Error, handler);
        let message = { toString() {
            assertOrdering(1);
            return "detail";
        } };
        new constructor(message);
        order === 2 ? 42 : 0
        "#,
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected value 42, got {value:?}").into())
}
