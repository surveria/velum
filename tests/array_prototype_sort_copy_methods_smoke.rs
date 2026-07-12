use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn sorts_with_default_and_custom_comparators() -> TestResult {
    eval_is_42(
        r#"
        let numeric = [3, 1, 2, 10];
        numeric.sort((a, b) => a - b);

        let lexicographic = [3, 1, 2, 10];
        lexicographic.sort();

        let holes = [3, 0, 1];
        delete holes[1];
        holes.sort();

        numeric.join("|") === "1|2|3|10" &&
            lexicographic.join("|") === "1|10|2|3" &&
            holes.length === 3 &&
            holes[0] === 1 &&
            holes[1] === 3 &&
            !("2" in holes)
            ? 42
            : 0
        "#,
    )
}

#[test]
fn sort_orders_undefined_after_values_and_holes_last() -> TestResult {
    eval_is_42(
        r#"
        let values = ["b", undefined, "a"];
        values.sort();
        values.length === 3 &&
            values[0] === "a" &&
            values[1] === "b" &&
            values[2] === undefined
            ? 42
            : 0
        "#,
    )
}

#[test]
fn sort_is_stable_and_rejects_non_callable_comparators() -> TestResult {
    eval_is_42(
        r"
        let input = [
            { key: 1, order: 0 },
            { key: 0, order: 1 },
            { key: 1, order: 2 },
            { key: 0, order: 3 },
        ];
        input.sort((a, b) => a.key - b.key);
        let stable =
            input[0].order === 1 &&
            input[1].order === 3 &&
            input[2].order === 0 &&
            input[3].order === 2;

        let typeError = false;
        try {
            [1, 2].sort(42);
        } catch (error) {
            typeError = error instanceof TypeError;
        }

        stable && typeError ? 42 : 0
        ",
    )
}

#[test]
fn numeric_sort_declines_packed_fast_paths_for_nan_values() -> TestResult {
    eval_is_42(
        r"
        let values = [];
        let expectedSum = 0;
        for (let index = 0; index < 128; index = index + 1) {
            let value = index % 7 === 0 ? NaN : 128 - index;
            values.push(value);
            if (!Number.isNaN(value)) {
                expectedSum = expectedSum + value;
            }
        }
        let source = values.slice();
        let sortedCopy = source.toSorted((left, right) => left - right);
        values.sort((left, right) => left - right);

        let actualSum = 0;
        let nanCount = 0;
        for (let value of values) {
            if (Number.isNaN(value)) {
                nanCount = nanCount + 1;
            } else {
                actualSum = actualSum + value;
            }
        }
        let copyNanCount = 0;
        for (let value of sortedCopy) {
            if (Number.isNaN(value)) {
                copyNanCount = copyNanCount + 1;
            }
        }

        values.length === 128 &&
            sortedCopy.length === 128 &&
            source.length === 128 &&
            nanCount === 19 &&
            copyNanCount === 19 &&
            actualSum === expectedSum
            ? 42
            : 0
        ",
    )
}

#[test]
fn splices_with_deletion_insertion_and_growth() -> TestResult {
    eval_is_42(
        r#"
        let replace = [1, 2, 3, 4, 5];
        let removed = replace.splice(1, 2, "a", "b", "c");

        let grow = [1, 2, 3];
        grow.splice(1, 0, "x");

        let shrink = [1, 2, 3, 4, 5];
        shrink.splice(1, 3);

        let negative = [1, 2, 3, 4];
        let negativeRemoved = negative.splice(-2);

        replace.join("|") === "1|a|b|c|4|5" &&
            removed.join("|") === "2|3" &&
            grow.join("|") === "1|x|2|3" &&
            shrink.join("|") === "1|5" &&
            negative.join("|") === "1|2" &&
            negativeRemoved.join("|") === "3|4"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn splice_honors_species_results_and_operation_order() -> TestResult {
    eval_is_42(
        r#"
        let log = "";
        let values = [1, , 3, 4];
        Object.defineProperty(values, "constructor", {
            get: function() {
                log = log + "constructor;";
                return {
                    get [Symbol.species]() {
                        log = log + "species;";
                        return function Result(length) {
                            log = log + "construct:" + length + ";";
                            return { kind: "splice" };
                        };
                    }
                };
            }
        });
        let start = { valueOf: function() { log = log + "start;"; return 1; } };
        let count = { valueOf: function() { log = log + "count;"; return 2; } };

        let removed = values.splice(start, count, "x");
        let descriptor = Object.getOwnPropertyDescriptor(removed, "1");

        log === "start;count;constructor;species;construct:2;" &&
            removed.kind === "splice" &&
            removed.length === 2 &&
            !("0" in removed) && removed[1] === 3 &&
            descriptor.writable && descriptor.enumerable && descriptor.configurable &&
            values.join("|") === "1|x|4" ? 42 : 0
        "#,
    )
}

#[test]
fn splice_result_failures_precede_source_mutation() -> TestResult {
    eval_is_42(
        r#"
        let values = [1, 2, 3];
        function ClosedResult() {
            return Object.preventExtensions({});
        }
        values.constructor = { [Symbol.species]: ClosedResult };

        let failed = false;
        try {
            values.splice(1, 1, "x");
        } catch (error) {
            failed = error instanceof TypeError;
        }

        let limitFailed = false;
        try {
            Array.prototype.splice.call({ length: 9007199254740991 }, 0, 0, null);
        } catch (error) {
            limitFailed = error instanceof TypeError;
        }

        failed && limitFailed &&
            values.length === 3 && values.join("|") === "1|2|3" ? 42 : 0
        "#,
    )
}

#[test]
fn fills_and_copies_within_with_relative_bounds() -> TestResult {
    eval_is_42(
        r#"
        [1, 2, 3, 4].fill(0, 1, 3).join("|") === "1|0|0|4" &&
            [1, 2, 3, 4, 5].fill(9, -2).join("|") === "1|2|3|9|9" &&
            [1, 2, 3, 4, 5].copyWithin(0, 3).join("|") === "4|5|3|4|5" &&
            [1, 2, 3, 4, 5].copyWithin(1, 0, 3).join("|") === "1|1|2|3|5" &&
            [1, 2, 3, 4, 5].copyWithin(-2, -3, -1).join("|") === "1|2|3|3|4"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn at_resolves_relative_indices() -> TestResult {
    eval_is_42(
        r"
        let values = [10, 20, 30];
        values.at(0) === 10 &&
            values.at(2) === 30 &&
            values.at(-1) === 30 &&
            values.at(-3) === 10 &&
            values.at(3) === undefined &&
            values.at(-4) === undefined
            ? 42
            : 0
        ",
    )
}

#[test]
fn find_last_scans_in_reverse() -> TestResult {
    eval_is_42(
        r"
        let values = [1, 2, 3, 4, 5];
        values.findLast((value) => value % 2 === 0) === 4 &&
            values.findLastIndex((value) => value % 2 === 0) === 3 &&
            [1, 3, 5].findLast((value) => value % 2 === 0) === undefined &&
            [1, 3, 5].findLastIndex((value) => value % 2 === 0) === -1
            ? 42
            : 0
        ",
    )
}

#[test]
fn change_by_copy_methods_do_not_mutate_source() -> TestResult {
    eval_is_42(
        r#"
        let sortSource = [3, 1, 2];
        let sorted = sortSource.toSorted((a, b) => a - b);

        let reverseSource = [1, 2, 3];
        let reversed = reverseSource.toReversed();

        let spliceSource = [1, 2, 3, 4];
        let spliced = spliceSource.toSpliced(1, 2, "x", "y");

        let withSource = [1, 2, 3];
        let replaced = withSource.with(1, 9);

        sorted.join("|") === "1|2|3" &&
            sortSource.join("|") === "3|1|2" &&
            reversed.join("|") === "3|2|1" &&
            reverseSource.join("|") === "1|2|3" &&
            spliced.join("|") === "1|x|y|4" &&
            spliceSource.join("|") === "1|2|3|4" &&
            replaced.join("|") === "1|9|3" &&
            withSource.join("|") === "1|2|3"
            ? 42
            : 0
        "#,
    )
}

#[test]
fn with_throws_range_error_on_out_of_bounds() -> TestResult {
    eval_is_42(
        r"
        let threw = 0;
        try {
            [1, 2, 3].with(3, 0);
        } catch (error) {
            if (error instanceof RangeError) {
                threw += 1;
            }
        }
        try {
            [1, 2, 3].with(-4, 0);
        } catch (error) {
            if (error instanceof RangeError) {
                threw += 1;
            }
        }
        threw === 2 ? 42 : 0
        ",
    )
}

#[test]
fn operates_on_generic_array_like_receivers() -> TestResult {
    eval_is_42(
        r#"
        let target = { length: 3, 0: "z", 1: "a", 2: "m" };
        Array.prototype.sort.call(target);

        let spliceTarget = { length: 3, 0: 1, 1: 2, 2: 3 };
        let removed = Array.prototype.splice.call(spliceTarget, 1, 1, "x");

        target[0] === "a" &&
            target[1] === "m" &&
            target[2] === "z" &&
            spliceTarget.length === 3 &&
            spliceTarget[1] === "x" &&
            removed.length === 1 &&
            removed[0] === 2
            ? 42
            : 0
        "#,
    )
}

#[test]
fn exposes_method_metadata() -> TestResult {
    eval_is_42(
        r#"
        Array.prototype.sort.length === 1 &&
            Array.prototype.splice.length === 2 &&
            Array.prototype.fill.length === 1 &&
            Array.prototype.copyWithin.length === 2 &&
            Array.prototype.at.length === 1 &&
            Array.prototype.findLast.length === 1 &&
            Array.prototype.findLastIndex.length === 1 &&
            Array.prototype.toSorted.length === 1 &&
            Array.prototype.toReversed.length === 0 &&
            Array.prototype.toSpliced.length === 2 &&
            Array.prototype.with.length === 2 &&
            Array.prototype.sort.name === "sort" &&
            Array.prototype.copyWithin.name === "copyWithin" &&
            Array.prototype.toSpliced.name === "toSpliced"
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
