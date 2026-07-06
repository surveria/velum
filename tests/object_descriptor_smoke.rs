use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const OBJECT_DESCRIPTOR_SCRIPT: &str = r#"
let objectKeys = "";
for (let key in Object) {
    objectKeys = objectKeys + key + ";";
}

let object = { a: 1 };
let returned = Object.defineProperty(object, "hidden", { value: 9 });
Object.defineProperty(object, "fixed", {
    value: 7,
    enumerable: true,
    writable: false,
    configurable: false
});
Object.defineProperty(object, "open", {
    value: 3,
    enumerable: true,
    writable: true,
    configurable: true
});

let fixedDescriptor = Object.getOwnPropertyDescriptor(object, "fixed");
let hiddenDescriptor = Object.getOwnPropertyDescriptor(object, "hidden");
let missingDescriptor = Object.getOwnPropertyDescriptor(object, "missing");
object.fixed = 8;
let deleteFixed = delete object.fixed;
let deleteHidden = delete object.hidden;
let deleteOpen = delete object.open;
let keys = Object.keys(object);
let child = { __proto__: object, own: 5 };

print(
    typeof Object.getOwnPropertyDescriptor,
    Object.getOwnPropertyDescriptor.name,
    Object.getOwnPropertyDescriptor.length,
    typeof Object.defineProperty,
    Object.defineProperty.name,
    Object.defineProperty.length,
    typeof Object.keys,
    Object.keys.name,
    Object.keys.length,
    typeof Object.hasOwn,
    Object.hasOwn.name,
    Object.hasOwn.length
);
print(
    fixedDescriptor.value,
    fixedDescriptor.enumerable,
    fixedDescriptor.writable,
    fixedDescriptor.configurable,
    object.fixed,
    deleteFixed
);
print(
    hiddenDescriptor.value,
    hiddenDescriptor.enumerable,
    hiddenDescriptor.writable,
    hiddenDescriptor.configurable,
    deleteHidden,
    missingDescriptor
);
print(
    keys.length,
    keys[0],
    keys[1],
    Object.hasOwn(object, "fixed"),
    Object.hasOwn(child, "fixed"),
    "fixed" in child,
    Object.hasOwn(child, "own"),
    deleteOpen,
    "keys:" + objectKeys
);

returned === object &&
    typeof Object.getOwnPropertyDescriptor === "function" &&
    Object.getOwnPropertyDescriptor.name === "getOwnPropertyDescriptor" &&
    Object.getOwnPropertyDescriptor.length === 2 &&
    typeof Object.defineProperty === "function" &&
    Object.defineProperty.name === "defineProperty" &&
    Object.defineProperty.length === 3 &&
    typeof Object.keys === "function" &&
    Object.keys.name === "keys" &&
    Object.keys.length === 1 &&
    typeof Object.hasOwn === "function" &&
    Object.hasOwn.name === "hasOwn" &&
    Object.hasOwn.length === 2 &&
    fixedDescriptor.value === 7 &&
    fixedDescriptor.enumerable === true &&
    fixedDescriptor.writable === false &&
    fixedDescriptor.configurable === false &&
    hiddenDescriptor.value === 9 &&
    hiddenDescriptor.enumerable === false &&
    hiddenDescriptor.writable === false &&
    hiddenDescriptor.configurable === false &&
    missingDescriptor === undefined &&
    object.fixed === 7 &&
    deleteFixed === false &&
    deleteHidden === false &&
    deleteOpen === true &&
    keys.length === 2 &&
    keys[0] === "a" &&
    keys[1] === "fixed" &&
    Object.hasOwn(object, "fixed") === true &&
    Object.hasOwn(child, "fixed") === false &&
    "fixed" in child &&
    Object.hasOwn(child, "own") === true &&
    objectKeys === "" ? 42 : 0
"#;

#[test]
fn supports_data_property_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(OBJECT_DESCRIPTOR_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function getOwnPropertyDescriptor 2 function defineProperty 3 function keys 1 function hasOwn 2",
            "7 true false false 7 false",
            "9 false false false false undefined",
            "2 a fixed true false true true true keys:",
        ],
    )
}

#[test]
fn reuses_descriptor_property_keys_and_shape_layout() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r#"
        let object = {};
        let first = undefined;
        let second = undefined;
        let third = undefined;
        Object.defineProperty(object, "slot", {
            value: 7,
            enumerable: true,
            writable: false,
            configurable: false
        });
        first = Object.getOwnPropertyDescriptor(object, "slot");
        first.value === 7 &&
            first.enumerable === true &&
            first.writable === false &&
            first.configurable === false ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))?;
    let descriptor_atoms = vm.resource_usage().atom_count;
    let descriptor_shapes = vm.resource_usage().shape_count;

    let value = vm.context().eval(
        r#"
        first = Object.getOwnPropertyDescriptor(object, "slot");
        second = Object.getOwnPropertyDescriptor(object, "slot");
        third = Object.getOwnPropertyDescriptor(object, "slot");
        first.value === 7 &&
            second.value === 7 &&
            third.value === 7 &&
            first.enumerable === true &&
            second.writable === false &&
            third.configurable === false ? 42 : 0
        "#,
    )?;
    ensure_value(&value, &Value::Number(42.0))?;
    ensure_usize(vm.resource_usage().atom_count, descriptor_atoms)?;
    ensure_usize(vm.resource_usage().shape_count, descriptor_shapes)
}

#[test]
fn preserves_descriptor_slots_after_delete_and_reinsert() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { first: 1, second: 2, third: 3 };
        let deleteFirst = delete object.first;
        let thirdBefore = Object.getOwnPropertyDescriptor(object, "third");

        object.first = 10;
        Object.defineProperty(object, "second", {
            value: 20,
            enumerable: true,
            writable: true,
            configurable: true
        });
        let deleteSecond = delete object.second;
        object.second = 22;

        let thirdAfter = Object.getOwnPropertyDescriptor(object, "third");
        let keys = Object.keys(object);
        let seen = "";
        for (let key in object) {
            seen = seen + key + ":" + object[key] + ";";
        }

        print(seen);
        print(keys.length, keys[0], keys[1], keys[2]);
        print(deleteFirst, deleteSecond, thirdBefore.value, thirdAfter.value);

        seen === "third:3;first:10;second:22;" &&
            keys.length === 3 &&
            keys[0] === "third" &&
            keys[1] === "first" &&
            keys[2] === "second" &&
            deleteFirst === true &&
            deleteSecond === true &&
            ("first" in object) &&
            ("second" in object) &&
            ("third" in object) &&
            thirdBefore.value === 3 &&
            thirdAfter.value === 3 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "third:3;first:10;second:22;",
            "3 third first second",
            "true true 3 3",
        ],
    )
}

#[test]
fn tracks_descriptor_attributes_in_shape_layouts() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();

    let value = vm.context().eval(
        r#"
        let fixedDescriptor = {
            value: 1,
            enumerable: true,
            writable: false,
            configurable: false
        };
        let openDescriptor = {
            value: 3,
            enumerable: true,
            writable: true,
            configurable: true
        };
        let closeDescriptor = {
            writable: false,
            configurable: false
        };
        let warmup = {};
        Object.defineProperty(warmup, "slot", fixedDescriptor);
        warmup.slot
        "#,
    )?;
    ensure_value(&value, &Value::Number(1.0))?;
    let fixed_shapes = vm.resource_usage().shape_count;
    ensure_positive(fixed_shapes, "fixed descriptor shapes")?;

    let value = vm.context().eval(
        r#"
        fixedDescriptor.value = 2;
        let second = {};
        Object.defineProperty(second, "slot", fixedDescriptor);
        second.slot
        "#,
    )?;
    ensure_value(&value, &Value::Number(2.0))?;
    ensure_usize(vm.resource_usage().shape_count, fixed_shapes)?;

    let value = vm.context().eval(
        r#"
        let third = {};
        Object.defineProperty(third, "slot", openDescriptor);
        third.slot
        "#,
    )?;
    ensure_value(&value, &Value::Number(3.0))?;
    let open_shapes = vm.resource_usage().shape_count;
    ensure_greater_than(open_shapes, fixed_shapes, "open descriptor shapes")?;

    let value = vm.context().eval(
        r#"
        Object.defineProperty(third, "slot", closeDescriptor);
        third.slot = 4;
        third.slot
        "#,
    )?;
    ensure_value(&value, &Value::Number(3.0))?;
    ensure_usize(vm.resource_usage().shape_count, open_shapes)
}

#[test]
fn preserves_out_of_order_property_lookup_with_vector_index() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(
        r#"
        let object = { zeta: 1, alpha: 2, middle: 3 };
        object.zeta = object.zeta + object.alpha;
        Object.defineProperty(object, "middle", {
            value: object.middle + object.zeta,
            enumerable: true,
            writable: true,
            configurable: true
        });
        let alphaDescriptor = Object.getOwnPropertyDescriptor(object, "alpha");
        let deleteAlpha = delete object.alpha;
        object.alpha = 20;
        let keys = Object.keys(object);
        let seen = "";
        for (let key in object) {
            seen = seen + key + ":" + object[key] + ";";
        }

        print(seen);
        print(keys.length, keys[0], keys[1], keys[2]);
        print(alphaDescriptor.value, deleteAlpha, "middle" in object);

        seen === "zeta:3;middle:6;alpha:20;" &&
            keys.length === 3 &&
            keys[0] === "zeta" &&
            keys[1] === "middle" &&
            keys[2] === "alpha" &&
            alphaDescriptor.value === 2 &&
            deleteAlpha === true &&
            ("zeta" in object) &&
            ("alpha" in object) &&
            ("middle" in object) &&
            object.zeta === 3 &&
            object.alpha === 20 &&
            object.middle === 6 ? 42 : 0
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "zeta:3;middle:6;alpha:20;",
            "3 zeta middle alpha",
            "2 true true",
        ],
    )
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }

    Err(format!("expected value {expected:?}, got {actual:?}").into())
}

fn ensure_output(actual: &[String], expected: &[&str]) -> TestResult {
    if actual
        .iter()
        .map(String::as_str)
        .eq(expected.iter().copied())
    {
        return Ok(());
    }

    Err(format!("expected output {expected:?}, got {actual:?}").into())
}

fn ensure_positive(actual: usize, label: &str) -> TestResult {
    if actual > 0 {
        return Ok(());
    }
    Err(format!("expected positive {label}, got {actual}").into())
}

fn ensure_greater_than(actual: usize, minimum: usize, label: &str) -> TestResult {
    if actual > minimum {
        return Ok(());
    }
    Err(format!("expected {label} greater than {minimum}, got {actual}").into())
}

fn ensure_usize(actual: usize, expected: usize) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected {expected}, got {actual}").into())
}
