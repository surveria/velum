use rs_quickjs::{Context, Engine, JsString, RuntimeLimits, Value};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn admits_portable_utf16_string_without_losing_surrogates() -> TestResult {
    let mut context = Context::new(RuntimeLimits::default());
    let portable = JsString::from_utf16(vec![0xD800]);
    context.register_host_function("portableString", move |_call| {
        Ok(Value::from(portable.clone()))
    })?;

    let value = context.eval("portableString().length + ':' + portableString().charCodeAt(0)")?;
    if value == Value::from("1:55296") {
        return Ok(());
    }
    Err(format!("unexpected portable string result: {value:?}").into())
}

#[test]
fn portable_string_payload_clone_shares_exact_semantics() -> TestResult {
    let utf8 = JsString::from_utf8("camera");
    let utf16 = JsString::from_utf16("camera".encode_utf16().collect());
    if utf8 == utf16 && utf8.identity().is_none() && utf16.identity().is_none() {
        return Ok(());
    }
    Err("portable string payloads diverged".into())
}

#[test]
fn reserves_utf16_and_lazy_utf8_bytes_before_rendering() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let value = vm.context().eval(r#""camera""#)?;
    let before = vm.resource_usage();
    let expected_bytes = "camera"
        .encode_utf16()
        .count()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add("camera".len());
    if before.string_count != 1 || before.string_bytes != expected_bytes {
        return Err(format!("unexpected string reservation: {before:?}").into());
    }
    let Value::String(text) = value else {
        return Err(format!("expected canonical string, got {value:?}").into());
    };
    if text.as_utf8() != Some("camera") {
        return Err(format!("unexpected UTF-8 rendering: {text:?}").into());
    }
    let after = vm.resource_usage();
    if after.string_count == before.string_count && after.string_bytes == before.string_bytes {
        return Ok(());
    }
    Err(format!("lazy rendering changed reserved storage: {before:?} -> {after:?}").into())
}
