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
        &Value::String("set:5".to_owned()),
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
        &Value::String("1:2:3".to_owned()),
    )
}

#[test]
fn get_and_set_stay_usable_as_plain_property_names() -> TestResult {
    ensure_eval(
        "var o = { get: 1, set: 2, get get() { return 3; } };
         var m = { get() { return 5; } };
         o.get + ':' + o.set + ':' + m.get()",
        &Value::String("3:2:5".to_owned()),
    )
}

#[test]
fn prototype_getter_sees_receiver_as_this() -> TestResult {
    ensure_eval(
        "function C() { this.tag = 'T'; }
         C.prototype = { get x() { return this.tag; } };
         new C().x",
        &Value::String("T".to_owned()),
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
        &Value::String("42:false".to_owned()),
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
        &Value::String("function:undefined:true:true:false:false".to_owned()),
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
        &Value::String("x:false".to_owned()),
    )
}

#[test]
fn delete_removes_accessor_property() -> TestResult {
    ensure_eval(
        "var o = { get x() { return 1; } };
         var removed = delete o.x;
         removed + ':' + (o.x === undefined)",
        &Value::String("true:true".to_owned()),
    )
}

#[test]
fn getter_can_throw_catchable_error() -> TestResult {
    ensure_eval(
        "var o = { get x() { throw new Error('boom'); } };
         var caught = '';
         try { o.x; } catch (e) { caught = e.message; }
         caught",
        &Value::String("boom".to_owned()),
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
