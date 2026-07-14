use rs_quickjs::{HostOperation, Runtime, Value, VmStorageKind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DETACH_NAME: &str = "hostDetachArrayBuffer";
const CREATE_IS_HTML_DDA_NAME: &str = "hostCreateIsHTMLDDA";

#[test]
fn creates_callable_is_html_dda_host_exotics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(CREATE_IS_HTML_DDA_NAME, HostOperation::CreateIsHtmlDda)?;
    let value = context.eval(
        "var dda = hostCreateIsHTMLDDA(); \
         dda.answer = 42; \
         !dda && typeof dda === 'undefined' && \
         dda == null && null == dda && dda == undefined && undefined == dda && \
         dda !== null && dda !== undefined && Object.is(dda, dda) && \
         dda() === null && dda.answer === 42 && \
         Object.getPrototypeOf(dda) === Function.prototype ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected IsHTMLDDA semantic invariants, got {value:?}").into())
}

#[test]
fn detaches_array_buffers_and_reconciles_storage() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(DETACH_NAME, HostOperation::DetachArrayBuffer)?;
    context.eval("var buffer = new ArrayBuffer(16); var view = new Uint8Array(buffer);")?;
    ensure_payload(&context, 16, "allocated byte payload")?;

    let value = context.eval(
        "hostDetachArrayBuffer(buffer); \
         buffer.byteLength === 0 && view.length === 0 && view[0] === undefined ? 42 : 0",
    )?;
    if value != Value::Number(42.0) {
        return Err(format!("expected detached buffer observation, got {value:?}").into());
    }
    ensure_payload(&context, 0, "detached byte payload")
}

#[test]
fn rejects_shared_buffers_and_non_buffers() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(DETACH_NAME, HostOperation::DetachArrayBuffer)?;
    let value = context.eval(
        "var sharedRejected = false; var objectRejected = false; \
         try { hostDetachArrayBuffer(new SharedArrayBuffer(8)); } \
         catch (error) { sharedRejected = error instanceof TypeError; } \
         try { hostDetachArrayBuffer({}); } \
         catch (error) { objectRejected = error instanceof TypeError; } \
         sharedRejected && objectRejected ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected detachment type errors, got {value:?}").into())
}

#[test]
fn preserves_detached_buffer_and_typed_array_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(DETACH_NAME, HostOperation::DetachArrayBuffer)?;
    let value = context.eval(
        "var resizable = new ArrayBuffer(4, { maxByteLength: 8 }); \
         var fixed = new Int8Array(resizable, 0, 2); \
         fixed.__proto__ = { 0: 99 }; \
         resizable.resize(0); \
         var ignoresPrototype = fixed[0] === undefined && !(0 in fixed); \
         var resultBuffer = new ArrayBuffer(2); \
         var result = new Int8Array(resultBuffer); \
         var mapped = Int8Array.from.call(function() { return result; }, [1, 2], \
             function(value) { if (value === 2) hostDetachArrayBuffer(resultBuffer); return value; }); \
         var constructorBuffer = new ArrayBuffer(4); \
         var rejected = false; \
         try { new Int8Array(constructorBuffer, { valueOf: function() { \
             hostDetachArrayBuffer(constructorBuffer); return 0; } }); } \
         catch (error) { rejected = error instanceof TypeError; } \
         var propertyBuffer = new ArrayBuffer(1, { maxByteLength: 1 }); \
         hostDetachArrayBuffer(propertyBuffer); \
         ignoresPrototype && mapped === result && result.length === 0 && rejected && \
             propertyBuffer.resizable === true ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected detached semantic invariants, got {value:?}").into())
}

#[test]
fn typed_array_callbacks_visit_initial_indices_after_detachment() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(DETACH_NAME, HostOperation::DetachArrayBuffer)?;
    let value = context.eval(
        "var buffer = new ArrayBuffer(2); var sample = new Int8Array(buffer); \
         var calls = 0; var second; \
         var result = sample.every(function(value) { \
             if (calls === 0) hostDetachArrayBuffer(buffer); \
             if (calls === 1) second = value; \
             calls = calls + 1; return true; \
         }); \
         result === true && calls === 2 && second === undefined ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected stable callback iteration, got {value:?}").into())
}

fn ensure_payload(context: &rs_quickjs::Context, expected: usize, label: &str) -> TestResult {
    let actual = context
        .storage_snapshot()?
        .payload_bytes(VmStorageKind::ByteBuffer);
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {label} {expected}, got {actual}").into())
}
