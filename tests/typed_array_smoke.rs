use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_array_buffer_and_uint8_array_index_access() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let buffer = new ArrayBuffer(8);
        let data = new Uint8Array(buffer);
        data[0] = 257;
        data[1] = -1;
        data[2] = 42;
        data[20] = 99;

        print(
            buffer.byteLength,
            data.length,
            data.byteLength,
            data.byteOffset
        );
        print(data[0], data[1], data[2], data[20]);

        buffer.byteLength === 8 &&
            data.length === 8 &&
            data.byteLength === 8 &&
            data.byteOffset === 0 &&
            data[0] === 1 &&
            data[1] === 255 &&
            data[2] === 42 &&
            data[20] === undefined ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["8 8 8 0".to_owned(), "1 255 42 undefined".to_owned()],
    )
}

#[test]
fn exposes_host_provided_uint8_array_global() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.create_host_uint8_array_global("imageData", vec![0, 1, 2, 3])?;
    if !matches!(value, Value::Object(_)) {
        return Err("expected host array value to be an object".into());
    }
    ensure_optional_origin(context.typed_array_debug_origin(&value)?, "host-provided")?;

    let result = context.eval(
        r"
        imageData[0] = imageData[1] + imageData[2] + imageData[3];
        imageData[0]
        ",
    )?;
    ensure_value(&result, &Value::Number(6.0))
}

#[test]
fn supports_numeric_element_kinds_and_array_like_sources() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let i8 = new Int8Array([127, 128, -129]);
        let clamped = new Uint8ClampedArray([-1, 0.5, 1.5, 254.6, 300]);
        let i16 = new Int16Array([32767, 32768, -32769]);
        let u16 = new Uint16Array([-1, 65537]);
        let i32 = new Int32Array([2147483648, -2147483649]);
        let u32 = new Uint32Array([-1, 4294967297]);
        let f32 = new Float32Array([1.337, Infinity, NaN]);
        let f64 = new Float64Array([Math.PI, -0]);

        print(i8[0], i8[1], i8[2]);
        print(clamped[0], clamped[1], clamped[2], clamped[3], clamped[4]);
        print(i16[0], i16[1], i16[2], u16[0], u16[1]);
        print(i32[0], i32[1], u32[0], u32[1]);

        i8[0] === 127 && i8[1] === -128 && i8[2] === 127 &&
            clamped[0] === 0 && clamped[1] === 0 && clamped[2] === 2 &&
            clamped[3] === 255 && clamped[4] === 255 &&
            i16[0] === 32767 && i16[1] === -32768 && i16[2] === 32767 &&
            u16[0] === 65535 && u16[1] === 1 &&
            i32[0] === -2147483648 && i32[1] === 2147483647 &&
            u32[0] === 4294967295 && u32[1] === 1 &&
            f32[0] === Math.fround(1.337) && f32[1] === Infinity &&
            Number.isNaN(f32[2]) && f64[0] === Math.PI &&
            1 / f64[1] === -Infinity ? 42 : 0
        ",
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "127 -128 127".to_owned(),
            "0 0 2 255 255".to_owned(),
            "32767 -32768 32767 65535 1".to_owned(),
            "-2147483648 2147483647 4294967295 1".to_owned(),
        ],
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn supports_shared_array_buffer_views_and_constructor_metadata() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let buffer = new ArrayBuffer(24);
        let u8 = new Uint8Array(buffer);
        let i16 = new Int16Array(buffer, 2, 3);
        let f64 = new Float64Array(buffer, 8, 2);

        i16[0] = -2;
        f64[0] = Math.PI;

        let names = [
            Int8Array, Uint8Array, Uint8ClampedArray,
            Int16Array, Uint16Array, Int32Array,
            Uint32Array, Float32Array, Float64Array
        ];
        let metadata = names[0].name === "Int8Array" &&
            names[8].name === "Float64Array" &&
            names[0].length === 1 && names[8].length === 1 &&
            Int16Array.BYTES_PER_ELEMENT === 2 &&
            Float64Array.BYTES_PER_ELEMENT === 8 &&
            Int16Array.prototype.BYTES_PER_ELEMENT === 2 &&
            ArrayBuffer.prototype.resize === undefined;

        print(buffer.byteLength, i16.length, i16.byteLength, i16.byteOffset);
        print(f64.length, f64.byteLength, f64.byteOffset);

        buffer.byteLength === 24 && i16.length === 3 && i16.byteLength === 6 &&
            i16.byteOffset === 2 && f64.length === 2 && f64.byteLength === 16 &&
            f64.byteOffset === 8 &&
            u8[2] === 254 && u8[3] === 255 && f64[0] === Math.PI && metadata ? 42 : 0
        "#,
    )?;

    ensure_output(
        context.output(),
        &["24 3 6 2".to_owned(), "2 16 8".to_owned()],
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn rejects_misaligned_views_and_calls_without_new() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r"
        let failures = 0;
        try { new Int16Array(new ArrayBuffer(8), 1); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { new Float64Array(new ArrayBuffer(8), 0, 2); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { Int8Array(1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { ArrayBuffer(1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        failures
        ",
    )?;

    ensure_value(&value, &Value::Number(4.0))
}

#[test]
fn exposes_shared_typed_array_intrinsics_and_accessors() -> TestResult {
    ensure_eval(
        r#"
        let intrinsic = Object.getPrototypeOf(Uint8Array);
        let shared = Object.getPrototypeOf(Uint8Array.prototype);
        let lengthDescriptor = Object.getOwnPropertyDescriptor(shared, "length");
        let tagDescriptor = Object.getOwnPropertyDescriptor(shared, Symbol.toStringTag);
        let speciesDescriptor = Object.getOwnPropertyDescriptor(intrinsic, Symbol.species);
        let value = new Uint8Array([1, 2, 3]);

        intrinsic.name === "TypedArray" && intrinsic.length === 0 &&
            Object.getPrototypeOf(Int16Array) === intrinsic &&
            Object.getPrototypeOf(Int16Array.prototype) === shared &&
            shared.constructor === intrinsic && value.constructor === Uint8Array &&
            value.length === 3 && value.byteLength === 3 && value.byteOffset === 0 &&
            value.buffer.byteLength === 3 && value[Symbol.toStringTag] === "Uint8Array" &&
            typeof lengthDescriptor.get === "function" &&
            lengthDescriptor.enumerable === false && lengthDescriptor.configurable === true &&
            typeof tagDescriptor.get === "function" &&
            typeof speciesDescriptor.get === "function" &&
            Uint8Array[Symbol.species] === Uint8Array &&
            typeof Uint8Array.from === "function" && typeof Uint8Array.of === "function"
            ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn supports_typed_array_callbacks_search_and_copy_methods() -> TestResult {
    ensure_eval(
        r#"
        let source = new Uint8Array([3, 1, 2, 3]);
        let visited = [];
        source.forEach((value, index) => visited.push(value + index));
        let mapped = source.map(value => value + 1);
        let filtered = source.filter(value => value > 1);
        let copy = source.slice(1, 3);
        let reversed = source.toReversed();
        let replaced = source.with(1, 9);

        visited.join(",") === "3,2,4,6" &&
            mapped instanceof Uint8Array && mapped.join(",") === "4,2,3,4" &&
            filtered instanceof Uint8Array && filtered.join(",") === "3,2,3" &&
            copy instanceof Uint8Array && copy.join(",") === "1,2" &&
            reversed.join(",") === "3,2,1,3" && replaced.join(",") === "3,9,2,3" &&
            source.every(value => value > 0) && source.some(value => value === 2) &&
            source.find(value => value === 2) === 2 &&
            source.findIndex(value => value === 3) === 0 &&
            source.findLast(value => value === 3) === 3 &&
            source.findLastIndex(value => value === 3) === 3 &&
            source.reduce((sum, value) => sum + value, 0) === 9 &&
            source.reduceRight((sum, value) => sum * 10 + value, 0) === 3213 &&
            source.includes(2) && source.indexOf(3) === 0 && source.lastIndexOf(3) === 3 &&
            source.at(-1) === 3 && source.toString() === "3,1,2,3" &&
            source.toLocaleString() === "3,1,2,3" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn supports_typed_array_mutation_sort_iteration_and_statics() -> TestResult {
    ensure_eval(
        r#"
        let target = new Uint8Array([1, 2, 3, 4]);
        target.set(target.subarray(0, 3), 1);
        let shared = target.subarray(1, 3);
        shared[0] = 9;
        target.copyWithin(2, 0, 2).fill(7, 3);

        let sorted = new Float64Array([10, 2, NaN, -0, 0]).toSorted();
        let mutable = new Float64Array([10, 2, NaN, -0, 0]);
        mutable.sort();
        let keyItems = [];
        for (let key of target.keys()) keyItems.push(key);
        let valueItems = [];
        for (let value of target.values()) valueItems.push(value);
        let entries = [];
        for (let entry of target.entries()) entries.push(entry.join(":"));
        let keys = keyItems.join(",");
        let values = valueItems.join(",");
        let from = Uint16Array.from([1, 2, 3], value => value * 2);
        let of = Int8Array.of(127, 128, -129);

        target.join(",") === "1,9,1,7" && shared.buffer === target.buffer &&
            sorted[0] === 0 && 1 / sorted[0] === -Infinity && sorted[1] === 0 &&
            sorted[2] === 2 && sorted[3] === 10 && Number.isNaN(sorted[4]) &&
            mutable.join(",") === sorted.join(",") && keys === "0,1,2,3" &&
            values === "1,9,1,7" && entries.join(",") === "0:1,1:9,2:1,3:7" &&
            from instanceof Uint16Array && from.join(",") === "2,4,6" &&
            of.join(",") === "127,-128,127" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn honors_species_and_rejects_invalid_receivers() -> TestResult {
    ensure_eval(
        r#"
        Object.defineProperty(Uint8Array, Symbol.species, {
          value: Int16Array,
          configurable: true
        });
        let mapped = new Uint8Array([1, 2]).map(value => value + 300);
        let failures = 0;
        try { Object.getPrototypeOf(Uint8Array.prototype).map.call([], value => value); }
        catch (error) { if (error instanceof TypeError) failures = failures + 1; }
        try { Object.getPrototypeOf(Uint8Array.prototype).set.call({}, [1]); }
        catch (error) { if (error instanceof TypeError) failures = failures + 1; }
        try { new Uint8Array(2).set([1, 2, 3]); }
        catch (error) { if (error instanceof RangeError) failures = failures + 1; }

        mapped instanceof Int16Array && mapped.join(",") === "301,302" && failures === 3
            ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn rejects_short_typed_array_static_results() -> TestResult {
    ensure_eval(
        r"
        let failures = 0;
        let constructors = [
            function() { return new Uint8Array(1); },
            function() { return new Uint8Array([0]); },
            function() { return new Uint8Array({ 0: 0, length: 1 }); },
            function() {
                return new Uint8Array({
                    [Symbol.iterator]: () => [0][Symbol.iterator]()
                });
            },
            function() { return new Uint8Array(new ArrayBuffer(1)); }
        ];
        for (let ShortResult of constructors) {
            try { Uint8Array.from.call(ShortResult, [1, 2]); }
            catch (error) { if (error.constructor === TypeError) failures = failures + 1; }
            try { Uint8Array.from.call(ShortResult, { 0: 1, 1: 2, length: 2 }); }
            catch (error) { if (error.constructor === TypeError) failures = failures + 1; }
            try { Uint8Array.of.call(ShortResult, 1, 2); }
            catch (error) { if (error.constructor === TypeError) failures = failures + 1; }
        }

        failures
        ",
        &Value::Number(15.0),
    )
}

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let actual = context.eval(source)?;
    ensure_value(&actual, expected)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[String]) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_optional_origin(actual: Option<&str>, expected: &str) -> TestResult {
    if actual == Some(expected) {
        return Ok(());
    }
    Err(format!("expected origin {expected:?}, got {actual:?}").into())
}
