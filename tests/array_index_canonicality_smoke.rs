use velum::{OptimizationMode, Value, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn canonical_array_indices_preserve_length_and_sparse_boundaries() -> TestResult {
    eval_true_in_both_modes(
        "canonical array indices",
        r#"
        (() => {
            const indices = ["0", "1", "9", "10", "99", "100", "4294967294"];
            for (const key of indices) {
                const array = [];
                if (!Reflect.set(array, key, 42)) return false;
                if (array.length !== Number(key) + 1 || array[key] !== 42) return false;
                const descriptor = Object.getOwnPropertyDescriptor(array, key);
                if (descriptor.value !== 42 || !descriptor.enumerable) return false;
                if (Object.keys(array).join(",") !== key) return false;
                array.length = 0;
                if (Object.hasOwn(array, key)) return false;
            }
            const array = [];
            Object.defineProperty(array, "4294967294", { value: 7, configurable: true });
            Object.defineProperty(array, "4294967295", { value: 9, configurable: true });
            if (array.length !== 4294967295) return false;
            array.length = 0;
            return !Object.hasOwn(array, "4294967294") && array["4294967295"] === 9;
        })()
        "#,
    )
}

#[test]
fn noncanonical_array_keys_remain_distinct_named_properties() -> TestResult {
    eval_true_in_both_modes(
        "noncanonical array keys",
        r#"
        (() => {
            const names = [
                "", "00", "01", "000000000000000000000000000000000001",
                "+0", "+1", "-0", "-1", " 1", "1 ", "\t1", "1\n", "\u00a01",
                "1.0", "1.5", "1e0", "0x1", "1_0", "1\0", "\u0661", "\uff11",
                "4294967295", "4294967296", "18446744073709551616",
                "99999999999999999999999999999999999999999999999999999999"
            ];
            const array = [7, 11];
            for (let index = 0; index < names.length; index++) {
                const key = names[index];
                if (!Reflect.set(array, key, index + 100)) return false;
                if (array.length !== 2 || array[0] !== 7 || array[1] !== 11) return false;
                if (!Object.hasOwn(array, key) || array[key] !== index + 100) return false;
            }
            if (Object.keys(array).join("|") !== ["0", "1"].concat(names).join("|")) return false;
            for (const key of names) {
                if (!Reflect.deleteProperty(array, key) || Object.hasOwn(array, key)) return false;
            }
            return array.length === 2 && Object.keys(array).join(",") === "0,1";
        })()
        "#,
    )
}

#[test]
fn ordinary_own_key_order_distinguishes_indices_from_similar_names() -> TestResult {
    eval_true_in_both_modes(
        "ordinary object numeric key ordering",
        r#"
        (() => {
            const object = Object.create(null);
            const names = ["4294967295", "+1", "10", "01", "0", "4294967294", "2", "\u0661", "1 "];
            for (let index = 0; index < names.length; index++) {
                Object.defineProperty(object, names[index], {
                    value: index, enumerable: true, configurable: true, writable: true
                });
            }
            const expected = ["0", "2", "10", "4294967294", "4294967295", "+1", "01", "\u0661", "1 "];
            if (Reflect.ownKeys(object).join("|") !== expected.join("|")) return false;
            for (let index = 0; index < names.length; index++) {
                if (Reflect.get(object, names[index]) !== index) return false;
            }
            delete object["01"];
            object["01"] = 42;
            expected.splice(expected.indexOf("01"), 1);
            expected.push("01");
            return Object.keys(object).join("|") === expected.join("|");
        })()
        "#,
    )
}

#[test]
fn typed_arrays_preserve_canonical_numeric_index_string_semantics() -> TestResult {
    eval_true_in_both_modes(
        "typed array numeric keys",
        r#"
        (() => {
            const array = new Uint8Array([7, 11]);
            const invalid = ["-0", "-1", "2", "1.5", "4294967294", "4294967295", "4294967296", "NaN", "Infinity"];
            for (const key of invalid) {
                if (!Reflect.set(array, key, 42) || Object.hasOwn(array, key)) return false;
                if (Reflect.defineProperty(array, key, { value: 42 })) return false;
            }
            const names = ["", "00", "01", "+0", "+1", " 1", "1 ", "1.0", "1e0", "0x1", "\u0661", "\uff11", "1\0"];
            for (const key of names) {
                if (!Reflect.defineProperty(array, key, { value: 42, configurable: true })) return false;
                if (!Object.hasOwn(array, key) || Reflect.get(array, key) !== 42) return false;
            }
            array["0"] = 13;
            array["1"] = 17;
            return array.length === 2 && array[0] === 13 && array[1] === 17;
        })()
        "#,
    )
}

#[test]
fn proxy_traps_observe_exact_property_keys_before_array_index_handling() -> TestResult {
    eval_true_in_both_modes(
        "proxy numeric key forwarding",
        r#"
        (() => {
            const seen = [];
            const target = [];
            const proxy = new Proxy(target, {
                set(object, key, value) { seen.push("set:" + key); return Reflect.set(object, key, value, object); },
                get(object, key) { seen.push("get:" + key); return Reflect.get(object, key, object); },
                has(object, key) { seen.push("has:" + key); return Reflect.has(object, key); },
                deleteProperty(object, key) { seen.push("delete:" + key); return Reflect.deleteProperty(object, key); }
            });
            const names = ["0", "01", "+1", "4294967294", "4294967295", "\u0661"];
            const expected = [];
            for (const key of names) {
                proxy[key] = 42;
                if (proxy[key] !== 42 || !(key in proxy) || !delete proxy[key]) return false;
                expected.push("set:" + key, "get:" + key, "has:" + key, "delete:" + key);
            }
            let conversions = 0;
            const key = { [Symbol.toPrimitive](hint) { conversions++; return hint === "string" ? "01" : "wrong"; } };
            proxy[key] = 7;
            proxy[-0] = 11;
            proxy[1n] = 13;
            expected.push("set:01", "set:0", "set:1");
            return seen.join("|") === expected.join("|") && conversions === 1 &&
                target["01"] === 7 && target[0] === 11 && target[1] === 13 && target.length === 4294967295;
        })()
        "#,
    )
}

fn eval_true_in_both_modes(case: &str, source: &str) -> TestResult {
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let mut vm = Vm::with_config(VmConfig::default().with_optimization_mode(mode));
        let value = vm
            .eval(source)
            .map_err(|error| format!("array-index regression '{case}' in {mode:?}: {error}"))?;
        if value != Value::Bool(true) {
            return Err(format!(
                "array-index regression '{case}' in {mode:?}: expected true, got {value:?}"
            )
            .into());
        }
    }
    Ok(())
}
