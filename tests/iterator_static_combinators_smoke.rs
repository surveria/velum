use velum::{Runtime, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let actual = eval(source)?;
    let expected = Value::from(expected);
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

#[test]
fn static_combinators_have_standard_function_metadata() -> TestResult {
    ensure_string(
        r#"
        const names = ["concat", "zip", "zipKeyed"];
        const metadata = names.map(name => {
            const fn = Iterator[name];
            const descriptor = Object.getOwnPropertyDescriptor(Iterator, name);
            let construct = "none";
            try { new fn([]); } catch (error) {
                construct = error instanceof TypeError ? "type" : "other";
            }
            return fn.name + ":" + fn.length + ":" + descriptor.enumerable + ":" + construct;
        });
        metadata.join("|")
        "#,
        "concat:0:false:type|zip:1:false:type|zipKeyed:1:false:type",
    )
}

#[test]
fn concat_opens_lazily_and_forwards_close_once() -> TestResult {
    ensure_string(
        r#"
        let opened = 0;
        let closed = 0;
        const iterable = {
            [Symbol.iterator]() {
                opened += 1;
                let index = 0;
                return {
                    next() {
                        index += 1;
                        return { value: index, done: index > 2 };
                    },
                    return() { closed += 1; return {}; }
                };
            }
        };
        const iterator = Iterator.concat(iterable, [3, 4]);
        const before = opened + ":" + closed;
        const first = iterator.next().value;
        iterator.return();
        iterator.return();
        before + ":" + first + ":" + opened + ":" + closed + ":" + iterator.next().done
        "#,
        "0:0:1:1:1:true",
    )
}

#[test]
fn zip_supports_all_modes_and_padding() -> TestResult {
    ensure_string(
        r#"
        const shortest = Array.from(Iterator.zip([[1, 2], [3]])).map(x => x.join(",")).join("|");
        const longest = Array.from(Iterator.zip([[1], [2, 3]], {
            mode: "longest",
            padding: ["a", "b"]
        })).map(x => x.join(",")).join("|");
        let strict = "none";
        try {
            Array.from(Iterator.zip([[1], [2, 3]], { mode: "strict" }));
        } catch (error) {
            strict = error instanceof TypeError ? "type" : "other";
        }
        shortest + ":" + longest + ":" + strict
        "#,
        "1,3:1,2|a,3:type",
    )
}

#[test]
fn zip_keyed_preserves_semantic_keys_and_result_shape() -> TestResult {
    ensure_string(
        r#"
        const symbol = Symbol("s");
        const input = { first: [1, 2] };
        input[symbol] = [3, 4];
        Object.defineProperty(input, "hidden", { enumerable: false, value: [5, 6] });
        input.skipped = undefined;
        const iterator = Iterator.zipKeyed(input);
        const value = iterator.next().value;
        const keys = Reflect.ownKeys(value);
        const descriptor = Object.getOwnPropertyDescriptor(value, "first");
        (Object.getPrototypeOf(value) === null) + ":" + keys.length + ":" +
            (keys[0] === "first") + ":" + (keys[1] === symbol) + ":" +
            value.first + ":" + value[symbol] + ":" + descriptor.writable + ":" +
            descriptor.enumerable + ":" + descriptor.configurable
        "#,
        "true:2:true:true:1:3:true:true:true",
    )
}

#[test]
fn zip_closes_open_iterators_in_reverse_order() -> TestResult {
    ensure_string(
        r#"
        const log = [];
        function iterator(name, values) {
            let index = 0;
            return {
                next() {
                    if (index < values.length) return { value: values[index++], done: false };
                    return { done: true };
                },
                return() { log.push(name); return {}; }
            };
        }
        const zipped = Iterator.zip([
            iterator("first", [1]),
            iterator("second", [2, 3]),
            iterator("third", [4, 5])
        ]);
        zipped.next();
        zipped.next();
        log.join(",")
        "#,
        "third,second",
    )
}
