use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

fn eval(source: &str) -> velum::Result<Value> {
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

fn ensure_eval(source: &str, expected: &Value) -> TestResult {
    let value = eval(source)?;
    ensure_value(&value, expected)
}

#[test]
fn object_literal_getter_returns_value() -> TestResult {
    ensure_eval(
        "var o = { get x() { return 7; } }; o.x",
        &Value::Number(7.0),
    )
}

#[test]
fn object_literal_setter_receives_assigned_value() -> TestResult {
    ensure_eval(
        "var log = ''; var o = { set x(v) { log += 'set:' + v; } }; o.x = 5; log",
        &Value::from("set:5"),
    )
}

#[test]
fn setter_only_property_reads_as_undefined() -> TestResult {
    ensure_eval(
        "var o = { set x(v) {} }; o.x === undefined",
        &Value::Bool(true),
    )
}

#[test]
fn getter_and_setter_merge_into_one_property() -> TestResult {
    ensure_eval(
        "var o = { _v: 1, get x() { return this._v; }, set x(v) { this._v = v; } };
         o.x = 41; o.x + 1",
        &Value::Number(42.0),
    )
}

#[test]
fn accessor_keys_support_string_number_and_computed_forms() -> TestResult {
    ensure_eval(
        "var key = 'c';
         var o = {
             get 'a b'() { return 1; },
             get 42() { return 2; },
             get [key]() { return 3; }
         };
         o['a b'] + ':' + o[42] + ':' + o.c",
        &Value::from("1:2:3"),
    )
}

#[test]
fn get_and_set_stay_usable_as_plain_property_names() -> TestResult {
    ensure_eval(
        "var o = { get: 1, set: 2, get get() { return 3; } };
         var m = { get() { return 5; } };
         o.get + ':' + o.set + ':' + m.get()",
        &Value::from("3:2:5"),
    )
}

#[test]
fn prototype_getter_sees_receiver_as_this() -> TestResult {
    ensure_eval(
        "function C() { this.tag = 'T'; }
         C.prototype = { get x() { return this.tag; } };
         new C().x",
        &Value::from("T"),
    )
}

#[test]
fn prototype_setter_intercepts_assignment_on_instance() -> TestResult {
    ensure_eval(
        "function C() {}
         C.prototype = { set x(v) { this.stored = v * 2; } };
         var c = new C();
         c.x = 21;
         c.stored + ':' + Object.prototype.hasOwnProperty.call(c, 'x')",
        &Value::from("42:false"),
    )
}

#[test]
fn getter_only_assignment_is_ignored_in_sloppy_mode() -> TestResult {
    ensure_eval(
        "var o = { get x() { return 1; } }; o.x = 99; o.x",
        &Value::Number(1.0),
    )
}

#[test]
fn define_property_supports_accessor_descriptors() -> TestResult {
    ensure_eval(
        "var o = {};
         Object.defineProperty(o, 'x', {
             get: function () { return 11; },
             configurable: true
         });
         o.x",
        &Value::Number(11.0),
    )
}

#[test]
fn get_own_property_descriptor_reports_accessor_fields() -> TestResult {
    ensure_eval(
        "var o = { get x() { return 1; } };
         var d = Object.getOwnPropertyDescriptor(o, 'x');
         (typeof d.get) + ':' + (typeof d.set) + ':' + d.enumerable + ':' + d.configurable
             + ':' + ('value' in d) + ':' + ('writable' in d)",
        &Value::from("function:undefined:true:true:false:false"),
    )
}

#[test]
fn define_property_rejects_mixed_data_and_accessor_descriptor() -> TestResult {
    ensure_eval(
        "var result = 'no error';
         try {
             Object.defineProperty({}, 'x', { value: 1, get: function () {} });
         } catch (e) {
             result = e instanceof TypeError;
         }
         result",
        &Value::Bool(true),
    )
}

#[test]
fn define_property_rejects_non_callable_getter() -> TestResult {
    ensure_eval(
        "var result = 'no error';
         try {
             Object.defineProperty({}, 'x', { get: 1 });
         } catch (e) {
             result = e instanceof TypeError;
         }
         result",
        &Value::Bool(true),
    )
}

#[test]
fn for_in_enumerates_accessor_key_without_calling_getter() -> TestResult {
    ensure_eval(
        "var called = false;
         var o = { get x() { called = true; return 1; } };
         var keys = '';
         for (var k in o) { keys += k; }
         keys + ':' + called",
        &Value::from("x:false"),
    )
}

#[test]
fn delete_removes_accessor_property() -> TestResult {
    ensure_eval(
        "var o = { get x() { return 1; } };
         var removed = delete o.x;
         removed + ':' + (o.x === undefined)",
        &Value::from("true:true"),
    )
}

#[test]
fn getter_can_throw_catchable_error() -> TestResult {
    ensure_eval(
        "var o = { get x() { throw new Error('boom'); } };
         var caught = '';
         try { o.x; } catch (e) { caught = e.message; }
         caught",
        &Value::from("boom"),
    )
}

#[test]
fn array_builtins_observe_index_accessors() -> TestResult {
    ensure_eval(
        r#"
        let searchReads = 0;
        let search = [1, 2, 3];
        Object.defineProperty(search, "1", {
            get: function() {
                searchReads = searchReads + 1;
                return 42;
            },
            enumerable: true,
            configurable: true
        });
        let index = search.indexOf(42);
        let included = search.includes(42);
        let lastIndex = search.lastIndexOf(42);
        let joined = search.join("|");

        let popReads = 0;
        let poppedSource = [1, 2];
        Object.defineProperty(poppedSource, "1", {
            get: function() {
                popReads = popReads + 1;
                return 9;
            },
            configurable: true
        });
        let popped = poppedSource.pop();

        let reverseReads = 0;
        let reverseWrite = 0;
        let reversed = [1, 2];
        Object.defineProperty(reversed, "0", {
            get: function() {
                reverseReads = reverseReads + 1;
                return 10;
            },
            set: function(value) {
                reverseWrite = value;
            },
            enumerable: true,
            configurable: true
        });
        reversed.reverse();

        let pushedValue = 0;
        let pushPrototype = Object.create(Array.prototype);
        Object.defineProperty(pushPrototype, "2", {
            set: function(value) {
                pushedValue = value;
            },
            configurable: true
        });
        let pushed = [1, 2];
        Object.setPrototypeOf(pushed, pushPrototype);
        let pushedLength = pushed.push(7);

        index === 1 && included && lastIndex === 1 && joined === "1|42|3" &&
            searchReads === 4 &&
            popped === 9 && popReads === 1 && poppedSource.length === 1 &&
            reverseReads === 1 && reverseWrite === 2 && reversed[1] === 10 &&
            pushedValue === 7 && pushedLength === 3 &&
            !Object.prototype.hasOwnProperty.call(pushed, "2")
        "#,
        &Value::Bool(true),
    )
}

#[test]
fn setter_parameter_count_is_validated() -> TestResult {
    let Err(error) = eval("var o = { set x() {} };") else {
        return Err("expected setter without parameter to fail parsing".into());
    };
    let message = error.to_string();
    if message.contains("setter") {
        return Ok(());
    }
    Err(format!("expected setter arity error, got '{message}'").into())
}

#[test]
fn getter_parameter_count_is_validated() -> TestResult {
    let Err(error) = eval("var o = { get x(v) {} };") else {
        return Err("expected getter with parameter to fail parsing".into());
    };
    let message = error.to_string();
    if message.contains("getter") {
        return Ok(());
    }
    Err(format!("expected getter arity error, got '{message}'").into())
}
