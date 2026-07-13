use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn preserves_array_length_coercion_and_descriptor_order() -> TestResult {
    eval_is_42(
        r#"
        let array = [1, 2, 3];
        let coercions = 0;
        let length = {
            valueOf: function () {
                coercions++;
                if (coercions === 2) {
                    Object.defineProperty(array, "length", { writable: false });
                }
                return array.length;
            }
        };
        let defined = Reflect.defineProperty(array, "length", {
            value: length,
            writable: true
        });
        let accessorRejected = !Reflect.defineProperty([], "length", {
            set: function (_value) {}
        });
        coercions === 2 && !defined && accessorRejected ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_observable_array_method_operation_order() -> TestResult {
    eval_is_42(
        r#"
        let marker = {};
        let findLastLengthFirst = false;
        let receiver = {};
        Object.defineProperty(receiver, "length", {
            get: function () { throw marker; }
        });
        try {
            Array.prototype.findLast.call(receiver);
        } catch (error) {
            findLastLengthFirst = error === marker;
        }

        let joined = [1, 2, 3];
        let separator = {
            toString: function () {
                joined.length = 1;
                return ".";
            }
        };
        let joinedValue = joined.join(separator);

        let reversed = ["first", "second"];
        Object.defineProperty(reversed, "0", {
            configurable: true,
            get: function () {
                reversed.length = 0;
                return "first";
            }
        });
        reversed.reverse();

        findLastLengthFirst &&
            joinedValue === "1.." &&
            !("0" in reversed) &&
            reversed[1] === "first" ? 42 : 0
        "#,
    )
}

#[test]
fn copy_methods_reject_excessive_lengths_before_index_reads() -> TestResult {
    eval_is_42(
        r#"
        let reads = 0;
        let source = { length: 4294967296 };
        Object.defineProperty(source, "0", {
            get: function () {
                reads++;
                return 1;
            }
        });
        let rangeErrors = 0;
        let calls = [
            function () { Array.prototype.toReversed.call(source); },
            function () { Array.prototype.toSorted.call(source); },
            function () { Array.prototype.toSpliced.call(source); },
            function () { Array.prototype.with.call(source, 0, 1); }
        ];
        for (let call of calls) {
            try {
                call();
            } catch (error) {
                if (error instanceof RangeError) {
                    rangeErrors++;
                }
            }
        }
        rangeErrors === 4 && reads === 0 ? 42 : 0
        "#,
    )
}

#[test]
fn push_and_unshift_use_ecmascript_length_boundaries() -> TestResult {
    eval_is_42(
        r#"
        let array = [];
        array.length = 4294967295;
        let arrayRangeError = false;
        try {
            array.push("tail");
        } catch (error) {
            arrayRangeError = error instanceof RangeError;
        }

        let arrayLike = { length: 9007199254740991 };
        let typeErrors = 0;
        try {
            Array.prototype.push.call(arrayLike, null);
        } catch (error) {
            if (error instanceof TypeError) {
                typeErrors++;
            }
        }
        try {
            Array.prototype.unshift.call(arrayLike, null);
        } catch (error) {
            if (error instanceof TypeError) {
                typeErrors++;
            }
        }

        arrayRangeError &&
            array.length === 4294967295 &&
            array[4294967295] === "tail" &&
            typeErrors === 2 ? 42 : 0
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
