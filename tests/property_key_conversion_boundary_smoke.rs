use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn routes_dynamic_property_consumers_through_to_property_key() -> TestResult {
    eval_is_42(
        r#"
        let hints = "";
        let key = {};
        key[Symbol.toPrimitive] = function (hint) {
            hints = hints + hint;
            return "answer";
        };
        let holder = { answer: 40 };
        holder[key] += 1;
        let present = key in holder;
        let own = Object.hasOwn(holder, key);
        let reflected = Reflect.get(holder, key);
        let removed = delete holder[key];
        present && own && reflected === 41 && removed && hints === "stringstringstringstringstring"
            ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_symbol_property_key_identity() -> TestResult {
    eval_is_42(
        r#"
        let symbol = Symbol("key");
        let key = {};
        key[Symbol.toPrimitive] = function (hint) {
            return hint === "string" ? symbol : "wrong";
        };
        let holder = {};
        holder[key] = 42;
        holder[symbol] === 42 && Reflect.get(holder, key) === 42 ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_ordinary_property_key_conversion_order() -> TestResult {
    eval_is_42(
        r#"
        let order = "";
        let key = {};
        key.toString = function () {
            order = order + "s";
            return {};
        };
        key.valueOf = function () {
            order = order + "v";
            return "answer";
        };
        let holder = { answer: 42 };
        holder[key] === 42 && order === "sv" ? 42 : 0
        "#,
    )
}

#[test]
fn rejects_non_primitive_property_keys() -> TestResult {
    eval_is_42(
        r#"
        let score = 40;
        let invalid = {};
        invalid[Symbol.toPrimitive] = function () { return {}; };
        try {
            ({})[invalid];
        } catch (error) {
            score = score + 1;
        }
        let abrupt = {};
        abrupt.toString = function () { throw Error("key failure"); };
        try {
            Reflect.get({}, abrupt);
        } catch (error) {
            score = score + (error.message === "key failure" ? 1 : 0);
        }
        score
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
