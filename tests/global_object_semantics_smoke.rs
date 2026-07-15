use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn global_identifier_access_uses_semantic_prototype_dispatch() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const global = this;
        const prototype = Object.getPrototypeOf(global);
        let gets = 0;
        let sets = 0;
        Object.setPrototypeOf(global, new Proxy(prototype, {
            has(target, key) {
                return key === "bareword" || Reflect.has(target, key);
            },
            get(target, key, receiver) {
                gets++;
                if (receiver !== global) throw new Error("wrong get receiver");
                return Reflect.get(target, key, receiver);
            },
            set(target, key, next, receiver) {
                sets++;
                if (receiver !== global) throw new Error("wrong set receiver");
                return Reflect.set(target, key, next, receiver);
            }
        }));
        const before = bareword;
        bareword = 12;
        before === undefined && gets === 1 && sets === 1 && global.bareword === 12 ? 42 : 0
        "#,
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("unexpected global Proxy dispatch result: {value:?}").into())
}

#[test]
fn sloppy_global_creation_respects_non_extensibility() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const functionObject = function() {};
        Object.preventExtensions(functionObject);
        functionObject.extra = 1;
        let strictRejected = false;
        try {
            (function() { "use strict"; functionObject.other = 2; }());
        } catch (error) {
            strictRejected = error instanceof TypeError;
        }
        Object.preventExtensions(this);
        absentGlobal = 12;
        !("extra" in functionObject) && strictRejected &&
            typeof absentGlobal === "undefined" && !("absentGlobal" in this) ? 42 : 0
        "#,
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("unexpected non-extensible global result: {value:?}").into())
}

#[test]
fn var_initializer_resolves_its_with_binding_before_rhs_effects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        var object = { target: 1 };
        with (object) {
            var target = delete object.target;
        }
        object.target === true && target === undefined ? 42 : 0
        "#,
    )?;
    if value == Value::Number(42.0) {
        return Ok(());
    }
    Err(format!("unexpected var binding-resolution result: {value:?}").into())
}
