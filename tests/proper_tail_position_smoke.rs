use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_number(source: &str, expected: f64) -> TestResult {
    let actual = eval(source)?;
    if actual == Value::Number(expected) {
        return Ok(());
    }
    Err(format!("expected number {expected}, got {actual:?}").into())
}

#[test]
fn tail_position_flows_through_expression_structure() -> TestResult {
    ensure_number(
        r#"
        "use strict";
        function parenthesized(n) {
            if (n === 0) return 1;
            return (parenthesized(n - 1));
        }
        function sequence(n) {
            if (n === 0) return 1;
            return 0, sequence(n - 1);
        }
        function conditional(n) {
            if (n === 0) return 1;
            return true ? conditional(n - 1) : 20;
        }
        function logicalAnd(n) {
            if (n === 0) return 1;
            return true && logicalAnd(n - 1);
        }
        function logicalOr(n) {
            if (n === 0) return 1;
            return false || logicalOr(n - 1);
        }
        function nullish(n) {
            if (n === 0) return 1;
            return null ?? nullish(n - 1);
        }
        parenthesized(400) + sequence(400) + conditional(400)
            + logicalAnd(400) + logicalOr(400) + nullish(400);
        "#,
        6.0,
    )
}

#[test]
fn tail_position_short_circuits_return_the_left_value() -> TestResult {
    ensure_number(
        r#"
        "use strict";
        function forbidden() { throw new Error("must not run"); }
        function logicalAnd() { return 20 && false && forbidden(); }
        function logicalOr() { return 21 || forbidden(); }
        function nullish() { return 22 ?? forbidden(); }
        function conditional() { return false ? forbidden() : 23; }
        Number(logicalAnd()) + logicalOr() + nullish() + conditional();
        "#,
        66.0,
    )
}

#[test]
fn tail_member_calls_preserve_the_receiver() -> TestResult {
    ensure_number(
        r#"
        "use strict";
        const staticHolder = {
            base: 20,
            recurse(n) {
                if (n === 0) return this.base;
                return this.recurse(n - 1);
            }
        };
        const computedHolder = {
            base: 22,
            recurse(n) {
                if (n === 0) return this.base;
                return this["recurse"](n - 1);
            }
        };
        staticHolder.recurse(400) + computedHolder.recurse(400);
        "#,
        42.0,
    )
}

#[test]
fn shadowed_eval_tail_calls_and_intrinsic_eval_stays_direct() -> TestResult {
    ensure_number(
        r#"
        function shadowed() {
            function recurse(n) {
                "use strict";
                if (n === 0) return 20;
                return eval(n - 1);
            }
            var eval = recurse;
            return recurse(400);
        }
        function direct() {
            "use strict";
            const local = 22;
            return eval("local");
        }
        shadowed() + direct();
        "#,
        42.0,
    )
}
