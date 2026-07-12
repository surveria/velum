use rs_quickjs::{OptimizationMode, OwnedValue, Vm, VmConfig};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const GLOBAL_MUTATION_SOURCE: &str = r#"
    var declared = 1;
    globalThis.parseInt = function (value) { return value + 40; };
    let directOverride = parseInt(2);

    Reflect.set(globalThis, "Date", function () { return 42; });
    let reflectOverride = Date();

    Reflect.defineProperty(globalThis, "declared", {
        value: 39,
        writable: true
    });
    declared += 3;
    let declaredOverride = declared === 42 && globalThis.declared === 42;

    Object.defineProperty(globalThis, "parseFloat", {
        get: function () { return function () { return 42; }; },
        configurable: true
    });
    let accessorOverride = parseFloat("ignored");

    Object.assign(globalThis, { JSON: { answer: 42 } });
    let assignOverride = JSON.answer;

    Object.defineProperty(globalThis, "evalFunction", {
        writable: true,
        enumerable: true,
        configurable: false
    });
    eval("function evalFunction() { return 42; }");
    let evalOverride = evalFunction();

    let strictUnresolved = false;
    try {
        (function () {
            "use strict";
            strictCreated = (globalThis.strictCreated = 5);
        })();
    } catch (error) {
        strictUnresolved = error.name === "ReferenceError";
    }
    delete globalThis.strictCreated;

    let deleted = delete globalThis.parseInt;
    let deletionVisible =
        deleted &&
        typeof parseInt === "undefined" &&
        !Object.prototype.hasOwnProperty.call(globalThis, "parseInt");
    parseInt = function () { return 42; };
    let recreated = parseInt();

    directOverride === 42 &&
        reflectOverride === 42 &&
        declaredOverride &&
        accessorOverride === 42 &&
        assignOverride === 42 &&
        evalOverride === 42 &&
        strictUnresolved &&
        deletionVisible &&
        recreated === 42 ? 42 : 0
"#;

#[test]
fn global_object_mutations_control_bare_identifier_semantics() -> TestResult {
    for mode in [OptimizationMode::Enabled, OptimizationMode::Disabled] {
        let config = VmConfig::default().with_optimization_mode(mode);
        let mut vm = Vm::with_config(config);
        let script = vm.compile(GLOBAL_MUTATION_SOURCE)?;
        let value = vm.eval_compiled_owned(&script)?;
        ensure_equal(&value, &OwnedValue::Number(42.0))?;
    }
    Ok(())
}

fn ensure_equal(actual: &OwnedValue, expected: &OwnedValue) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
