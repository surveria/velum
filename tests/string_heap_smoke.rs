use rs_quickjs::{Engine, Value, VmResourceUsage};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REFERENCE_ERROR_NAME: &str = "ReferenceError";
const MISSING_REFERENCE_MESSAGE: &str = "'missing' is not defined";
const ERROR_MESSAGE_PROPERTY: &str = "message";
const CAMERA_LABEL: &str = "camera";
const CAMERA_FIRST_CHAR: &str = "c";
const CAMERA_KEYS: &str = "0;1;2;3;4;5;";
const HOLDER_LABEL_SCRIPT: &str = r#"
var holder = {};
holder.label = "camera";
holder.label
"#;
const ECHO_LABEL_SCRIPT: &str = r#"
(function(value) {
    return value;
})("camera")
"#;

fn string_payload_bytes(value: &str) -> usize {
    value
        .encode_utf16()
        .count()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(value.len())
}

fn ensure_string_usage_unchanged(
    actual: &VmResourceUsage,
    expected: &VmResourceUsage,
) -> TestResult {
    ensure_usize(actual.string_count, expected.string_count)?;
    ensure_usize(actual.string_bytes, expected.string_bytes)
}

#[test]
fn tracks_heap_strings_without_reallocating_repeated_runtime_strings() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    ensure_usize(vm.resource_usage().string_count, 0)?;
    ensure_usize(vm.resource_usage().string_bytes, 0)?;

    let typeof_value = vm.context().eval("typeof neverDeclared")?;
    ensure_value(&typeof_value, &Value::from("undefined"))?;
    let after_typeof = vm.resource_usage();
    ensure_usize(after_typeof.string_count, 1)?;
    ensure_usize(after_typeof.string_bytes, string_payload_bytes("undefined"))?;

    let repeated_typeof = vm.context().eval("typeof anotherMissing")?;
    ensure_value(&repeated_typeof, &Value::from("undefined"))?;
    let after_repeated_typeof = vm.resource_usage();
    ensure_string_usage_unchanged(&after_repeated_typeof, &after_typeof)?;

    let literal_value = vm.context().eval(r#""front""#)?;
    ensure_value(&literal_value, &Value::from("front"))?;
    let after_literal = vm.resource_usage();
    ensure_usize(after_literal.string_count, 2)?;
    ensure_usize(
        after_literal.string_bytes,
        string_payload_bytes("undefinedfront"),
    )?;

    let repeated_literal = vm.context().eval("`front`")?;
    ensure_value(&repeated_literal, &Value::from("front"))?;
    let after_repeated_literal = vm.resource_usage();
    ensure_usize(
        after_repeated_literal.string_count,
        after_literal.string_count,
    )?;
    ensure_usize(
        after_repeated_literal.string_bytes,
        after_literal.string_bytes,
    )?;

    let concat_value = vm.context().eval(r#""front" + "-door""#)?;
    ensure_value(&concat_value, &Value::from("front-door"))?;
    let after_concat = vm.resource_usage();
    ensure_usize(after_concat.string_count, 3)?;
    ensure_usize(
        after_concat.string_bytes,
        string_payload_bytes("undefinedfrontfront-door"),
    )?;

    let repeated_concat = vm.context().eval(r#""front" + "-door""#)?;
    ensure_value(&repeated_concat, &Value::from("front-door"))?;
    let after_repeated_concat = vm.resource_usage();
    ensure_usize(
        after_repeated_concat.string_count,
        after_concat.string_count,
    )?;
    ensure_usize(
        after_repeated_concat.string_bytes,
        after_concat.string_bytes,
    )?;

    let static_index = vm.context().eval(r#""front"[1]"#)?;
    ensure_value(&static_index, &Value::from("r"))?;
    let after_static_index = vm.resource_usage();
    ensure_usize(
        after_static_index.string_count,
        after_concat.string_count.saturating_add(1),
    )?;
    ensure_usize(
        after_static_index.string_bytes,
        after_concat
            .string_bytes
            .saturating_add(string_payload_bytes("r")),
    )?;

    let repeated_static_index = vm.context().eval(r#""front"[1]"#)?;
    ensure_value(&repeated_static_index, &Value::from("r"))?;
    let after_repeated_static_index = vm.resource_usage();
    ensure_usize(
        after_repeated_static_index.string_count,
        after_static_index.string_count,
    )?;
    ensure_usize(
        after_repeated_static_index.string_bytes,
        after_static_index.string_bytes,
    )?;

    let dynamic_index = vm.context().eval(r#"let i = 1; "front"[i]"#)?;
    ensure_value(&dynamic_index, &Value::from("r"))?;
    let after_dynamic_index = vm.resource_usage();
    ensure_usize(
        after_dynamic_index.string_count,
        after_static_index.string_count,
    )?;
    ensure_usize(
        after_dynamic_index.string_bytes,
        after_static_index.string_bytes,
    )?;

    let unicode_index = vm.context().eval(r#""\u00e9x"[0]"#)?;
    ensure_value(&unicode_index, &Value::from("\u{00e9}"))?;
    let after_unicode_index = vm.resource_usage();
    ensure_usize(
        after_unicode_index.string_count,
        after_static_index.string_count.saturating_add(2),
    )?;
    ensure_usize(
        after_unicode_index.string_bytes,
        after_static_index
            .string_bytes
            .saturating_add(string_payload_bytes("\u{00e9}x"))
            .saturating_add(string_payload_bytes("\u{00e9}")),
    )
}

#[test]
fn string_concat_uses_heap_dedup_and_respects_limits() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm
        .context()
        .eval(r#"var name = "camera"; name + "-stream-" + 7"#)?;
    ensure_value(&value, &Value::from("camera-stream-7"))?;
    let after_first = vm.resource_usage();

    let repeated = vm
        .context()
        .eval(r#"var name = "camera"; name + "-stream-" + 7"#)?;
    ensure_value(&repeated, &Value::from("camera-stream-7"))?;
    let after_repeated = vm.resource_usage();
    ensure_usize(after_repeated.string_count, after_first.string_count)?;
    ensure_usize(after_repeated.string_bytes, after_first.string_bytes)?;

    let max_string_len = "camera-stream".len().saturating_sub(1);
    let runtime = rs_quickjs::Runtime::with_limits(rs_quickjs::RuntimeLimits {
        max_string_len,
        ..rs_quickjs::RuntimeLimits::default()
    });
    let mut context = runtime.context();
    let Err(error) = context.eval(r#""camera" + "-stream""#) else {
        return Err("expected string concat limit to fail".into());
    };
    ensure_text(error.to_string().as_str(), "resource limit")
}

#[test]
fn bytecode_string_concat_chain_interns_only_the_final_result() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(r#"var name = "camera"; name + "-stream-" + 7"#)?;

    let value = vm.eval_compiled(&script)?;
    ensure_value(&value, &Value::from("camera-stream-7"))?;
    let usage = vm.resource_usage();

    ensure_usize(usage.string_count, 2)?;
    ensure_usize(
        usage.string_bytes,
        string_payload_bytes("cameracamera-stream-7"),
    )
}

#[test]
fn bytecode_string_concat_chain_preserves_numeric_prefix_addition() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(r#"1 + 2 + "-stream""#)?;
    ensure_value(&value, &Value::from("3-stream"))
}

#[test]
fn interns_error_properties_in_vm_heap() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    ensure_usize(vm.resource_usage().string_count, 0)?;
    ensure_usize(vm.resource_usage().string_bytes, 0)?;

    let name = vm
        .context()
        .eval("try { missing; } catch (error) { error.name }")?;
    ensure_value(&name, &Value::from(REFERENCE_ERROR_NAME))?;
    let after_name = vm.resource_usage();
    ensure_at_least(after_name.string_count, 2, "error property strings")?;
    ensure_at_least(
        after_name.string_bytes,
        string_payload_bytes(REFERENCE_ERROR_NAME)
            .saturating_add(string_payload_bytes(MISSING_REFERENCE_MESSAGE)),
        "error property string bytes",
    )?;

    let repeated_name = vm
        .context()
        .eval("try { missing; } catch (error) { error.name }")?;
    ensure_value(&repeated_name, &Value::from(REFERENCE_ERROR_NAME))?;
    let after_repeated_name = vm.resource_usage();
    ensure_usize(after_repeated_name.string_count, after_name.string_count)?;
    ensure_usize(after_repeated_name.string_bytes, after_name.string_bytes)?;

    let message = vm
        .context()
        .eval("try { missing; } catch (error) { error.message }")?;
    ensure_value(&message, &Value::from(MISSING_REFERENCE_MESSAGE))?;
    let after_message = vm.resource_usage();
    ensure_usize(after_message.string_count, after_name.string_count)?;
    ensure_usize(after_message.string_bytes, after_name.string_bytes)?;

    let dynamic_message = vm.context().eval(
        r#"
        let key = "message";
        try { missing; } catch (error) { error[key] }
        "#,
    )?;
    ensure_value(&dynamic_message, &Value::from(MISSING_REFERENCE_MESSAGE))?;
    let after_dynamic_message = vm.resource_usage();
    ensure_usize(
        after_dynamic_message.string_count,
        after_message.string_count.saturating_add(1),
    )?;
    ensure_usize(
        after_dynamic_message.string_bytes,
        after_message
            .string_bytes
            .saturating_add(string_payload_bytes(ERROR_MESSAGE_PROPERTY)),
    )
}

#[test]
fn keeps_string_wrapper_indices_virtual_and_heap_backed() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.context().eval("String")?;
    let after_constructor = vm.resource_usage();

    let length = vm
        .context()
        .eval(r#"let boxed = new String("camera"); boxed.length"#)?;
    ensure_value(&length, &Value::Number(6.0))?;
    let after_construct = vm.resource_usage();
    ensure_usize(
        after_construct.string_count,
        after_constructor.string_count.saturating_add(1),
    )?;
    ensure_usize(
        after_construct.string_bytes,
        after_constructor
            .string_bytes
            .saturating_add(string_payload_bytes(CAMERA_LABEL)),
    )?;

    let first = vm.context().eval("boxed[0]")?;
    ensure_value(&first, &Value::from(CAMERA_FIRST_CHAR))?;
    let after_first = vm.resource_usage();
    ensure_usize(
        after_first.string_count,
        after_construct.string_count.saturating_add(1),
    )?;
    ensure_usize(
        after_first.string_bytes,
        after_construct
            .string_bytes
            .saturating_add(string_payload_bytes(CAMERA_FIRST_CHAR)),
    )?;

    let repeated_first = vm.context().eval("boxed[0]")?;
    ensure_value(&repeated_first, &Value::from(CAMERA_FIRST_CHAR))?;
    let after_repeated_first = vm.resource_usage();
    ensure_usize(after_repeated_first.string_count, after_first.string_count)?;
    ensure_usize(after_repeated_first.string_bytes, after_first.string_bytes)?;

    let delete_first = vm.context().eval("delete boxed[0]")?;
    ensure_value(&delete_first, &Value::Bool(false))?;
    let first_after_delete = vm.context().eval("boxed[0]")?;
    ensure_value(&first_after_delete, &Value::from(CAMERA_FIRST_CHAR))?;

    let keys = vm.context().eval(
        r#"
        let keys = "";
        for (let key in boxed) {
            keys = keys + key + ";";
        }
        keys
        "#,
    )?;
    ensure_value(&keys, &Value::from(CAMERA_KEYS))?;

    let after_first_wrapper_shape_count = after_construct.shape_count;
    let short_length = vm
        .context()
        .eval(r#"let shortBoxed = new String("go"); shortBoxed.length"#)?;
    ensure_value(&short_length, &Value::Number(2.0))?;
    ensure_usize(
        vm.resource_usage().shape_count,
        after_first_wrapper_shape_count,
    )
}

#[test]
fn inherited_string_wrapper_indices_are_heap_backed() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.context().eval("String")?;
    let after_constructor = vm.resource_usage();

    let first = vm.context().eval(
        r#"
        var stringProto = new String("camera");
        var child = {};
        child.__proto__ = stringProto;
        child[0]
        "#,
    )?;
    ensure_heap_string(&first, CAMERA_FIRST_CHAR)?;
    let after_first = vm.resource_usage();
    ensure_usize(
        after_first.string_count,
        after_constructor.string_count.saturating_add(2),
    )?;
    ensure_usize(
        after_first.string_bytes,
        after_constructor
            .string_bytes
            .saturating_add(string_payload_bytes(CAMERA_LABEL))
            .saturating_add(string_payload_bytes(CAMERA_FIRST_CHAR)),
    )?;

    let repeated = vm.context().eval("child[0]")?;
    ensure_heap_string(&repeated, CAMERA_FIRST_CHAR)?;
    let after_repeated = vm.resource_usage();
    ensure_usize(after_repeated.string_count, after_first.string_count)?;
    ensure_usize(after_repeated.string_bytes, after_first.string_bytes)
}

#[test]
fn interns_string_wrapper_descriptor_values_in_vm_heap() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    vm.context().eval(
        r#"
        (function() {
        let boxed = new String("warm");
        return Object.getOwnPropertyDescriptor(boxed, 0).value;
        })()
        "#,
    )?;
    let after_warmup = vm.resource_usage();

    vm.context().eval(
        r#"
        var descriptorBoxed = new String("camera");
        var descriptorValue = Object.getOwnPropertyDescriptor(descriptorBoxed, 0);
        descriptorValue
        "#,
    )?;
    let after_descriptor = vm.resource_usage();
    ensure_usize(
        after_descriptor.string_count,
        after_warmup.string_count.saturating_add(2),
    )?;
    ensure_usize(
        after_descriptor.string_bytes,
        after_warmup
            .string_bytes
            .saturating_add(string_payload_bytes(CAMERA_LABEL))
            .saturating_add(string_payload_bytes(CAMERA_FIRST_CHAR)),
    )?;

    let value = vm.context().eval("descriptorValue.value")?;
    ensure_heap_string(&value, CAMERA_FIRST_CHAR)?;
    let after_value_read = vm.resource_usage();
    ensure_usize(after_value_read.string_count, after_descriptor.string_count)?;
    ensure_usize(after_value_read.string_bytes, after_descriptor.string_bytes)?;

    let repeated = vm.context().eval(
        r#"
        (function() {
        let boxed = new String("camera");
        return Object.getOwnPropertyDescriptor(boxed, 0).value;
        })()
        "#,
    )?;
    ensure_heap_string(&repeated, CAMERA_FIRST_CHAR)?;
    let after_repeated = vm.resource_usage();
    ensure_usize(after_repeated.string_count, after_descriptor.string_count)?;
    ensure_usize(after_repeated.string_bytes, after_descriptor.string_bytes)
}

#[test]
fn normalizes_context_owned_strings_after_storage_boundaries() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let retained = vm.context().eval(r#"var retained = "camera"; retained"#)?;
    ensure_heap_string(&retained, CAMERA_LABEL)?;
    let after_retained = vm.resource_usage();
    ensure_usize(after_retained.string_count, 1)?;
    ensure_usize(
        after_retained.string_bytes,
        string_payload_bytes(CAMERA_LABEL),
    )?;

    let repeated = vm.context().eval("retained")?;
    ensure_heap_string(&repeated, CAMERA_LABEL)?;
    let after_repeated = vm.resource_usage();
    ensure_usize(after_repeated.string_count, after_retained.string_count)?;
    ensure_usize(after_repeated.string_bytes, after_retained.string_bytes)?;

    let property = vm.context().eval(HOLDER_LABEL_SCRIPT)?;
    ensure_heap_string(&property, CAMERA_LABEL)?;
    let after_property = vm.resource_usage();
    ensure_usize(after_property.string_count, after_retained.string_count)?;
    ensure_usize(after_property.string_bytes, after_retained.string_bytes)?;

    let parameter = vm.context().eval(ECHO_LABEL_SCRIPT)?;
    ensure_heap_string(&parameter, CAMERA_LABEL)?;
    let after_parameter = vm.resource_usage();
    ensure_usize(
        after_parameter.string_count,
        after_retained.string_count.saturating_add(1),
    )?;
    ensure_usize(after_parameter.string_bytes, after_retained.string_bytes)
}

#[test]
fn compiled_string_literals_use_script_local_constants() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    let script = vm.compile(
        r#"
        var holder = { camera: "front" };
        var repeated = "front";
        holder["camera"];
        "#,
    )?;

    ensure_usize(script.usage().static_string_count(), 2)?;
    let value = vm.eval_compiled(&script)?;
    ensure_heap_string(&value, "front")?;
    let after_first = vm.resource_usage();
    ensure_usize(after_first.string_count, 1)?;
    ensure_usize(after_first.string_bytes, string_payload_bytes("front"))?;

    let value = vm.eval_compiled(&script)?;
    ensure_heap_string(&value, "front")?;
    let after_second = vm.resource_usage();
    ensure_usize(after_second.string_count, after_first.string_count)?;
    ensure_usize(after_second.string_bytes, after_first.string_bytes)
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_heap_string(actual: &Value, expected: &str) -> TestResult {
    let Value::String(value) = actual else {
        return Err(format!("expected heap string {expected:?}, got {actual:?}").into());
    };
    if value.as_str() == expected {
        return Ok(());
    }
    Err(format!(
        "expected heap string {expected:?}, got {:?}",
        value.as_str()
    )
    .into())
}

fn ensure_text(actual: &str, expected: &str) -> TestResult {
    if actual.contains(expected) {
        return Ok(());
    }
    Err(format!("expected text {actual:?} to contain {expected:?}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}

fn ensure_at_least(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual >= minimum {
        return Ok(());
    }
    Err(format!("expected {label} to be at least {minimum}, got {actual}").into())
}
