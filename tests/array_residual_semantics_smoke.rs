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

#[test]
fn mutating_array_fast_paths_respect_index_descriptors() -> TestResult {
    eval_is_42(
        r#"
        let typeErrors = 0;
        let calls = [
            function () {
                let array = [1, 2, 3];
                Object.defineProperty(array, "2", { configurable: false });
                array.pop();
            },
            function () {
                let array = [1, 2, 3];
                Object.defineProperty(array, "0", { writable: false });
                array.reverse();
            },
            function () {
                let array = [1, 2, 3];
                Object.defineProperty(array, "0", { writable: false });
                array.shift();
            },
            function () {
                let array = [1, 2, 3];
                Object.defineProperty(array, "0", { writable: false });
                array.unshift(0);
            }
        ];
        for (let call of calls) {
            try {
                call();
            } catch (error) {
                if (error instanceof TypeError) {
                    typeErrors++;
                }
            }
        }
        typeErrors === calls.length ? 42 : 0
        "#,
    )
}

#[test]
fn generic_array_mutations_define_existing_proxy_properties_with_value_only() -> TestResult {
    eval_is_42(
        r#"
        let array = [1, 2];
        let descriptors = [];
        let proxy = new Proxy(array, {
            defineProperty(target, key, descriptor) {
                descriptors.push(Object.keys(descriptor).join(","));
                return Reflect.defineProperty(target, key, descriptor);
            }
        });
        let popped = Array.prototype.pop.call(proxy);
        let shifted = Array.prototype.shift.call(proxy);
        popped === 2 && shifted === 1 && array.length === 0 &&
            descriptors.every(function (keys) { return keys === "value"; }) ? 42 : 0
        "#,
    )
}

#[test]
fn array_index_assignment_routes_through_a_proxy_prototype_set_trap() -> TestResult {
    eval_is_42(
        r#"
        var receiver;
        var observed;
        var prototype = new Proxy({}, {
            set: function (target, property, value, actualReceiver) {
                observed = [target, property, value, actualReceiver];
                return true;
            }
        });
        receiver = new Array(1);
        Object.setPrototypeOf(receiver, prototype);
        receiver[0] = 1;
        observed[0] !== receiver && observed[1] === "0" &&
            observed[2] === 1 && observed[3] === receiver ? 42 : 0
        "#,
    )
}

#[test]
fn array_unscopables_lists_at_and_blocks_it_in_with_environments() -> TestResult {
    eval_is_42(
        r#"
        let unscopables = Array.prototype[Symbol.unscopables];
        let at = 42;
        let observed;
        with (Array.prototype) {
            observed = at;
        }
        unscopables.at === true &&
            Reflect.ownKeys(unscopables)[0] === "at" &&
            observed === 42 ? 42 : 0
        "#,
    )
}

#[test]
fn array_searches_leave_fast_paths_before_observable_bound_coercion() -> TestResult {
    eval_is_42(
        r#"
        function search(method, values, start) {
            let log = [];
            Object.setPrototypeOf(values, new Proxy(Array.prototype, {
                has: function (target, key) {
                    log.push(String(key));
                    return Reflect.has(target, key);
                }
            }));
            Array.prototype[method].call(values, 100, {
                valueOf: function () {
                    values.length = 0;
                    return start;
                }
            });
            return log.join(",");
        }
        search("indexOf", [1, 2, 3], 0) === "0,1,2" &&
            search("lastIndexOf", [1, 2, 3], 2) === "2,1,0" ? 42 : 0
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
