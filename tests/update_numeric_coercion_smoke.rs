use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn updates_bindings_with_single_to_number_conversion() -> TestResult {
    expect_true(
        r#"
        let text = "41";
        let previous = text++;
        let boolean = true;
        let prefixed = ++boolean;
        let nil = null;
        let zero = nil--;
        let missing;
        ++missing;
        let calls = 0;
        let object = {
            valueOf() {
                calls += 1;
                return "7";
            }
        };
        let objectPrevious = object++;
        previous === 41 && text === 42 &&
            prefixed === 2 && boolean === 2 &&
            zero === 0 && nil === -1 &&
            Number.isNaN(missing) &&
            objectPrevious === 7 && object === 8 && calls === 1
        "#,
    )
}

#[test]
fn updates_property_array_private_and_super_references() -> TestResult {
    expect_true(
        r#"
        let object = { staticValue: "9", computedValue: false };
        let staticPrevious = object.staticValue++;
        let keyCalls = 0;
        function key() {
            keyCalls += 1;
            return "computedValue";
        }
        let computed = ++object[key()];
        let items = ["12"];
        let arrayPrevious = items[0]++;

        class Base {}
        Base.prototype.value = "20";
        class Derived extends Base {
            update() {
                return super.value++;
            }
        }
        let derived = new Derived();
        let superPrevious = derived.update();

        class PrivateCounter {
            #value = "30";
            update() {
                let previous = this.#value++;
                return previous === 30 && this.#value === 31;
            }
        }

        staticPrevious === 9 && object.staticValue === 10 &&
            computed === 1 && object.computedValue === 1 && keyCalls === 1 &&
            arrayPrevious === 12 && items[0] === 13 &&
            superPrevious === 20 && derived.value === 21 &&
            new PrivateCounter().update()
        "#,
    )
}

#[test]
fn update_conversion_errors_are_catchable_and_do_not_store() -> TestResult {
    expect_true(
        r#"
        let symbol = Symbol("counter");
        let symbolError = false;
        try {
            symbol++;
        } catch (error) {
            symbolError = error.constructor === TypeError;
        }

        let calls = 0;
        let object = {
            [Symbol.toPrimitive]() {
                calls += 1;
                return {};
            }
        };
        let original = object;
        let objectError = false;
        try {
            ++object;
        } catch (error) {
            objectError = error.constructor === TypeError;
        }

        symbolError && objectError && object === original && calls === 1
        "#,
    )
}

#[test]
fn computed_update_checks_nullish_base_before_property_key_conversion() -> TestResult {
    expect_true(
        r#"
        let expressionCalls = 0;
        function propertyExpression() {
            expressionCalls += 1;
            return {
                toString() {
                    throw new Error("property key conversion must not run");
                }
            };
        }

        let errorIsTypeError = false;
        try {
            null[propertyExpression()]++;
        } catch (error) {
            errorIsTypeError = error.constructor === TypeError;
        }

        errorIsTypeError && expressionCalls === 1
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
