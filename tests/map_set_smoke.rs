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
fn map_stores_and_reads_entries() -> TestResult {
    ensure_string(
        r#"
        const m = new Map();
        m.set("a", 1).set("b", 2);
        "" + m.size + ":" + m.get("a") + ":" + m.get("b")
            + ":" + (m.get("missing") === undefined)
            + ":" + m.has("a") + ":" + m.has("missing")
        "#,
        "2:1:2:true:true:false",
    )
}

#[test]
fn map_constructor_seeds_from_iterable_pairs() -> TestResult {
    ensure_string(
        r#"
        const m = new Map([["x", 10], ["y", 20]]);
        "" + m.size + ":" + m.get("x") + ":" + m.get("y")
        "#,
        "2:10:20",
    )
}

#[test]
fn map_keys_use_same_value_zero() -> TestResult {
    ensure_string(
        r#"
        const m = new Map();
        m.set(NaN, "nan");
        m.set(0, "zero");
        m.set(-0, "negzero");
        "" + m.size + ":" + m.get(NaN) + ":" + m.get(0)
        "#,
        "2:nan:negzero",
    )
}

#[test]
fn map_object_keys_compare_by_identity() -> TestResult {
    ensure_string(
        r#"
        const m = new Map();
        const key = {};
        m.set(key, "held");
        "" + m.get(key) + ":" + (m.get({}) === undefined)
            + ":" + m.delete(key) + ":" + m.size + ":" + m.delete(key)
        "#,
        "held:true:true:0:false",
    )
}

#[test]
fn set_deduplicates_and_mutates() -> TestResult {
    ensure_string(
        r#"
        const s = new Set([1, 2, 2, 3]);
        const initial = s.size;
        s.add(4).add(1);
        const grown = s.size;
        s.delete(1);
        "" + initial + ":" + grown + ":" + s.size + ":" + s.has(2) + ":" + s.has(9)
        "#,
        "3:4:3:true:false",
    )
}

#[test]
fn for_each_visits_entries_in_insertion_order() -> TestResult {
    ensure_string(
        r##"
        const m = new Map([["a", 1], ["b", 2]]);
        let out = "";
        m.forEach(function (value, key, map) {
            out = out + key + value + (map === m ? "!" : "?");
        });
        const s = new Set(["x", "y"]);
        s.forEach(function (value, key) {
            out = out + value + (value === key ? "=" : "#");
        });
        out
        "##,
        "a1!b2!x=y=",
    )
}

#[test]
fn collections_iterate_with_for_of_and_spread() -> TestResult {
    ensure_string(
        r#"
        const m = new Map([["k1", "v1"], ["k2", "v2"]]);
        let acc = "";
        for (const pair of m) {
            acc = acc + pair[0] + "=" + pair[1] + ";";
        }
        const s = new Set(["p", "q"]);
        for (const v of s) {
            acc = acc + v;
        }
        acc + ":" + [...s].length + ":" + [...m.keys()].join("|")
        "#,
        "k1=v1;k2=v2;pq:2:k1|k2",
    )
}

#[test]
fn iterator_protocol_reports_done() -> TestResult {
    ensure_string(
        r#"
        const it = new Map([["a", 1]]).entries();
        const first = it.next();
        const second = it.next();
        "" + first.value[0] + ":" + first.value[1] + ":" + first.done
            + ":" + (second.value === undefined) + ":" + second.done
        "#,
        "a:1:false:true:true",
    )
}

#[test]
fn constructors_require_new_and_validate_entries() -> TestResult {
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
        kind(function () { Map(); })
            + ":" + kind(function () { Set(); })
            + ":" + kind(function () { new Map([1]); })
        "#,
        "TypeError:TypeError:TypeError",
    )
}

#[test]
fn prototype_identity_and_instanceof_hold() -> TestResult {
    ensure_string(
        r#"
        const m = new Map();
        const s = new Set();
        "" + (Object.getPrototypeOf(m) === Map.prototype)
            + ":" + (m instanceof Map)
            + ":" + (s instanceof Set)
            + ":" + (s instanceof Map)
            + ":" + (new Set().keys === new Set().values)
        "#,
        "true:true:true:false:true",
    )
}

#[test]
fn clear_empties_collections() -> TestResult {
    ensure_string(
        r#"
        const m = new Map([["a", 1]]);
        const s = new Set("abc");
        m.clear();
        s.clear();
        "" + m.size + ":" + s.size + ":" + (m.get("a") === undefined)
        "#,
        "0:0:true",
    )
}

#[test]
fn set_seeds_from_string_iterables() -> TestResult {
    ensure_string(
        r#"
        const s = new Set("aba");
        "" + s.size + ":" + s.has("a") + ":" + s.has("b") + ":" + s.has("c")
        "#,
        "2:true:true:false",
    )
}

#[test]
fn set_constructor_observes_the_add_protocol() -> TestResult {
    ensure_string(
        r#"
        const original = Set.prototype.add;
        let calls = 0;
        Set.prototype.add = function (value) {
            calls += 1;
            return original.call(this, value);
        };
        const seeded = new Set([1, 2]);
        Object.defineProperty(Set.prototype, "add", {
            get: function () { throw new RangeError("adder"); }
        });
        let error = "none";
        try { new Set([]); } catch (caught) { error = caught.name; }
        calls + ":" + seeded.size + ":" + error
        "#,
        "2:2:RangeError",
    )
}

#[test]
fn set_iteration_is_live_branded_and_metadata_complete() -> TestResult {
    ensure_string(
        r#"
        const set = new Set([1, 2]);
        const iterator = set.values();
        const first = iterator.next().value;
        set.delete(2);
        set.add(2);
        set.add(3);
        const rest = [iterator.next().value, iterator.next().value].join("");
        const done = iterator.next().done;
        set.add(4);
        const remainsDone = iterator.next().done;
        let rejected = false;
        try { iterator.next.call({}); } catch (error) {
            rejected = error instanceof TypeError;
        }
        const otherAccepted = iterator.next.call(new Set().values()).done;
        const prototype = Object.getPrototypeOf(iterator);
        first + ":" + rest + ":" + done + ":" + remainsDone + ":" + rejected +
            ":" + otherAccepted + ":" + prototype[Symbol.toStringTag] +
            ":" + Set.prototype[Symbol.toStringTag] +
            ":" + Object.getOwnPropertyDescriptor(Set.prototype, "size").get.name
        "#,
        "1:23:true:true:true:true:Set Iterator:Set:get size",
    )
}

#[test]
fn set_for_each_observes_deletion_readdition_and_growth() -> TestResult {
    ensure_string(
        r#"
        const set = new Set([1, 2, 3]);
        const seen = [];
        set.forEach(function (value) {
            seen.push(value);
            if (value === 1) {
                set.delete(2);
                set.delete(3);
                set.add(2);
                set.add(4);
            }
        });
        seen.join("") + ":" + [...set].join("")
        "#,
        "124:124",
    )
}
