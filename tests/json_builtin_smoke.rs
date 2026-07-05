use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const JSON_BUILTIN_SCRIPT: &str = r#"
let parsed = JSON.parse('{"camera":"front","active":true,"count":2,"items":[1,null,"x"],"nested":{"ok":false}}');
let jsonKeys = "";
for (let key in JSON) {
    jsonKeys = jsonKeys + key + ";";
}

let primitiveOk =
    JSON.parse("true") === true &&
    JSON.parse("false") === false &&
    JSON.parse("null") === null &&
    JSON.parse("42") === 42 &&
    JSON.parse('"lens"') === "lens";

let generated = JSON.stringify({
    z: 1,
    a: "front",
    skip: undefined,
    fn: function() {
        return 1;
    },
    nested: { ok: true },
    arr: [true, undefined, null, NaN, Infinity]
});
let arrayText = JSON.stringify(parsed.items);
let negativeZero = JSON.stringify(-0);

print(
    typeof JSON,
    JSON.__proto__ === Object.prototype,
    typeof JSON.parse,
    JSON.parse.name,
    JSON.parse.length,
    typeof JSON.stringify,
    JSON.stringify.name,
    JSON.stringify.length
);
print(parsed.camera, parsed.active, parsed.count, parsed.items.length, parsed.items[2], parsed.nested.ok);
print(
    JSON.stringify(null),
    JSON.stringify(true),
    JSON.stringify(false),
    JSON.stringify("front"),
    JSON.stringify(42),
    JSON.stringify(NaN),
    JSON.stringify(Infinity),
    JSON.stringify(undefined),
    negativeZero
);
print(arrayText);
print(generated);
print("keys:" + jsonKeys);

primitiveOk &&
    typeof JSON === "object" &&
    JSON.__proto__ === Object.prototype &&
    typeof JSON.parse === "function" &&
    JSON.parse.name === "parse" &&
    JSON.parse.length === 2 &&
    typeof JSON.stringify === "function" &&
    JSON.stringify.name === "stringify" &&
    JSON.stringify.length === 3 &&
    parsed.camera === "front" &&
    parsed.active === true &&
    parsed.count === 2 &&
    parsed.items.length === 3 &&
    parsed.items[0] === 1 &&
    parsed.items[1] === null &&
    parsed.items[2] === "x" &&
    parsed.nested.ok === false &&
    arrayText === '[1,null,"x"]' &&
    generated === '{"z":1,"a":"front","nested":{"ok":true},"arr":[true,null,null,null,null]}' &&
    JSON.stringify(undefined) === undefined &&
    negativeZero === "0" &&
    jsonKeys === "" ? 42 : 0
	"#;

#[test]
fn exposes_json_parse_and_stringify() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(JSON_BUILTIN_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "object true function parse 2 function stringify 3",
            "front true 2 3 x false",
            "null true false \"front\" 42 null null undefined 0",
            "[1,null,\"x\"]",
            "{\"z\":1,\"a\":\"front\",\"nested\":{\"ok\":true},\"arr\":[true,null,null,null,null]}",
            "keys:",
        ],
    )
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
