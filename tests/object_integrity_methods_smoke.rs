use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_object_extensibility_and_integrity_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { a: 1 };
        let before = Object.isExtensible(object);
        let prevented = Object.preventExtensions(object);
        object.b = 2;
        let defineRejected = false;
        try {
            Object.defineProperty(object, "c", { value: 3 });
        } catch (error) {
            defineRejected = error instanceof TypeError;
        }

        let protoRejected = false;
        try {
            Object.setPrototypeOf(object, { p: 1 });
        } catch (error) {
            protoRejected = error instanceof TypeError;
        }

        Object.defineProperty(object, "a", {
            value: 7,
            enumerable: true,
            writable: true,
            configurable: true
        });
        let descriptor = Object.getOwnPropertyDescriptor(object, "a");

        print(
            Object.preventExtensions.length,
            Object.isExtensible.length,
            Object.seal.length,
            Object.freeze.length,
            Object.isSealed.length,
            Object.isFrozen.length
        );
        print(
            before,
            prevented === object,
            Object.isExtensible(object),
            object.b,
            defineRejected,
            protoRejected
        );
        print(descriptor.value, descriptor.writable, descriptor.configurable);

        before === true &&
            prevented === object &&
            Object.isExtensible(object) === false &&
            object.b === undefined &&
            defineRejected &&
            protoRejected &&
            descriptor.value === 7 &&
            descriptor.writable === true &&
            descriptor.configurable === true ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "1 1 1 1 1 1",
            "true true false undefined true true",
            "7 true true",
        ],
    )
}

#[test]
fn supports_seal_and_freeze_descriptor_transitions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let sealed = { a: 1 };
        Object.defineProperty(sealed, "hidden", { value: 2 });
        let sealedReturned = Object.seal(sealed);
        sealed.a = 3;
        let deleteSealed = delete sealed.a;
        let sealedDescriptor = Object.getOwnPropertyDescriptor(sealed, "a");
        let hiddenDescriptor = Object.getOwnPropertyDescriptor(sealed, "hidden");

        let frozen = { a: 1 };
        let frozenReturned = Object.freeze(frozen);
        frozen.a = 5;
        let deleteFrozen = delete frozen.a;
        let frozenDescriptor = Object.getOwnPropertyDescriptor(frozen, "a");

        let array = [1, 2];
        Object.freeze(array);
        array[0] = 9;
        array[2] = 3;
        let element = Object.getOwnPropertyDescriptor(array, "0");
        let length = Object.getOwnPropertyDescriptor(array, "length");

        print(
            sealedReturned === sealed,
            Object.isSealed(sealed),
            Object.isFrozen(sealed),
            sealed.a,
            deleteSealed
        );
        print(sealedDescriptor.writable, sealedDescriptor.configurable);
        print(hiddenDescriptor.writable, hiddenDescriptor.configurable);
        print(
            frozenReturned === frozen,
            Object.isSealed(frozen),
            Object.isFrozen(frozen),
            frozen.a,
            deleteFrozen
        );
        print(frozenDescriptor.writable, frozenDescriptor.configurable);
        print(array[0], array[2], Object.isFrozen(array), element.writable, length.writable);

        sealedReturned === sealed &&
            Object.isSealed(sealed) === true &&
            Object.isFrozen(sealed) === false &&
            sealed.a === 3 &&
            deleteSealed === false &&
            sealedDescriptor.writable === true &&
            sealedDescriptor.configurable === false &&
            hiddenDescriptor.writable === false &&
            hiddenDescriptor.configurable === false &&
            frozenReturned === frozen &&
            Object.isSealed(frozen) === true &&
            Object.isFrozen(frozen) === true &&
            frozen.a === 1 &&
            deleteFrozen === false &&
            frozenDescriptor.writable === false &&
            frozenDescriptor.configurable === false &&
            array[0] === 1 &&
            array[2] === undefined &&
            Object.isFrozen(array) === true &&
            element.writable === false &&
            length.writable === false ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "true true false 3 false",
            "true false",
            "false false",
            "true true true 1 false",
            "false false",
            "1 undefined true false false",
        ],
    )
}

#[test]
fn treats_primitives_as_already_non_extensible_sealed_and_frozen() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        print(
            Object.preventExtensions(7),
            Object.seal("x"),
            Object.freeze(true),
            Object.preventExtensions(undefined),
            Object.freeze(null)
        );
        print(
            Object.isExtensible(7),
            Object.isExtensible(null),
            Object.isSealed("x"),
            Object.isSealed(undefined),
            Object.isFrozen(true),
            Object.isFrozen(null)
        );

        Object.preventExtensions(7) === 7 &&
            Object.seal("x") === "x" &&
            Object.freeze(true) === true &&
            Object.preventExtensions(undefined) === undefined &&
            Object.freeze(null) === null &&
            Object.isExtensible(7) === false &&
            Object.isExtensible(null) === false &&
            Object.isSealed("x") === true &&
            Object.isSealed(undefined) === true &&
            Object.isFrozen(true) === true &&
            Object.isFrozen(null) === true ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &["7 x true undefined null", "false false true true true true"],
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    let actual: Vec<&str> = actual.iter().map(String::as_str).collect();
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected output {expected:?}, got {actual:?}").into())
}
