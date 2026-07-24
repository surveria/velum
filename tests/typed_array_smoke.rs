use velum::{Runtime, Value};

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
        let f16 = new Float16Array([-0, 0, 1.337, 65520, NaN]);
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
            1 / f16[0] === -Infinity && 1 / f16[1] === Infinity &&
            f16[2] === Math.f16round(1.337) && f16[3] === Infinity &&
            Number.isNaN(f16[4]) &&
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
            Uint32Array, Float16Array, Float32Array, Float64Array
        ];
        let metadata = names[0].name === "Int8Array" &&
            names[9].name === "Float64Array" &&
            names[0].length === 3 && names[9].length === 3 &&
            Int16Array.BYTES_PER_ELEMENT === 2 &&
            Float16Array.BYTES_PER_ELEMENT === 2 &&
            Float64Array.BYTES_PER_ELEMENT === 8 &&
            Float16Array.prototype.BYTES_PER_ELEMENT === 2 &&
            Int16Array.prototype.BYTES_PER_ELEMENT === 2 &&
            typeof ArrayBuffer.prototype.resize === "function";

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
fn supports_bigint_element_kinds_and_content_boundaries() -> TestResult {
    ensure_eval(
        r#"
        let signed = new BigInt64Array([
            0n,
            9223372036854775807n,
            9223372036854775808n,
            -9223372036854775809n
        ]);
        let unsigned = new BigUint64Array([-1n, 18446744073709551616n, 5n]);
        let sorted = new BigInt64Array([3n, -2n, 1n]).toSorted();
        let mapped = signed.map(value => value + 1n);
        let failures = 0;
        try { new BigInt64Array([1]); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { new Uint8Array([1n]); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { new BigInt64Array(new Uint8Array(0)); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        let conversions = 0;
        signed[0] = { valueOf() { conversions = conversions + 1; return 9n; } };
        Reflect.defineProperty(signed, "1", {
            value: { valueOf() { conversions = conversions + 1; return -7n; } }
        });
        let rejected = Reflect.defineProperty(signed, "2", {
            value: 4n,
            configurable: false
        });
        let descriptor = Object.getOwnPropertyDescriptor(
            BigInt64Array.prototype,
            "BYTES_PER_ELEMENT"
        );
        let indexDescriptor = Object.getOwnPropertyDescriptor(signed, "0");

        signed[0] === 9n && signed[1] === -7n &&
            signed[2] === -9223372036854775808n &&
            signed[3] === 9223372036854775807n &&
            unsigned[0] === 18446744073709551615n && unsigned[1] === 0n &&
            unsigned[2] === 5n && sorted.join(",") === "-2,1,3" &&
            mapped[0] === 1n && mapped[2] === -9223372036854775807n &&
            BigInt64Array.length === 3 && BigUint64Array.length === 3 &&
            BigInt64Array.BYTES_PER_ELEMENT === 8 &&
            BigUint64Array.prototype.BYTES_PER_ELEMENT === 8 &&
            descriptor.writable === false && descriptor.enumerable === false &&
            descriptor.configurable === false && indexDescriptor.value === 9n &&
            indexDescriptor.writable === true && indexDescriptor.enumerable === true &&
            indexDescriptor.configurable === true &&
            Object.keys(signed).join(",") === "0,1,2,3" &&
            conversions === 2 && rejected === false && failures === 3 ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_relative_end_uses_length_for_explicit_undefined() -> TestResult {
    ensure_eval(
        r"
        let filled = new Uint8Array([0, 0]).fill(1, 0, undefined);
        let sliced = new Uint8Array([1, 2]).subarray(0, undefined);
        filled.join(',') === '1,1' && sliced.join(',') === '1,2' ? 42 : 0
        ",
        &Value::Number(42.0),
    )
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
        try { new BigInt64Array(new SharedArrayBuffer(6)); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { new Uint32Array(new ArrayBuffer(129, { maxByteLength: 1814 })); } catch (error) {
            if (error instanceof RangeError) failures = failures + 1;
        }
        try { Int8Array(1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        try { ArrayBuffer(1); } catch (error) {
            if (error instanceof TypeError) failures = failures + 1;
        }
        const largeResizable = new Uint8Array(
            new ArrayBuffer(255, { maxByteLength: 4294967295 })
        );
        const largeGrowable = new Uint8ClampedArray(
            new SharedArrayBuffer(223, { maxByteLength: 268435440 })
        );
        largeResizable.length === 255 && largeGrowable.length === 223 ? failures : 0
        ",
    )?;

    context.storage_snapshot()?;
    ensure_value(&value, &Value::Number(6.0))
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
fn typed_array_to_locale_string_dispatches_each_element() -> TestResult {
    ensure_eval(
        r#"
        let calls = [];
        let options = { marker: 1 };
        Number.prototype.toLocaleString = function (locales, receivedOptions) {
            calls.push(Number(this) + ":" + locales + ":" +
                (receivedOptions === options) + ":" + arguments.length);
            return "value-" + this;
        };
        let sample = new Uint8Array([3, 1, 2]);
        Object.defineProperty(sample, "length", {
            get: function () { throw new Error("length property must not be read"); }
        });
        sample.toLocaleString("en", options) === "value-3,value-1,value-2" &&
            calls.join("|") === "3:en:true:2|1:en:true:2|2:en:true:2" ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_numeric_writes_preserve_conversion_and_reflect_results() -> TestResult {
    ensure_eval(
        r#"
        let sample = new Uint8Array([1]);
        let conversions = 0;
        let value = {
            valueOf: function () {
                conversions += 1;
                return 7;
            }
        };
        let setInvalid = Reflect.set(sample, "2", value);
        let setNegativeZero = Reflect.set(sample, "-0", value);
        let assignmentThrew = false;
        try {
            sample["3"] = {
                valueOf: function () { throw new Error("converted"); }
            };
        } catch (error) {
            assignmentThrew = error.message === "converted";
        }
        Object.preventExtensions(sample);
        let defineOrdinary = Reflect.defineProperty(sample, "1.0", { value: 9 });

        setInvalid && setNegativeZero && assignmentThrew && conversions === 2 &&
            sample[2] === undefined && sample["-0"] === undefined &&
            defineOrdinary === false ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_intrinsic_is_abstract_but_constructable() -> TestResult {
    ensure_eval(
        r#"
        let TypedArray = Object.getPrototypeOf(Uint8Array);
        let propagated = false;
        let source = {};
        Object.defineProperty(source, "length", {
            get: function () { throw new Error("source length"); }
        });
        try {
            TypedArray.from(source);
        } catch (error) {
            propagated = error.message === "source length";
        }
        let abstract = false;
        try {
            new TypedArray();
        } catch (error) {
            abstract = error instanceof TypeError;
        }
        propagated && abstract ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_iteration_uses_internal_length_and_live_view_values() -> TestResult {
    ensure_eval(
        r#"
        let source = new Uint8Array([1, 2, 3, 4]);
        let lengthReads = 0;
        Object.defineProperty(source, "length", {
            get() {
                lengthReads += 1;
                return 0;
            }
        });
        let visits = 0;
        let internalLength = source.every(() => {
            visits += 1;
            return true;
        }) && source.includes(3) && source.indexOf(2) === 1 &&
            source.lastIndexOf(4) === 3 &&
            source.reduce((sum, value) => sum + value, 0) === 10;

        let growBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
        let growing = new Uint8Array(growBuffer);
        growing.set([1, 2, 3, 4]);
        let growSeen = [];
        growing.some((value, index) => {
            growSeen.push(value);
            if (index === 1) growBuffer.resize(6);
            return false;
        });

        let shrinkBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
        let shrinking = new Uint8Array(shrinkBuffer);
        shrinking.set([1, 2, 3, 4]);
        let shrinkSeen = [];
        shrinking.reduce((sum, value, index) => {
            shrinkSeen.push(value);
            if (index === 1) shrinkBuffer.resize(3);
            return sum;
        }, 0);

        let iteratorBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
        let fixed = new Uint8Array(iteratorBuffer, 0, 4);
        let iterator = fixed.values();
        iterator.next();
        iteratorBuffer.resize(3);
        let iteratorRejected = false;
        try {
            iterator.next();
        } catch (error) {
            iteratorRejected = error instanceof TypeError;
        }
        let sharedPrototype = Object.getPrototypeOf(Uint8Array.prototype);

        internalLength && lengthReads === 0 && visits === 4 &&
            growSeen.join(",") === "1,2,3,4" &&
            shrinkSeen.length === 4 && shrinkSeen[0] === 1 &&
            shrinkSeen[1] === 2 && shrinkSeen[2] === 3 &&
            shrinkSeen[3] === undefined && iteratorRejected &&
            sharedPrototype.toString === Array.prototype.toString ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_copy_methods_refresh_views_and_preserve_aliasing() -> TestResult {
    ensure_eval(
        r#"
        let source = new Uint8Array([10, 20, 30, 40, 50, 60]);
        source.constructor = {
            [Symbol.species]: function() {
                return new Uint8Array(source.buffer, 2);
            }
        };
        let aliased = source.slice(1, 4);

        let rab = new ArrayBuffer(4, { maxByteLength: 8 });
        let tracking = new Uint8Array(rab);
        tracking.set([0, 1, 2, 3]);
        tracking.copyWithin({
            valueOf() {
                rab.resize(3);
                return 2;
            }
        }, 0);

        let values = new Uint8Array([1, 2, 3, 4]);
        let lengthReads = 0;
        Object.defineProperty(values, "length", {
            get() {
                lengthReads += 1;
                return 0;
            }
        });
        let reversedCopy = values.toReversed();
        values.reverse();

        let subarrayBuffer = new ArrayBuffer(4, { maxByteLength: 8 });
        let trackingSubarray = new Uint8Array(subarrayBuffer).subarray(1);
        subarrayBuffer.resize(6);

        let withSource = new Uint8Array([0, 1, 2]);
        let withResult = withSource.with(1, {
            valueOf() {
                withSource[0] = 3;
                return 4;
            }
        });

        aliased.join(",") === "20,20,20,60" &&
            tracking.join(",") === "0,1,0" &&
            reversedCopy.join(",") === "4,3,2,1" &&
            values.join(",") === "4,3,2,1" && lengthReads === 0 &&
            trackingSubarray.length === 5 &&
            withResult.join(",") === "3,4,2" &&
            withSource.join(",") === "3,1,2" ? 42 : 0
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
fn batches_typed_array_storage_without_changing_numeric_or_overlap_semantics() -> TestResult {
    ensure_eval(
        r#"
        let overlap = new Uint16Array([1, 2, 3, 4]);
        overlap.set(overlap.subarray(0, 3), 1);

        let converted = new Float64Array(4);
        converted.set(new Uint16Array([6, 7]), 1);

        let filled = new BigInt64Array(4);
        filled.fill(-1n, 1, 3);

        let copied = new Uint32Array([1, 2, 3, 4, 5]);
        copied.copyWithin(1, 0, 4).reverse();

        let sorted = new Float64Array([4, NaN, -0, 2]);
        sorted.sort();

        let indexed = new Uint8Array([4]);
        indexed["01"] = 9;
        let huge = "184467440737095516160";
        let hugeIsOrdinary = Reflect.set(indexed, huge, 8) === true && indexed[huge] === 8;

        overlap.join(",") === "1,1,2,3" &&
            converted.join(",") === "0,6,7,0" &&
            filled.join(",") === "0,-1,-1,0" &&
            copied.join(",") === "4,3,2,1,1" &&
            sorted[0] === 0 && 1 / sorted[0] === -Infinity &&
            sorted[1] === 2 && sorted[2] === 4 && Number.isNaN(sorted[3]) &&
            indexed[0] === 4 && indexed["01"] === 9 && hugeIsOrdinary ? 42 : 0
        "#,
        &Value::Number(42.0),
    )
}

#[test]
fn typed_array_set_preserves_array_like_order_and_resize_semantics() -> TestResult {
    ensure_eval(
        r#"
        let primitive = new Uint8Array([0, 0, 0, 9]);
        primitive.set("123");

        let target = new Uint8Array([0, 0, 0]);
        let events = [];
        let source = {
            get length() { events.push("length"); return 2; },
            get 0() { events.push("get0"); return { valueOf() {
                events.push("convert0"); return 7;
            } }; },
            get 1() { events.push("get1"); throw new Error("stop"); },
            get 2() { events.push("get2"); return 9; }
        };
        try {
            target.set(source, { valueOf() { events.push("offset"); return 1; } });
        } catch (error) {}

        let detached = new Uint8Array(1);
        let detachedRejected = false;
        try {
            detached.set([1], { valueOf() {
                detached.buffer.transfer();
                return 0;
            } });
        } catch (error) {
            detachedRejected = error instanceof TypeError;
        }

        let rab = new ArrayBuffer(4, { maxByteLength: 4 });
        let resized = new Uint8Array(rab, 0, 4);
        let requested = [];
        let resizingSource = {
            length: 3,
            get 0() { requested.push(0); return 5; },
            get 1() { requested.push(1); rab.resize(1); return 6; },
            get 2() { requested.push(2); return 7; }
        };
        resized.set(resizingSource);

        primitive.join(",") === "1,2,3,9" && target.join(",") === "0,7,0" &&
            events.join(",") === "offset,length,get0,convert0,get1" &&
            detachedRejected === true && requested.join(",") === "0,1,2" &&
            new Uint8Array(rab)[0] === 5 ? 42 : 0
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

#[test]
fn supports_uint8_array_base64_and_hex_codecs() -> TestResult {
    ensure_eval(
        r#"
        let decoded = Uint8Array.fromBase64("x+/y");
        let decodedUrl = Uint8Array.fromBase64("x-_y", { alphabet: "base64url" });
        let decodedHex = Uint8Array.fromHex("666F6f");

        let base = new Uint8Array([255, 255, 255, 255, 255, 255]);
        let partial = base.subarray(1, 4);
        let base64Result = partial.setFromBase64("Zm9vYmFy");
        let hexResult = base.subarray(4).setFromHex("aabbcc");

        let wroteBeforeError = false;
        let errorTarget = new Uint8Array([255, 255, 255, 255, 255]);
        try {
            errorTarget.setFromBase64("MjYyZg===");
        } catch (error) {
            wroteBeforeError = error instanceof SyntaxError &&
                errorTarget.join(",") === "50,54,50,255,255";
        }

        let descriptors = Object.getOwnPropertyDescriptor(Uint8Array, "fromBase64");
        let failures = 0;
        try { Uint8Array.fromHex("abc"); }
        catch (error) { if (error instanceof SyntaxError) failures += 1; }
        try { Uint8Array.fromBase64("x-_y"); }
        catch (error) { if (error instanceof SyntaxError) failures += 1; }
        try { Uint8Array.fromBase64({ toString: () => "Zg==" }); }
        catch (error) { if (error instanceof TypeError) failures += 1; }

        decoded.join(",") === "199,239,242" &&
            decodedUrl.join(",") === "199,239,242" &&
            decodedHex.join(",") === "102,111,111" &&
            base64Result.read === 4 && base64Result.written === 3 &&
            hexResult.read === 4 && hexResult.written === 2 &&
            base.join(",") === "255,102,111,111,170,187" &&
            new Uint8Array([199, 239, 242]).toBase64({ alphabet: "base64url" }) === "x-_y" &&
            new Uint8Array([255]).toBase64({ omitPadding: true }) === "/w" &&
            decodedHex.toHex() === "666f6f" &&
            Uint8Array.fromBase64.length === 1 && Uint8Array.fromHex.length === 1 &&
            Uint8Array.prototype.setFromBase64.length === 1 &&
            Uint8Array.prototype.setFromHex.length === 1 &&
            Uint8Array.prototype.toBase64.length === 0 &&
            Uint8Array.prototype.toHex.length === 0 &&
            descriptors.writable && !descriptors.enumerable && descriptors.configurable &&
            wroteBeforeError && failures === 3 ? 42 : 0
        "#,
        &Value::Number(42.0),
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
