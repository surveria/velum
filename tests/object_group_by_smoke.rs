use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn group_by_creates_null_prototype_groups_with_data_properties() -> TestResult {
    expect_true(
        r#"
        const grouped = Object.groupBy([1, 2, 3, 4], function (value, index) {
            return index % 2 ? "even-index" : "odd-index";
        });
        const descriptor = Object.getOwnPropertyDescriptor(grouped, "odd-index");
        Object.getPrototypeOf(grouped) === null &&
            grouped["odd-index"].join("|") === "1|3" &&
            grouped["even-index"].join("|") === "2|4" &&
            descriptor.writable && descriptor.enumerable && descriptor.configurable &&
            Object.groupBy.name === "groupBy" && Object.groupBy.length === 2
        "#,
    )
}

#[test]
fn group_by_uses_property_keys_and_preserves_symbol_identity() -> TestResult {
    expect_true(
        r#"
        const symbol = Symbol("group");
        const keyCalls = [];
        const key = {
            [Symbol.toPrimitive]: function (hint) {
                keyCalls.push(hint);
                return "coerced";
            }
        };
        const grouped = Object.groupBy([1, 2, 3], function (value) {
            if (value === 1) return symbol;
            return key;
        });
        grouped[symbol].join("|") === "1" &&
            grouped.coerced.join("|") === "2|3" &&
            Object.getOwnPropertySymbols(grouped)[0] === symbol &&
            keyCalls.join("|") === "string|string"
        "#,
    )
}

#[test]
fn group_by_defines_proto_as_an_own_data_property() -> TestResult {
    expect_true(
        r#"
        const grouped = Object.groupBy([1], function () { return "__proto__"; });
        const descriptor = Object.getOwnPropertyDescriptor(grouped, "__proto__");
        Object.getPrototypeOf(grouped) === null &&
            descriptor.value === grouped["__proto__"] &&
            descriptor.value.join("|") === "1"
        "#,
    )
}

#[test]
fn group_by_closes_iterator_on_callback_and_key_conversion_errors() -> TestResult {
    expect_true(
        r#"
        function closingIterator() {
            let closed = false;
            return {
                state: function () { return closed; },
                iterable: {
                    [Symbol.iterator]: function () { return this; },
                    next: function () { return { value: 1, done: false }; },
                    return: function () { closed = true; return {}; }
                }
            };
        }
        const callbackCase = closingIterator();
        let callbackThrew = false;
        try {
            Object.groupBy(callbackCase.iterable, function () { throw new RangeError("stop"); });
        } catch (error) {
            callbackThrew = error instanceof RangeError;
        }
        const keyCase = closingIterator();
        let keyThrew = false;
        try {
            Object.groupBy(keyCase.iterable, function () {
                return { [Symbol.toPrimitive]: function () { throw new TypeError("key"); } };
            });
        } catch (error) {
            keyThrew = error instanceof TypeError;
        }
        callbackThrew && callbackCase.state() && keyThrew && keyCase.state()
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
