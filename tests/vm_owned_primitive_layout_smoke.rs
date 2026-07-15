use std::mem::size_of;

use velum::{JsString, JsSymbol, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn keeps_vm_owned_primitive_handles_compact() -> TestResult {
    let owned_string_size = size_of::<String>();
    for (name, actual) in [
        ("JsString", size_of::<JsString>()),
        ("JsSymbol", size_of::<JsSymbol>()),
        ("Value", size_of::<Value>()),
    ] {
        if actual > owned_string_size {
            return Err(format!(
                "{name} grew to {actual} bytes, above the {owned_string_size}-byte owned String baseline"
            )
            .into());
        }
    }
    Ok(())
}
