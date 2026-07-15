use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn static_optional_member_short_circuits_nullish_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        function once(value) { calls += 1; return value; }
        [
          once(null)?.value,
          once(undefined)?.value,
          once({ value: 7 })?.value,
          calls
        ].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("||7|3"))
}

#[test]
fn optional_member_calls_preserve_receivers_and_skip_nullish_arguments() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        function argument() { calls += 1; return 1; }
        const object = {
          value: 41,
          add(value) { return this.value + value; }
        };
        const direct = object?.add(1);
        const spread = object?.add(...[1]);
        const skipped = null?.add(argument());
        [direct, spread, skipped, calls].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("42|42||0"))
}

#[test]
fn optional_calls_preserve_receivers_and_skip_arguments() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        function argument() { calls += 1; return 1; }
        function add(value) { return 41 + value; }
        const object = {
          value: 41,
          add(value) { return this.value + value; }
        };
        const missing = null;
        [
          add?.(1),
          missing?.(argument()),
          object.add?.(1),
          object.add?.(...[1]),
          object.missing?.(argument()),
          calls
        ].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("42||42|42||0"))
}

#[test]
fn optional_computed_and_private_members_short_circuit() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let calls = 0;
        function key() { calls += 1; return "value"; }
        class Box {
          #value = 42;
          read(value) { return value?.#value; }
        }
        const box = new Box();
        [
          null?.[key()],
          null?.[key()].nested(argument),
          ({ value: 42 })?.[key()],
          box.read(null),
          box.read(box),
          calls
        ].join("|")
        "#,
    )?;
    ensure_value(&value, &Value::from("||42||42|1"))
}

#[test]
fn optional_super_calls_preserve_the_derived_receiver() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        class Base {
          method(value) { return this.base + value; }
        }
        class Derived extends Base {
          base = 41;
          call() { return super.method?.(1); }
        }
        new Derived().call()
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn optional_chain_delete_preserves_reference_semantics() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        let keyCalls = 0;
        function key() { keyCalls += 1; return "value"; }
        let object = { value: 42, nested: { value: 7 } };
        let staticDeleted = delete object?.value;
        let computedDeleted = delete object?.nested?.[key()];
        let skipped = delete null?.nested[key()];
        let strictRejected = false;
        try {
          (function() {
            "use strict";
            let fixed = {};
            Object.defineProperty(fixed, "value", { value: 1 });
            delete fixed?.value;
          }());
        } catch (error) {
          strictRejected = error instanceof TypeError;
        }
        staticDeleted && computedDeleted && skipped && strictRejected &&
          object.value === undefined && object.nested.value === undefined &&
          keyCalls === 1 ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))?;

    if context
        .eval("class Box { #value; remove(box) { delete box?.#value; } }")
        .is_err()
    {
        return Ok(());
    }
    Err("expected optional private member deletion to fail during parsing".into())
}

#[test]
fn question_before_decimal_remains_a_conditional_operator() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval("true?.3:0")?;
    ensure_value(&value, &Value::Number(0.3))
}

#[test]
fn optional_chains_cannot_tag_templates() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let result = context.eval("const target = { tag() {} }; target?.tag`value`;");
    if result.is_err() {
        return Ok(());
    }
    Err("expected optional-chain tagged template to fail during parsing".into())
}

#[test]
fn optional_chains_cannot_directly_follow_new_expressions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    for source in [
        "const object = { C: class {} }; new object?.C();",
        "const object = { C: class {} }; new object?.['C']();",
        "class C {} new C?.();",
    ] {
        if context.eval(source).is_ok() {
            return Err(format!("expected optional constructor chain to fail: {source}").into());
        }
    }
    let value = context.eval("class C { constructor() { this.value = 42; } } new C()?.value;")?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected:?}, got {actual:?}").into())
}
