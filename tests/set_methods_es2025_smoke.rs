use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn union_preserves_receiver_then_argument_order() -> TestResult {
    eval_is_42(
        r#"
        let result = new Set([3, 1, 2]).union(new Set([5, 4, 2]));
        let order = [];
        result.forEach(function (value) {
            order.push(value);
        });
        order.join("|") === "3|1|2|5|4" ? 42 : 0
        "#,
    )
}

#[test]
fn intersection_and_difference_over_both_branches() -> TestResult {
    eval_is_42(
        r#"
        function join(set) {
            let parts = [];
            set.forEach(function (value) {
                parts.push(value);
            });
            return parts.join("|");
        }
        join(new Set([1, 2, 3]).intersection(new Set([2, 3, 4, 5, 6]))) === "2|3" &&
            join(new Set([1, 2, 3, 4, 5, 6]).intersection(new Set([2, 4]))) === "2|4" &&
            join(new Set([1, 2, 3]).difference(new Set([2, 3, 4, 5, 6]))) === "1" &&
            join(new Set([1, 2, 3, 4, 5, 6]).difference(new Set([2, 4]))) === "1|3|5|6"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn symmetric_difference_matches_spec() -> TestResult {
    eval_is_42(
        r#"
        let result = new Set([1, 2, 3]).symmetricDifference(new Set([3, 4, 5]));
        let parts = [];
        result.forEach(function (value) {
            parts.push(value);
        });
        parts.join("|") === "1|2|4|5" ? 42 : 0
        "#,
    )
}

#[test]
fn set_composition_observes_iterator_setup_and_live_mutation_order() -> TestResult {
    eval_is_42(
        r#"
        function values(set) {
            let result = [];
            set.forEach(function (value) { result.push(value); });
            return result.join("|");
        }
        function clearingSetLike(receiver) {
            return {
                size: 0,
                has: function () { throw new Error("unexpected has"); },
                keys: function () {
                    return {
                        get next() {
                            receiver.clear();
                            receiver.add(4);
                            return function () { return { done: true }; };
                        }
                    };
                }
            };
        }
        let unionReceiver = new Set([1, 2, 3]);
        let union = unionReceiver.union(clearingSetLike(unionReceiver));
        let symmetricReceiver = new Set([1, 2, 3]);
        let symmetric = symmetricReceiver.symmetricDifference(
            clearingSetLike(symmetricReceiver)
        );
        let intersectionReceiver = new Set([1, 2, 3, 4]);
        let intersectionLike = {
            size: 0,
            has: function () { throw new Error("unexpected has"); },
            keys: function* () {
                yield* intersectionReceiver.keys();
                intersectionReceiver.clear();
            }
        };
        values(union) === "4" && values(symmetric) === "4" &&
            values(intersectionReceiver.intersection(intersectionLike)) === "1|2|3|4"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn predicate_methods_report_relationships() -> TestResult {
    eval_is_42(
        r"
        new Set([1, 2]).isSubsetOf(new Set([1, 2, 3])) === true &&
            new Set([1, 2, 3]).isSubsetOf(new Set([1, 2])) === false &&
            new Set([1, 2, 3]).isSupersetOf(new Set([2, 3])) === true &&
            new Set([1, 2]).isSupersetOf(new Set([1, 2, 3])) === false &&
            new Set([1, 2]).isDisjointFrom(new Set([3, 4])) === true &&
            new Set([1, 2]).isDisjointFrom(new Set([2, 3])) === false
            ? 42
            : 0
        ",
    )
}

#[test]
fn accepts_map_backed_set_like_arguments() -> TestResult {
    eval_is_42(
        r#"
        function join(set) {
            let parts = [];
            set.forEach(function (value) {
                parts.push(value);
            });
            return parts.join("|");
        }
        let setLike = new Map([[2, "b"], [4, "d"], [6, "f"]]);
        join(new Set([1, 2, 3]).union(setLike)) === "1|2|3|4|6" &&
            join(new Set([1, 2, 3, 4, 5, 6, 7]).intersection(setLike)) === "2|4|6" &&
            new Set([1, 2, 3, 4, 5, 6]).isSupersetOf(setLike) === true
            ? 42
            : 0
        "#,
    )
}

#[test]
fn normalizes_negative_zero_keys() -> TestResult {
    eval_is_42(
        r"
        let result = new Set([-0]).union(new Set([0]));
        let count = 0;
        result.forEach(function () {
            count += 1;
        });
        count === 1 && result.has(0) && result.has(-0) ? 42 : 0
        ",
    )
}

#[test]
fn validates_set_like_argument() -> TestResult {
    eval_is_42(
        r"
        let errors = 0;
        try {
            new Set([1]).union(42);
        } catch (error) {
            if (error instanceof TypeError) errors += 1;
        }
        try {
            new Set([1]).union({ size: NaN, has: function () {}, keys: function () {} });
        } catch (error) {
            if (error instanceof TypeError) errors += 1;
        }
        try {
            new Set([1]).union({ size: -1, has: function () {}, keys: function () {} });
        } catch (error) {
            if (error instanceof RangeError) errors += 1;
        }
        try {
            new Set([1]).union({ size: 1, has: 5, keys: function () {} });
        } catch (error) {
            if (error instanceof TypeError) errors += 1;
        }
        errors === 4 ? 42 : 0
        ",
    )
}

#[test]
fn exposes_method_metadata() -> TestResult {
    eval_is_42(
        r#"
        Set.prototype.union.length === 1 &&
            Set.prototype.intersection.length === 1 &&
            Set.prototype.difference.length === 1 &&
            Set.prototype.symmetricDifference.length === 1 &&
            Set.prototype.isSubsetOf.length === 1 &&
            Set.prototype.isSupersetOf.length === 1 &&
            Set.prototype.isDisjointFrom.length === 1 &&
            Set.prototype.union.name === "union" &&
            Set.prototype.symmetricDifference.name === "symmetricDifference" &&
            Set.prototype.isDisjointFrom.name === "isDisjointFrom"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn predicates_iterate_lazily_close_early_and_observe_receiver_mutation() -> TestResult {
    eval_is_42(
        r#"
        let receiver = new Set(["a", "b", "c"]);
        const mutating = {
            size: 3,
            has: function (value) {
                if (value === "a") {
                    receiver.delete("b");
                    receiver.delete("c");
                    receiver.add("b");
                }
                return false;
            },
            keys: function () { throw new Error("unexpected keys"); }
        };
        const disjoint = receiver.isDisjointFrom(mutating);
        let nextCalls = 0;
        let returnCalls = 0;
        const iterator = {
            next: function () {
                nextCalls += 1;
                return { value: 2, done: false };
            },
            return: function () {
                returnCalls += 1;
                return {};
            }
        };
        const setLike = {
            size: 1,
            has: function () { return false; },
            keys: function () { return iterator; }
        };
        disjoint && [...receiver].join("") === "ab" &&
            !new Set([3, 4]).isSupersetOf(setLike) &&
            nextCalls === 1 && returnCalls === 1
            ? 42
            : 0
        "#,
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
