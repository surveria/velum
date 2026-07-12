use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn exposes_standard_array_iterator_methods_and_prototype() -> TestResult {
    eval_is_42(
        r#"
        let values = [1].values();
        let keys = [1].keys();
        let entries = [1].entries();
        let prototype = Object.getPrototypeOf(values);
        let nextDescriptor = Object.getOwnPropertyDescriptor(prototype, "next");
        let tagDescriptor = Object.getOwnPropertyDescriptor(prototype, Symbol.toStringTag);

        Array.prototype.values === Array.prototype[Symbol.iterator] &&
            Array.prototype.values.name === "values" && Array.prototype.values.length === 0 &&
            Array.prototype.keys.name === "keys" && Array.prototype.keys.length === 0 &&
            Array.prototype.entries.name === "entries" && Array.prototype.entries.length === 0 &&
            Object.getPrototypeOf(keys) === prototype &&
            Object.getPrototypeOf(entries) === prototype &&
            nextDescriptor.value.name === "next" && nextDescriptor.value.length === 0 &&
            nextDescriptor.writable && !nextDescriptor.enumerable && nextDescriptor.configurable &&
            tagDescriptor.value === "Array Iterator" && !tagDescriptor.writable &&
            !tagDescriptor.enumerable && tagDescriptor.configurable &&
            values[Symbol.iterator]() === values ? 42 : 0
        "#,
    )
}

#[test]
fn array_iterators_are_live_and_support_generic_receivers() -> TestResult {
    eval_is_42(
        r#"
        let valuesSource = [10];
        let values = valuesSource.values();
        let first = values.next();
        valuesSource.push(20);
        let second = values.next();
        let done = values.next();
        valuesSource.push(30);
        let stillDone = values.next();

        let generic = { length: 2, 0: "a" };
        let keys = Array.prototype.keys.call(generic);
        let key0 = keys.next();
        generic.length = 3;
        let key1 = keys.next();
        let key2 = keys.next();

        let entries = Array.prototype.entries.call({ length: 2, 1: "b" });
        let entry0 = entries.next().value;
        let entry1 = entries.next().value;

        first.value === 10 && !first.done &&
            second.value === 20 && !second.done && done.done && stillDone.done &&
            key0.value === 0 && key1.value === 1 && key2.value === 2 &&
            entry0[0] === 0 && entry0[1] === undefined &&
            entry1[0] === 1 && entry1[1] === "b" ? 42 : 0
        "#,
    )
}

#[test]
fn array_iterator_next_rejects_incompatible_receivers() -> TestResult {
    eval_is_42(
        r"
        let next = Object.getPrototypeOf([].values()).next;
        let failures = 0;
        for (let receiver of [undefined, null, {}, [].values().next]) {
            try {
                next.call(receiver);
            } catch (error) {
                if (error instanceof TypeError) {
                    failures = failures + 1;
                }
            }
        }
        failures === 4 ? 42 : 0
        ",
    )
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected value Number(42), got {value:?}").into())
}
