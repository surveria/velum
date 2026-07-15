use rs_quickjs::{HostOperation, Runtime, Value, VmStorageKind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DETACH_NAME: &str = "hostDetachArrayBuffer";
const COLLECT_GARBAGE_NAME: &str = "hostCollectGarbage";
const CREATE_REALM_NAME: &str = "hostCreateRealm";
const CREATE_IS_HTML_DDA_NAME: &str = "hostCreateIsHTMLDDA";
const EVAL_SCRIPT_NAME: &str = "hostEvalScript";
const GET_ABSTRACT_MODULE_SOURCE_NAME: &str = "hostGetAbstractModuleSource";

#[test]
fn collects_garbage_during_active_evaluation_without_losing_live_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(COLLECT_GARBAGE_NAME, HostOperation::CollectGarbage)?;
    let value = context.eval(
        "(function() { \
             var live = { answer: 42 }; \
             var discarded = { nested: [1, 2, 3] }; \
             hostCollectGarbage(); \
             return live.answer; \
         })()",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected live value to survive host GC, got {value:?}").into())
}

#[test]
fn host_gc_preserves_detached_buffer_constructor_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(COLLECT_GARBAGE_NAME, HostOperation::CollectGarbage)?;
    context.register_host_operation(DETACH_NAME, HostOperation::DetachArrayBuffer)?;
    let value = context.eval(
        "(function() { \
             var buffer = new ArrayBuffer(4096); \
             var offset = { valueOf: function() { \
                 hostDetachArrayBuffer(buffer); \
                 hostCollectGarbage(); \
                 return 2048; \
             } }; \
             try { \
                 new DataView(buffer, offset); \
             } catch (error) { \
                 return error.constructor === TypeError && buffer.byteLength === 0 ? 42 : 0; \
             } \
             return 0; \
         })()",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected detached DataView construction to throw TypeError, got {value:?}").into())
}

#[test]
fn creates_callable_is_html_dda_host_exotics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(CREATE_IS_HTML_DDA_NAME, HostOperation::CreateIsHtmlDda)?;
    let value = context.eval(
        "var dda = hostCreateIsHTMLDDA(); \
         dda.answer = 42; \
         function Superclass() {} \
         Superclass.prototype = dda; \
         class Derived extends Superclass {} \
         var derived = new Derived(); \
         !dda && typeof dda === 'undefined' && \
         dda == null && null == dda && dda == undefined && undefined == dda && \
         dda !== null && dda !== undefined && Object.is(dda, dda) && \
         dda() === null && dda.answer === 42 && \
         Object.getPrototypeOf(dda) === Function.prototype && \
         Object.getPrototypeOf(Derived.prototype) === dda && \
         derived instanceof Derived && derived instanceof Superclass ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected IsHTMLDDA semantic invariants, got {value:?}").into())
}

#[test]
fn exposes_abstract_module_source_intrinsic_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(
        GET_ABSTRACT_MODULE_SOURCE_NAME,
        HostOperation::GetAbstractModuleSource,
    )?;
    let value = context.eval(
        "var constructor = hostGetAbstractModuleSource(); \
         var prototype = constructor.prototype; \
         var tag = Object.getOwnPropertyDescriptor(prototype, Symbol.toStringTag); \
         delete prototype[Symbol.toStringTag]; \
         Object.defineProperty(prototype, Symbol.toStringTag, tag); \
         var threw = false; \
         try { new constructor(); } catch (error) { threw = error instanceof TypeError; } \
         typeof constructor === 'function' && constructor.name === 'AbstractModuleSource' && \
             constructor.length === 0 && Object.getPrototypeOf(prototype) === Object.prototype && \
             typeof tag.get === 'function' && tag.set === undefined && !tag.enumerable && \
             tag.configurable && tag.get.call(262) === undefined && threw ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected AbstractModuleSource intrinsic invariants, got {value:?}").into())
}

#[test]
fn resolves_abstract_module_source_for_an_explicit_realm_global() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(CREATE_REALM_NAME, HostOperation::CreateRealm)?;
    context.register_host_operation(
        GET_ABSTRACT_MODULE_SOURCE_NAME,
        HostOperation::GetAbstractModuleSource,
    )?;
    let value = context.eval(
        "var child = hostCreateRealm(); \
         var parentConstructor = hostGetAbstractModuleSource(); \
         var childConstructor = hostGetAbstractModuleSource(child); \
         var invalidTargetRejected = false; \
         try { hostGetAbstractModuleSource({}); } \
         catch (error) { invalidTargetRejected = error instanceof TypeError; } \
         parentConstructor !== childConstructor && \
             Object.getPrototypeOf(childConstructor.prototype) === child.Object.prototype && \
             childConstructor.prototype.constructor === childConstructor && \
             invalidTargetRejected ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected a realm-owned AbstractModuleSource intrinsic, got {value:?}").into())
}

#[test]
fn eval_script_operation_uses_global_script_environment() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(EVAL_SCRIPT_NAME, HostOperation::EvalScript)?;
    let value = context.eval(
        "function run() { \
             let localOnly = 1; \
             hostEvalScript('var scriptVar = 2; let scriptLexical = 3;'); \
             return typeof localOnly + ':' + scriptVar + ':' + scriptLexical; \
         } \
         var result = run(); \
         var varIsPermanent = delete scriptVar === false; \
         var lexicalIsNotProperty = \
             !Object.prototype.hasOwnProperty.call(globalThis, 'scriptLexical'); \
         result === 'number:2:3' && varIsPermanent && lexicalIsNotProperty ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected global Script evaluation invariants, got {value:?}").into())
}

#[test]
fn eval_script_preflights_global_declarations_atomically() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation(EVAL_SCRIPT_NAME, HostOperation::EvalScript)?;
    let value = context.eval(
        "hostEvalScript('var declared; function declaredFunction() {}'); \
         var lexicalCollision = false; \
         try { hostEvalScript('var leaked; let declared;'); } \
         catch (error) { lexicalCollision = error instanceof SyntaxError; } \
         Object.defineProperty(globalThis, 'configurableGlobal', { \
             value: 7, configurable: true \
         }); \
         hostEvalScript('let configurableGlobal = 8;'); \
         var configurableLexical = configurableGlobal === 8 && \
             globalThis.configurableGlobal === 7; \
         Object.defineProperty(globalThis, 'existingVar', { \
             value: 9, writable: false, enumerable: false, configurable: true \
         }); \
         hostEvalScript('var existingVar;'); \
         var descriptor = Object.getOwnPropertyDescriptor(globalThis, 'existingVar'); \
         var descriptorPreserved = descriptor.value === 9 && !descriptor.writable && \
             !descriptor.enumerable && descriptor.configurable; \
         Object.preventExtensions(globalThis); \
         hostEvalScript('var existingVar;'); \
         var rejectedNewVar = false; \
         try { hostEvalScript('var brandNew;'); } \
         catch (error) { rejectedNewVar = error instanceof TypeError; } \
         lexicalCollision && typeof leaked === 'undefined' && configurableLexical && \
             descriptorPreserved && rejectedNewVar ? 42 : 0",
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("expected atomic global declaration instantiation, got {value:?}").into())
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
