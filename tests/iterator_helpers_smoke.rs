use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> rs_quickjs::Result<Value> {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(source)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_string(source: &str, expected: &str) -> TestResult {
    ensure_value(&eval(source)?, &Value::from(expected))
}

#[test]
fn iterator_global_exposes_prototype_and_from() -> TestResult {
    ensure_string(
        r#"
        "" + typeof Iterator + ":" + typeof Iterator.prototype.map
            + ":" + typeof Iterator.prototype.filter + ":" + typeof Iterator.from
            + ":" + Iterator.name + ":" + Iterator.prototype.map.length
        "#,
        "function:function:function:function:Iterator:1",
    )
}

#[test]
fn iterator_constructor_is_abstract_but_subclassable() -> TestResult {
    ensure_string(
        r#"
        let direct = "none";
        try { new Iterator(); } catch (e) { direct = e instanceof TypeError ? "type" : "other"; }
        let call = "none";
        try { Iterator(); } catch (e) { call = e instanceof TypeError ? "type" : "other"; }
        class Numbers extends Iterator {
            constructor() { super(); this.n = 0; }
            next() { this.n += 1; return { value: this.n, done: this.n > 2 }; }
        }
        const sub = new Numbers();
        direct + ":" + call + ":" + (sub instanceof Numbers) + ":" + (sub instanceof Iterator)
            + ":" + sub.map(x => x * 2).toArray().join(",")
        "#,
        "type:type:true:true:2,4",
    )
}

#[test]
fn generator_objects_inherit_iterator_helpers() -> TestResult {
    ensure_string(
        r#"
        function* nums() { yield 1; yield 2; yield 3; yield 4; }
        nums().map(x => x * 10).filter(x => x > 10).toArray().join(",")
        "#,
        "20,30,40",
    )
}

#[test]
fn builtin_collection_iterators_inherit_iterator_helpers() -> TestResult {
    ensure_string(
        r#"
        const fromArray = [5, 6].values().map(x => x + 1).toArray().join(",");
        const fromSet = new Set([1, 2, 3]).values().map(x => x * 2).toArray().join(",");
        const fromMap = new Map([["a", 1]]).keys().toArray().join(",");
        fromArray + ":" + fromSet + ":" + fromMap
        "#,
        "6,7:2,4,6:a",
    )
}

#[test]
fn take_and_drop_validate_and_slice() -> TestResult {
    ensure_string(
        r#"
        function* nums() { yield 1; yield 2; yield 3; yield 4; }
        let negative = "none";
        try { nums().take(-1); } catch (e) { negative = e instanceof RangeError ? "range" : "other"; }
        let nan = "none";
        try { nums().drop(NaN); } catch (e) { nan = e instanceof RangeError ? "range" : "other"; }
        nums().take(2).toArray().join(",") + ":" + nums().drop(2).toArray().join(",")
            + ":" + nums().take(Infinity).toArray().length + ":" + negative + ":" + nan
        "#,
        "1,2:3,4:4:range:range",
    )
}

#[test]
fn flat_map_flattens_inner_iterables_lazily() -> TestResult {
    ensure_string(
        r#"
        function* nums() { yield 1; yield 2; }
        const flat = nums().flatMap(x => [x, x * 100]).toArray().join(",");
        let primitive = "none";
        try { nums().flatMap(x => x).toArray(); }
        catch (e) { primitive = e instanceof TypeError ? "type" : "other"; }
        flat + ":" + primitive
        "#,
        "1,100,2,200:type",
    )
}

#[test]
fn eager_consumers_cover_reduce_and_predicates() -> TestResult {
    ensure_string(
        r#"
        function* nums() { yield 1; yield 2; yield 3; yield 4; }
        let empty = "none";
        try { [].values().reduce((a, b) => a + b); }
        catch (e) { empty = e instanceof TypeError ? "type" : "other"; }
        const seen = [];
        nums().forEach((v, i) => seen.push(v + "@" + i));
        nums().reduce((a, b) => a + b, 100) + ":" + nums().reduce((a, b) => a + b)
            + ":" + nums().some(x => x === 3) + ":" + nums().some(x => x > 9)
            + ":" + nums().every(x => x > 0) + ":" + nums().every(x => x > 2)
            + ":" + nums().find(x => x > 2) + ":" + seen.join(",") + ":" + empty
        "#,
        "110:10:true:false:true:false:3:1@0,2@1,3@2,4@3:type",
    )
}

#[test]
fn helpers_are_lazy_and_close_the_underlying_iterator() -> TestResult {
    ensure_string(
        r#"
        function* nums() { yield 1; yield 2; yield 3; }
        let ran = false;
        nums().map(() => { ran = true; });
        let closed = false;
        const closable = {
            i: 0,
            next() { this.i += 1; return { value: this.i, done: false }; },
            return() { closed = true; return { done: true }; }
        };
        Iterator.from(closable).take(1).toArray();
        const helper = nums().map(x => x);
        const first = helper.next().value;
        const finished = helper.return();
        const afterReturn = helper.next();
        (!ran) + ":" + closed + ":" + first + ":" + finished.done
            + ":" + afterReturn.done + ":" + (afterReturn.value === undefined)
        "#,
        "true:true:1:true:true:true",
    )
}

#[test]
fn iterator_from_wraps_plain_iterators_and_strings() -> TestResult {
    ensure_string(
        r#"
        const plain = {
            i: 0,
            next() { this.i += 1; return { value: this.i, done: this.i > 3 }; }
        };
        const wrapped = Iterator.from(plain);
        function* gen() { yield 7; }
        const g = gen();
        let primitive = "none";
        try { Iterator.from(5); } catch (e) { primitive = e instanceof TypeError ? "type" : "other"; }
        wrapped.toArray().join(",") + ":" + Iterator.from("ab").toArray().join(",")
            + ":" + (Iterator.from(g) === g) + ":" + (typeof wrapped.map) + ":" + primitive
        "#,
        "1,2,3:a,b:true:function:type",
    )
}

#[test]
fn iterator_from_string_fallback_preserves_exact_utf16_code_points() -> TestResult {
    ensure_string(
        r#"
        delete String.prototype[Symbol.iterator];
        const lone = String.fromCharCode(0xD800);
        const pair = String.fromCodePoint(0x1F4F7);
        const values = Iterator.from(lone + pair).toArray();
        values.length + ":" + values[0].length + ":" + values[0].charCodeAt(0)
            + ":" + values[1].length + ":" + values[1].codePointAt(0)
        "#,
        "2:1:55296:2:128247",
    )
}

#[test]
fn helper_methods_validate_receiver_and_callback() -> TestResult {
    ensure_string(
        r#"
        function* nums() { yield 1; }
        let receiver = "none";
        try { Iterator.prototype.map.call(1, x => x); }
        catch (e) { receiver = e instanceof TypeError ? "type" : "other"; }
        let callback = "none";
        try { nums().map(1); } catch (e) { callback = e instanceof TypeError ? "type" : "other"; }
        let consumer = "none";
        try { Iterator.prototype.toArray.call(null); }
        catch (e) { consumer = e instanceof TypeError ? "type" : "other"; }
        receiver + ":" + callback + ":" + consumer
        "#,
        "type:type:type",
    )
}

#[test]
fn helper_callback_errors_close_the_underlying_iterator() -> TestResult {
    ensure_string(
        r#"
        let closed = 0;
        function closable() {
            return {
                i: 0,
                next() { this.i += 1; return { value: this.i, done: false }; },
                return() { closed += 1; return { done: true }; }
            };
        }
        let mapError = "none";
        try { Iterator.from(closable()).map(() => { throw new Error("boom"); }).next(); }
        catch (e) { mapError = e.message; }
        let forEachError = "none";
        try { Iterator.from(closable()).forEach(() => { throw new Error("bang"); }); }
        catch (e) { forEachError = e.message; }
        mapError + ":" + forEachError + ":" + closed
        "#,
        "boom:bang:2",
    )
}

#[test]
fn helper_objects_report_iterator_helper_tag() -> TestResult {
    ensure_string(
        r"
        function* nums() { yield 1; }
        Object.prototype.toString.call(nums().map(x => x))
        ",
        "[object Iterator Helper]",
    )
}

#[test]
fn iterator_results_survive_garbage_collection() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        function* nums() { yield 1; yield 2; yield 3; }
        globalThis.helper = nums().map(x => x * 2);
        globalThis.first = helper.next().value;
        ",
    )?;
    context.collect_garbage()?;
    let value =
        context.eval("first + \":\" + helper.next().value + \":\" + helper.next().value")?;
    ensure_value(&value, &Value::from("2:4:6"))
}

#[test]
fn iterator_intrinsic_prototype_surface_is_spec_shaped() -> TestResult {
    ensure_string(
        r#"
        const arrayIteratorPrototype = Object.getPrototypeOf([].values());
        const iteratorPrototype = Object.getPrototypeOf(arrayIteratorPrototype);
        const constructorDesc = Object.getOwnPropertyDescriptor(iteratorPrototype, "constructor");
        const tagDesc = Object.getOwnPropertyDescriptor(iteratorPrototype, Symbol.toStringTag);
        const child = Object.create(iteratorPrototype);
        constructorDesc.set.call(child, 123);
        tagDesc.set.call(child, "Child Iterator");
        let homeThrows = false;
        try { constructorDesc.set.call(iteratorPrototype, 0); }
        catch (e) { homeThrows = e instanceof TypeError; }
        let disposed = 0;
        const disposable = Object.create(iteratorPrototype);
        disposable.return = function () { disposed += 1; return { done: true }; };
        const disposeResult = disposable[Symbol.dispose]();
        (iteratorPrototype === Iterator.prototype) + ":"
            + (typeof constructorDesc.get) + ":" + (typeof constructorDesc.set) + ":"
            + (constructorDesc.get.call() === Iterator) + ":" + tagDesc.get.call() + ":"
            + child.constructor + ":" + child[Symbol.toStringTag] + ":" + homeThrows + ":"
            + disposed + ":" + (disposeResult === undefined)
        "#,
        "true:function:function:true:Iterator:123:Child Iterator:true:1:true",
    )
}

#[test]
fn argument_validation_closes_without_reading_next() -> TestResult {
    ensure_value(
        &eval(
            r#"
            let closed = 0;
            const closable = {
                __proto__: Iterator.prototype,
                get next() { throw new Error("next must not be read"); },
                return() { closed += 1; return {}; }
            };
            const attempt = fn => { try { fn(); } catch (e) {} };
            attempt(() => closable.map());
            attempt(() => closable.filter({}));
            attempt(() => closable.flatMap());
            attempt(() => closable.reduce());
            attempt(() => closable.forEach(null));
            attempt(() => closable.some());
            attempt(() => closable.every(1));
            attempt(() => closable.find());
            attempt(() => closable.take(NaN));
            attempt(() => closable.drop(-1));
            closed
            "#,
        )?,
        &Value::Number(10.0),
    )
}

#[test]
fn iterator_from_observes_strings_and_accepts_callable_objects() -> TestResult {
    ensure_string(
        r#"
        const original = String.prototype[Symbol.iterator];
        let observedType = "none";
        Object.defineProperty(String.prototype, Symbol.iterator, {
            configurable: true,
            get() {
                "use strict";
                observedType = typeof this;
                return original;
            }
        });
        const text = Iterator.from("ab").toArray().join(",");
        function source() {}
        source.index = 0;
        source.next = function () {
            source.index += 1;
            return { value: source.index, done: source.index > 2 };
        };
        text + ":" + observedType + ":" + Iterator.from(source).toArray().join(",")
        "#,
        "a,b:string:1,2",
    )
}
