use rs_quickjs::{Context, JsString, RuntimeLimits, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn admits_portable_utf16_string_without_losing_surrogates() -> TestResult {
    let mut context = Context::new(RuntimeLimits::default());
    let portable = JsString::from_utf16(vec![0xD800]);
    context.register_host_function("portableString", move |_call| {
        Ok(Value::HeapString(portable.clone()))
    })?;

    let value = context.eval("portableString().length + ':' + portableString().charCodeAt(0)")?;
    if value == Value::String("1:55296".to_owned()) {
        return Ok(());
    }
    Err(format!("unexpected portable string result: {value:?}").into())
}

#[test]
fn portable_string_payload_clone_shares_exact_semantics() -> TestResult {
    let utf8 = JsString::from_utf8("camera".to_owned());
    let utf16 = JsString::from_utf16("camera".encode_utf16().collect());
    if utf8 == utf16 && utf8.identity().is_none() && utf16.identity().is_none() {
        return Ok(());
    }
    Err("portable string payloads diverged".into())
}
