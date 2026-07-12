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
    ensure_value(&eval(source)?, &Value::from(expected))
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

#[test]
fn weak_map_upserts_preserve_callbacks_and_mutations() -> TestResult {
    ensure_string(
        r#"
        "use strict";
        const map = new WeakMap();
        const present = {};
        const missing = Symbol("missing");
        map.set(present, 1);
        let calls = 0;
        const existing = map.getOrInsertComputed(present, function () {
            calls += 1;
            return 2;
        });
        const inserted = map.getOrInsertComputed(missing, function (key) {
            calls += 1;
            map.set(key, 3);
            return this === undefined && arguments.length === 1 && key === missing ? 4 : 0;
        });
        const direct = {};
        "" + existing + ":" + inserted + ":" + map.get(missing)
            + ":" + map.getOrInsert(direct, 5) + ":" + calls
            + ":" + WeakMap.prototype.getOrInsert.length
            + ":" + WeakMap.prototype.getOrInsertComputed.name
        "#,
        "1:4:4:5:1:2:getOrInsertComputed",
    )
}

#[test]
fn weak_collection_constructors_use_dynamic_adders_and_close_iterators() -> TestResult {
    ensure_string(
        r#"
        const key = {};
        const original = WeakMap.prototype.set;
        let receiverMatches = false;
        let observedValue = 0;
        WeakMap.prototype.set = function (entryKey, value) {
            receiverMatches = this instanceof WeakMap;
            observedValue = value;
            return original.call(this, entryKey, value);
        };
        const seeded = new WeakMap([[key, 7]]);
        let closed = 0;
        const iterable = {};
        iterable[Symbol.iterator] = function () {
            return {
                next: function () { return { value: [key, 8], done: false }; },
                return: function () { closed += 1; return {}; }
            };
        };
        WeakMap.prototype.set = function () { throw new RangeError("stop"); };
        let errorName = "none";
        try {
            new WeakMap(iterable);
        } catch (error) {
            errorName = error.name;
        }
        "" + receiverMatches + ":" + observedValue + ":" + seeded.get(key)
            + ":" + errorName + ":" + closed
        "#,
        "true:7:7:RangeError:1",
    )
}

#[test]
fn registered_symbols_are_not_weak_keys() -> TestResult {
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
        const key = Symbol.for("registered");
        const map = new WeakMap();
        const set = new WeakSet();
        "" + kind(function () { map.set(key, 1); })
            + ":" + kind(function () { map.getOrInsert(key, 1); })
            + ":" + kind(function () { map.getOrInsertComputed(key, function () { return 1; }); })
            + ":" + kind(function () { set.add(key); })
            + ":" + (map.get(key) === undefined) + ":" + map.has(key)
        "#,
        "TypeError:TypeError:TypeError:TypeError:true:false",
    )
}

#[test]
fn weak_collection_to_string_tags_have_standard_descriptors() -> TestResult {
    ensure_string(
        r#"
        const mapDescriptor = Object.getOwnPropertyDescriptor(
            WeakMap.prototype,
            Symbol.toStringTag
        );
        const setDescriptor = Object.getOwnPropertyDescriptor(
            WeakSet.prototype,
            Symbol.toStringTag
        );
        "" + WeakMap.prototype[Symbol.toStringTag]
            + ":" + WeakSet.prototype[Symbol.toStringTag]
            + ":" + mapDescriptor.writable + ":" + mapDescriptor.enumerable
            + ":" + mapDescriptor.configurable
            + ":" + setDescriptor.writable + ":" + setDescriptor.enumerable
            + ":" + setDescriptor.configurable
        "#,
        "WeakMap:WeakSet:false:false:true:false:false:true",
    )
}
