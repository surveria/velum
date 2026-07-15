use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn ensure_string(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    if actual == Value::from(expected) {
        return Ok(());
    }
    Err(format!("expected string {expected:?}, got {actual:?}").into())
}

fn ensure_string_after_jobs(source: &str, expected: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)?;
    let actual = context.eval("result")?;
    if actual == Value::from(expected) {
        return Ok(());
    }
    Err(format!("expected string {expected:?}, got {actual:?}").into())
}

#[test]
fn using_declarations_dispose_resources_at_each_lexical_scope_exit() -> TestResult {
    ensure_string(
        r#"
        const seen = [];
        function run() {
            using outer = { [Symbol.dispose]() { seen.push("outer"); } };
            {
                using first = { [Symbol.dispose]() { seen.push("first"); } };
                using second = { [Symbol.dispose]() { seen.push("second"); } };
                seen.push("body");
            }
            seen.push("after-block");
        }
        run();
        seen.join(",")
        "#,
        "body,second,first,after-block,outer",
    )
}

#[test]
fn using_disposal_preserves_abrupt_completion_and_suppression_order() -> TestResult {
    ensure_string(
        r#"
        const first = {};
        const second = {};
        const body = {};
        try {
            using outer = { [Symbol.dispose]() { throw first; } };
            using inner = { [Symbol.dispose]() { throw second; } };
            throw body;
        } catch (error) {
            [
                error instanceof SuppressedError,
                error.error === first,
                error.suppressed instanceof SuppressedError,
                error.suppressed.error === second,
                error.suppressed.suppressed === body
            ].join(":");
        }
        "#,
        "true:true:true:true:true",
    )
}

#[test]
fn using_for_initializer_disposes_before_propagating_initializer_error() -> TestResult {
    ensure_string(
        r#"
        let disposed = false;
        const resource = { [Symbol.dispose]() { disposed = true; } };
        try {
            for (
                using first = resource, second = (() => { throw new Error("stop"); })();
                false;
            ) {}
        } catch (error) {
            [error.message, disposed].join(":");
        }
        "#,
        "stop:true",
    )
}

#[test]
fn await_using_suspends_function_completion_and_preserves_mixed_order() -> TestResult {
    ensure_string_after_jobs(
        r#"
        const seen = [];
        let result = "pending";
        async function run() {
            using first = { [Symbol.dispose]() { seen.push("first"); } };
            await using second = {
                [Symbol.asyncDispose]() {
                    seen.push("second");
                    return Promise.resolve();
                }
            };
            using third = { [Symbol.dispose]() { seen.push("third"); } };
            return 7;
        }
        run().then(value => { result = value + ":" + seen.join(","); });
        "#,
        "7:third,second,first",
    )
}

#[test]
fn await_using_disposes_nested_blocks_and_for_of_iterations_before_advancing() -> TestResult {
    ensure_string_after_jobs(
        r#"
        const seen = [];
        let result = "pending";
        async function run() {
            {
                await using block = {
                    [Symbol.asyncDispose]() { seen.push("block"); }
                };
                seen.push("inside");
            }
            for (await using item of [
                { id: 1, [Symbol.asyncDispose]() { seen.push("dispose-1"); } },
                { id: 2, [Symbol.asyncDispose]() { seen.push("dispose-2"); } }
            ]) {
                seen.push("body-" + item.id);
            }
        }
        run().then(() => { result = seen.join(","); });
        "#,
        "inside,block,body-1,dispose-1,body-2,dispose-2",
    )
}
