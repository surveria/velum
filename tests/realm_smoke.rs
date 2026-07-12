use rs_quickjs::{HostOperation, Runtime, Value};

type TestResult = rs_quickjs::Result<()>;

#[test]
fn realms_share_vm_storage_but_isolate_globals_and_intrinsics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let root = context.current_realm();
    let other = context.create_realm()?;

    assert_eq!(
        context.eval_in_realm(&root, "globalThis.marker = 11; marker")?,
        Value::Number(11.0)
    );
    assert_eq!(
        context.eval_in_realm(&other, "globalThis.marker = 29; marker")?,
        Value::Number(29.0)
    );
    assert_eq!(context.eval("marker")?, Value::Number(11.0));

    let root_global = context.realm_global(&root)?;
    let other_global = context.realm_global(&other)?;
    assert_ne!(root_global, other_global);

    let root_array = context.eval_in_realm(&root, "Array")?;
    let other_array = context.eval_in_realm(&other, "Array")?;
    assert_ne!(root_array, other_array);
    context.storage_snapshot()?;
    Ok(())
}

#[test]
fn realm_switch_restores_the_caller_after_abrupt_completion() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval("globalThis.marker = 'root'")?;
    let other = context.create_realm()?;

    let result = context.eval_in_realm(
        &other,
        "globalThis.marker = 'other'; throw new Error('stop')",
    );
    assert!(result.is_err());
    assert_eq!(context.eval("marker")?, Value::String("root".to_owned()));
    assert_eq!(
        context.eval_in_realm(&other, "marker")?,
        Value::String("other".to_owned())
    );
    Ok(())
}

#[test]
fn realm_handles_cannot_cross_vm_boundaries() -> TestResult {
    let runtime = Runtime::new();
    let mut first = runtime.context();
    let mut second = runtime.context();
    let realm = first.create_realm()?;

    let error = second
        .eval_in_realm(&realm, "1")
        .err()
        .ok_or_else(|| rs_quickjs::Error::runtime("foreign realm was accepted"))?;
    assert!(error.to_string().contains("another VM"));
    Ok(())
}

#[test]
fn test262_realm_host_preserves_function_origin_and_default_prototype() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation("__createRealm", HostOperation::CreateRealm)?;
    context
        .eval("var $262 = { createRealm: function () { return { global: __createRealm() }; } };")?;

    let result = context.eval(
        r#"
        var realmA = $262.createRealm().global;
        realmA.calls = 0;
        var realmB = $262.createRealm().global;
        var newTarget = new realmB.Function();
        newTarget.prototype = null;
        var fn = Reflect.construct(realmA.Function, ["calls += 1;"], newTarget);
        var prototypeOk = Object.getPrototypeOf(fn) === realmB.Function.prototype;
        var instanceOk = new fn() instanceof realmA.Object;
        prototypeOk && instanceOk && realmA.calls === 1;
        "#,
    )?;
    assert_eq!(result, Value::Bool(true));
    Ok(())
}

#[test]
fn foreign_async_function_exposes_its_origin_realm_prototype() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation("__createRealm", HostOperation::CreateRealm)?;
    let result = context.eval(
        r#"
        var other = __createRealm();
        var AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
        var OtherAsyncFunction = Object.getPrototypeOf(
            other.eval("(0, async function () {})")
        ).constructor;
        var newTarget = new other.Function();
        newTarget.prototype = null;
        var fn = Reflect.construct(AsyncFunction, [], newTarget);
        Object.getPrototypeOf(fn) === OtherAsyncFunction.prototype;
        "#,
    )?;
    assert_eq!(result, Value::Bool(true));
    Ok(())
}

#[test]
fn foreign_generator_constructor_separates_new_target_and_body_realms() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation("__createRealm", HostOperation::CreateRealm)?;
    let result = context.eval(
        r#"
        var realmA = __createRealm();
        realmA.calls = 0;
        var aGeneratorFunction = realmA.eval("(function* () {})").constructor;
        var aGeneratorPrototype = Object.getPrototypeOf(
            realmA.eval("(function* () {})").prototype
        );
        var realmB = __createRealm();
        var bGeneratorFunction = realmB.eval("(function* () {})").constructor;
        var newTarget = new realmB.Function();
        newTarget.prototype = null;
        var fn = Reflect.construct(aGeneratorFunction, ["calls += 1;"], newTarget);
        var functionPrototypeOk =
            Object.getPrototypeOf(fn) === bGeneratorFunction.prototype;
        var instancePrototypeOk =
            Object.getPrototypeOf(fn.prototype) === aGeneratorPrototype;
        var gen = fn();
        var instanceOk = gen instanceof realmA.Object;
        gen.next();
        functionPrototypeOk && instancePrototypeOk && instanceOk && realmA.calls === 1;
        "#,
    )?;
    assert_eq!(result, Value::Bool(true));
    Ok(())
}

#[test]
fn foreign_callable_allocates_typed_errors_in_its_origin_realm() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation("__createRealm", HostOperation::CreateRealm)?;
    let result = context.eval(
        r#"
        var other = __createRealm();
        var C = other.eval("(class {})");
        var classError;
        try { C(); } catch (error) { classError = error; }
        var otherToString = other.String.prototype.toString;
        var methodError;
        try { otherToString.call(true); } catch (error) { methodError = error; }
        classError.constructor === other.TypeError &&
            methodError.constructor === other.TypeError;
        "#,
    )?;
    assert_eq!(result, Value::Bool(true));
    Ok(())
}

#[test]
fn array_species_ignores_a_foreign_realms_intrinsic_array_constructor() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation("__createRealm", HostOperation::CreateRealm)?;
    let result = context.eval(
        r"
        var other = __createRealm();
        var array = [];
        var callCount = 0;
        var OArray = other.Array;
        var speciesDesc = { get: function () { callCount += 1; } };
        array.constructor = OArray;
        Object.defineProperty(OArray, Symbol.species, speciesDesc);
        var result = array.map(function () {});
        Object.getPrototypeOf(result) === Array.prototype &&
            other.Array.prototype !== Array.prototype &&
            other.Object.prototype !== Object.prototype &&
            callCount === 0;
        ",
    )?;
    assert_eq!(result, Value::Bool(true));
    Ok(())
}

#[test]
fn throw_type_error_is_shared_within_and_distinct_between_realms() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.register_host_operation("__createRealm", HostOperation::CreateRealm)?;
    let result = context.eval(
        r#"
        function thrower(global) {
            return global.Object.getOwnPropertyDescriptor(
                new global.Function('"use strict"; return arguments;')(),
                "callee"
            ).get;
        }
        var other = __createRealm();
        var local = thrower(globalThis);
        var foreign = thrower(other);
        local === thrower(globalThis) && foreign === thrower(other) && local !== foreign;
        "#,
    )?;
    assert_eq!(result, Value::Bool(true));
    Ok(())
}
