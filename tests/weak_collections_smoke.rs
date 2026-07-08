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

fn ensure_string(source: &str, expected: &str) -> TestResult {
    ensure_value(&eval(source)?, &Value::String(expected.to_owned()))
}

#[test]
fn weak_map_stores_object_and_symbol_keys() -> TestResult {
    ensure_string(
        r#"
        const wm = new WeakMap();
        const objectKey = {};
        const symbolKey = Symbol("weak");
        wm.set(objectKey, "object").set(symbolKey, "symbol");
        "" + wm.get(objectKey) + ":" + wm.get(symbolKey)
            + ":" + wm.has(objectKey) + ":" + wm.has({})
        "#,
        "object:symbol:true:false",
    )
}

#[test]
fn weak_set_stores_object_and_symbol_values() -> TestResult {
    ensure_string(
        r#"
        const ws = new WeakSet();
        const objectKey = {};
        const symbolKey = Symbol("weak");
        ws.add(objectKey).add(symbolKey);
        "" + ws.has(objectKey) + ":" + ws.has(symbolKey) + ":" + ws.has({})
            + ":" + ws.delete(objectKey) + ":" + ws.has(objectKey)
        "#,
        "true:true:false:true:false",
    )
}

#[test]
fn weak_collections_seed_from_iterables() -> TestResult {
    ensure_string(
        r#"
        const key = {};
        const setKey = {};
        const wm = new WeakMap([[key, 7]]);
        const ws = new WeakSet([setKey]);
        "" + wm.get(key) + ":" + ws.has(setKey)
        "#,
        "7:true",
    )
}

#[test]
fn weak_collections_handle_primitive_keys_per_method() -> TestResult {
    ensure_string(
        r#"
        function kind(callback) {
            try {
                callback();
                return "none";
            } catch (error) {
                return error instanceof TypeError ? "TypeError" : "other";
            }
        }
        const wm = new WeakMap();
        const ws = new WeakSet();
        "" + (wm.get(1) === undefined)
            + ":" + wm.has(1)
            + ":" + wm.delete(1)
            + ":" + ws.has("x")
            + ":" + ws.delete("x")
            + ":" + kind(function () { wm.set(1, 2); })
            + ":" + kind(function () { ws.add(1); })
        "#,
        "true:false:false:false:false:TypeError:TypeError",
    )
}

#[test]
fn weak_collection_constructors_and_receivers_are_validated() -> TestResult {
    ensure_string(
        r#"
        function kind(callback) {
            try {
                callback();
                return "none";
            } catch (error) {
                return error instanceof TypeError ? "TypeError" : "other";
            }
        }
        const wm = new WeakMap();
        const ws = new WeakSet();
        "" + kind(function () { WeakMap(); })
            + ":" + kind(function () { WeakSet(); })
            + ":" + kind(function () { new WeakMap([[1, 2]]); })
            + ":" + kind(function () { new WeakSet([1]); })
            + ":" + kind(function () { WeakMap.prototype.get.call(ws, {}); })
            + ":" + (wm instanceof WeakMap)
            + ":" + (ws instanceof WeakSet)
            + ":" + (WeakMap.prototype.size === undefined)
            + ":" + (WeakSet.prototype.clear === undefined)
        "#,
        "TypeError:TypeError:TypeError:TypeError:TypeError:true:true:true:true",
    )
}
