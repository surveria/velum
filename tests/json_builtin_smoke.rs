use rs_quickjs::{Engine, Runtime, Value};

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

const DIRECT_JSON_TARGET_ARGS_SCRIPT: &str = r#"
let order = "";
let mark = function(label, value) {
    order = order + label;
    return value;
};

let run = function() {
    let parsed = JSON.parse(mark("a", '{"value":7}'), mark("b", "unused"));
    let text = JSON.stringify(
        mark("c", { value: parsed.value, skip: undefined }),
        mark("d", null),
        mark("e", 2)
    );
    return parsed.value === 7 && text === '{\n  "value": 7\n}';
};

let first = run();
let second = run();

first && second && order === "abcdeabcde" ? 42 : 0
"#;

const JSON_CALLBACKS_SCRIPT: &str = r#"
let order = "";
let parsed = JSON.parse(
    '{"a":1,"nested":{"b":2},"arr":[3,4],"drop":5}',
    function(key, value) {
        order = order + key + ";";
        if (key === "drop" || key === "1") {
            return undefined;
        }
        if (key === "b") {
            return value + 40;
        }
        return value;
    }
);

let replacerThisOk = false;
let replacerText = JSON.stringify(
    { a: 1, b: 2, c: 3 },
    function(key, value) {
        if (key === "a" && this.a === 1) {
            replacerThisOk = true;
        }
        if (key === "b") {
            return undefined;
        }
        return value;
    }
);

let listText = JSON.stringify({ a: 1, b: 2, c: 3 }, ["c", "a", "c"]);
let prettyText = JSON.stringify({ a: 1, nested: { b: 2 } }, null, 2);
let arrayText = JSON.stringify([1, 2, 3], function(key, value) {
    if (key === "1") {
        return undefined;
    }
    return value;
});

let toJsonKey = "";
let toJsonText = JSON.stringify({
    keep: {
        toJSON: function(key) {
            toJsonKey = key;
            return { answer: 42, skip: undefined };
        }
    }
});

parsed.a === 1 &&
    parsed.nested.b === 42 &&
    parsed.arr.length === 2 &&
    parsed.arr[0] === 3 &&
    parsed.arr[1] === undefined &&
    parsed.drop === undefined &&
    order === "a;b;nested;0;1;arr;drop;;" &&
    replacerThisOk &&
    replacerText === '{"a":1,"c":3}' &&
    listText === '{"c":3,"a":1}' &&
    prettyText === '{\n  "a": 1,\n  "nested": {\n    "b": 2\n  }\n}' &&
    arrayText === '[1,null,3]' &&
    toJsonKey === "keep" &&
    toJsonText === '{"keep":{"answer":42}}'
    ? 42
    : 0
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

#[test]
fn direct_json_targets_preserve_argument_semantics() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(DIRECT_JSON_TARGET_ARGS_SCRIPT)?;

    ensure_at_least(
        script.usage().bytecode_direct_native_call_count(),
        2,
        "direct JSON native call operands",
    )?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))?;

    let usage = vm.resource_usage();
    ensure_at_least(
        usage.native_call_cache_misses,
        2,
        "direct JSON native call cache misses",
    )?;
    ensure_at_least(
        usage.native_call_cache_hits,
        2,
        "direct JSON native call cache hits",
    )
}

#[test]
fn supports_json_reviver_replacer_space_and_to_json() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(JSON_CALLBACKS_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::from(expected))
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

fn ensure_at_least(actual: usize, expected: usize, label: &str) -> TestResult {
    if actual >= expected {
        return Ok(());
    }

    Err(format!("expected {label} >= {expected}, got {actual}").into())
}

#[test]
fn parse_reviver_transforms_and_deletes() -> TestResult {
    ensure_string(
        r#"
        const scaled = JSON.parse('{"a":1,"b":{"c":2}}', function (key, value) {
            if (typeof value === "number") {
                return value * 10;
            }
            if (key === "b") {
                return value;
            }
            return value;
        });
        const dropped = JSON.parse('{"keep":1,"drop":2}', function (key, value) {
            if (key === "drop") {
                return undefined;
            }
            return value;
        });
        "" + scaled.a + ":" + scaled.b.c + ":" + dropped.keep
            + ":" + ("drop" in dropped)
        "#,
        "10:20:1:false",
    )
}

#[test]
fn parse_reviver_preserves_non_configurable_array_elements() -> TestResult {
    ensure_string(
        r#"
        const parsed = JSON.parse('[1,2]', function (key, value) {
            if (key === "0") {
                Object.defineProperty(this, "1", { configurable: false });
            }
            return key === "1" ? undefined : value;
        });
        String(parsed.length) + ":" + String(parsed[1]) + ":" + String(1 in parsed)
        "#,
        "2:2:true",
    )
}

#[test]
fn stringify_replacer_array_filters_keys() -> TestResult {
    ensure_string(
        r#"
        JSON.stringify({a: 1, b: 2, c: {a: 3, d: 4}}, ["a", "c"])
        "#,
        r#"{"a":1,"c":{"a":3}}"#,
    )
}

#[test]
fn stringify_space_forms_indent_output() -> TestResult {
    ensure_string(
        r#"
        const numeric = JSON.stringify({a: [1]}, null, 2);
        const stringy = JSON.stringify({a: 1}, null, "--");
        const clamped = JSON.stringify({a: 1}, null, 100);
        const compact = JSON.stringify({a: 1}, null, 0);
        numeric + "|" + stringy + "|" + (clamped === JSON.stringify({a: 1}, null, 10))
            + "|" + compact
        "#,
        "{\n  \"a\": [\n    1\n  ]\n}|{\n--\"a\": 1\n}|true|{\"a\":1}",
    )
}

#[test]
fn stringify_cycles_throw_type_error() -> TestResult {
    ensure_string(
        r#"
        const o = {};
        o.self = o;
        let kind = "none";
        try {
            JSON.stringify(o);
        } catch (error) {
            kind = error instanceof TypeError ? "TypeError" : "other";
        }
        kind
        "#,
        "TypeError",
    )
}
