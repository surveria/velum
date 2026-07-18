use velum::{Engine, Runtime, RuntimeLimits, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_flat_depth_and_sparse_sources() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let nested = [1, [2, [3, [4]]], 5];
        let flatDefault = nested.flat();
        let flatTwo = nested.flat(2);

        let sparse = Array(5);
        sparse[0] = [1, 2];
        sparse[2] = [3, [4]];
        sparse[4] = 5;
        let flatSparse = sparse.flat(2);

        flatDefault.length === 4 &&
            flatDefault[0] === 1 &&
            flatDefault[1] === 2 &&
            flatDefault[2][0] === 3 &&
            flatDefault[2][1][0] === 4 &&
            flatDefault[3] === 5 &&
            flatTwo.length === 5 &&
            flatTwo[0] === 1 &&
            flatTwo[1] === 2 &&
            flatTwo[2] === 3 &&
            flatTwo[3][0] === 4 &&
            flatTwo[4] === 5 &&
            flatSparse.length === 5 &&
            flatSparse.join("|") === "1|2|3|4|5" ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_array_flat_map_callbacks_and_generic_receivers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let values = [1, 2, 3];
        let thisArg = { scale: 10 };
        let mapped = values.flatMap(function(value, index, receiver) {
            return [value * this.scale, index, receiver === values];
        }, thisArg);

        let object = { 0: [1, [2]], 2: [3], length: 3 };
        let genericFlat = Array.prototype.flat.call(object, 2);
        let genericFlatMap = Array.prototype.flatMap.call(object, function(value, index) {
            return [index, value];
        });

        mapped.join("|") === "10|0|true|20|1|true|30|2|true" &&
            genericFlat.join("|") === "1|2|3" &&
            genericFlatMap.length === 4 &&
            genericFlatMap[0] === 0 &&
            genericFlatMap[1][0] === 1 &&
            genericFlatMap[1][1][0] === 2 &&
            genericFlatMap[2] === 2 &&
            genericFlatMap[3][0] === 3 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn honors_flat_species_and_proxy_array_identity() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        function Result(length) {
            this.initialLength = length;
        }
        const source = [[1], [2]];
        source.constructor = { [Symbol.species]: Result };
        const flat = source.flat();
        const mapped = source.flatMap(function (value) { return value; });

        let constructorReads = 0;
        const proxy = new Proxy([[3]], {
            get: function (target, property, receiver) {
                if (property === "constructor") {
                    constructorReads++;
                }
                return Reflect.get(target, property, receiver);
            }
        });
        const proxyFlat = proxy.flat();

        flat instanceof Result &&
            mapped instanceof Result &&
            flat.initialLength === 0 &&
            mapped.initialLength === 0 &&
            !Object.prototype.hasOwnProperty.call(flat, "length") &&
            !Object.prototype.hasOwnProperty.call(mapped, "length") &&
            flat[0] === 1 && flat[1] === 2 &&
            mapped[0] === 1 && mapped[1] === 2 &&
            constructorReads === 1 && proxyFlat[0] === 3 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_missing_flat_map_callbacks_and_limits_steps() -> TestResult {
    let runtime = Runtime::with_limits(RuntimeLimits {
        max_runtime_steps: 128,
        ..RuntimeLimits::default()
    });
    let mut context = runtime.context();

    let Err(callback_error) = context.eval("[1].flatMap();") else {
        return Err("expected Array.prototype.flatMap to reject a missing callback".into());
    };
    ensure_error_contains(&callback_error, "callback")?;

    let Err(limit_error) = context.eval("Array.prototype.flat.call({ length: 1000 });") else {
        return Err("expected Array.prototype.flat to hit runtime step limit".into());
    };
    ensure_error_contains(&limit_error, "runtime steps")
}

#[test]
fn guards_deep_flattening_of_self_referential_arrays() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        const self = [];
        self[0] = self;
        let selfCycleGuarded = false;
        try {
            self.flat(268435440);
        } catch (error) {
            selfCycleGuarded =
                error instanceof RangeError &&
                String(error.message).includes("Maximum call stack size exceeded");
        }

        const flat = [1508738142, 10, 8, 13, 0].flat(268435440);
        selfCycleGuarded &&
            flat.length === 5 &&
            flat[0] === 1508738142 &&
            flat[4] === 0 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn compiled_calls_mark_flat_and_flat_map_direct_targets() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r"
        let flat = [1, [2, [3]]].flat(2);
        let mapped = flat.flatMap(function(value) {
            return [value, value + 1];
        });
        mapped.length === 6 && mapped[5] === 4 ? 42 : 0
        ",
    )?;

    ensure_min_usize(script.usage().bytecode_direct_native_call_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_error_contains(error: &velum::Error, text: &str) -> TestResult {
    let message = error.to_string();
    if message.contains(text) {
        return Ok(());
    }

    Err(format!("expected error containing '{text}', got '{message}'").into())
}

fn ensure_min_usize(actual: usize, expected_minimum: usize) -> TestResult {
    if actual >= expected_minimum {
        return Ok(());
    }

    Err(format!("expected at least {expected_minimum}, got {actual}").into())
}
