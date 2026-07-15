use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn group_by_preserves_key_and_value_order() -> TestResult {
    expect_true(
        r#"
        const calls = [];
        const grouped = Map.groupBy([1, 2, 3, 4], function (value, index) {
            calls.push(value + ":" + index + ":" + arguments.length);
            return value % 2 ? "odd" : "even";
        });
        [...grouped.keys()].join("|") === "odd|even" &&
            grouped.get("odd").join("|") === "1|3" &&
            grouped.get("even").join("|") === "2|4" &&
            calls.join("|") === "1:0:2|2:1:2|3:2:2|4:3:2"
        "#,
    )
}

#[test]
fn group_by_normalizes_zero_and_closes_on_callback_error() -> TestResult {
    expect_true(
        r#"
        let closed = 0;
        const iterator = {
            [Symbol.iterator]: function () { return this; },
            next: function () { return { value: -0, done: false }; },
            return: function () { closed += 1; return {}; }
        };
        let threw = false;
        try {
            Map.groupBy(iterator, function () { throw new RangeError("stop"); });
        } catch (error) {
            threw = error instanceof RangeError;
        }
        const grouped = Map.groupBy([-0, 0], function (value) { return value; });
        threw && closed === 1 && grouped.size === 1 &&
            1 / [...grouped.keys()][0] === Infinity && grouped.get(0).length === 2
        "#,
    )
}

#[test]
fn get_or_insert_preserves_existing_values_and_inserts_missing_values() -> TestResult {
    expect_true(
        r#"
        const map = new Map([["present", 1]]);
        const object = {};
        map.getOrInsert("present", 2) === 1 &&
            map.getOrInsert("missing", 3) === 3 && map.get("missing") === 3 &&
            map.getOrInsert(-0, 4) === 4 && map.get(0) === 4 &&
            map.getOrInsert(object, 5) === 5 && map.get(object) === 5 &&
            Map.prototype.getOrInsert.name === "getOrInsert" &&
            Map.prototype.getOrInsert.length === 2 &&
            Map.groupBy.name === "groupBy" && Map.groupBy.length === 2
        "#,
    )
}

#[test]
fn get_or_insert_computed_observes_callback_contract_and_mutation() -> TestResult {
    expect_true(
        r#"
        const map = new Map([["present", 1]]);
        let calls = 0;
        const present = map.getOrInsertComputed("present", function () {
            calls += 1;
            return 2;
        });
        const inserted = map.getOrInsertComputed(-0, function (key) {
            "use strict";
            calls += 1;
            if (this !== undefined || arguments.length !== 1 || 1 / key !== Infinity) {
                return -1;
            }
            map.set(0, 6);
            return 7;
        });
        present === 1 && inserted === 7 && map.get(0) === 7 && calls === 1 &&
            Map.prototype.getOrInsertComputed.name === "getOrInsertComputed" &&
            Map.prototype.getOrInsertComputed.length === 2
        "#,
    )
}

fn expect_true(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!("expected true, got {value:?}").into())
}
