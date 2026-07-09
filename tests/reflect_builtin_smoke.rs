use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const REFLECT_BUILTIN_SCRIPT: &str = r#"
        let target = { alpha: 1 };
        Object.defineProperty(target, "beta", {
            value: 2,
            enumerable: false,
            writable: true,
            configurable: true
        });

        let getAlpha = Reflect.get(target, "alpha");
        let hasBeta = Reflect.has(target, "beta");
        let hasMissing = Reflect.has(target, "missing");
        let setGamma = Reflect.set(target, "gamma", 3);
        let deletedAlpha = Reflect.deleteProperty(target, "alpha");
        let keys = Reflect.ownKeys(target).join(",");
        let symbol = Symbol("reflect-key");
        let registered = Symbol.for("shared-reflect-key");
        target[symbol] = 5;
        target[registered] = 6;
        let reflectKeys = Reflect.ownKeys(target);
        let registeredAgain = Symbol.for("shared-reflect-key");
        let registryKey = Symbol.keyFor(registered);

        let proto = { marker: 7 };
        let child = Object.create(proto);
        let protoMatch = Reflect.getPrototypeOf(child) === proto;
        let swapped = Reflect.setPrototypeOf(child, null);
        let clearedProto = Reflect.getPrototypeOf(child) === null;

        let extensible = {};
        let wasExtensible = Reflect.isExtensible(extensible);
        let prevented = Reflect.preventExtensions(extensible);
        let nowSealed = Reflect.isExtensible(extensible) === false;
        let locked = {};
        Object.preventExtensions(locked);
        let rejectedPrototype = Reflect.setPrototypeOf(locked, {});

        function Point(x, y) {
            this.x = x;
            this.y = y;
        }
        let point = Reflect.construct(Point, [3, 4]);
        let constructedOk = point.x === 3 && point.y === 4 && point instanceof Point;

        function sum(a, b, c) {
            return a + b + c;
        }
        let applied = Reflect.apply(sum, null, [10, 20, 12]);

        let toStringTag = Object.prototype.toString.call(Reflect);

        print(
            typeof Reflect,
            Reflect.get.name,
            Reflect.get.length,
            Reflect.apply.length,
            toStringTag
        );
        print(getAlpha, hasBeta, hasMissing, setGamma, deletedAlpha, keys);
        print(protoMatch, swapped, clearedProto, wasExtensible, prevented, nowSealed);
        print(constructedOk, applied);

        typeof Reflect === "object" &&
            toStringTag === "[object Reflect]" &&
            Reflect.get.name === "get" &&
            Reflect.get.length === 2 &&
            Reflect.apply.length === 3 &&
            getAlpha === 1 &&
            hasBeta === true &&
            hasMissing === false &&
            setGamma === true &&
            target.gamma === 3 &&
            deletedAlpha === true &&
            keys === "beta,gamma" &&
            reflectKeys.length === 4 &&
            reflectKeys[0] === "beta" &&
            reflectKeys[1] === "gamma" &&
            reflectKeys[2] === symbol &&
            reflectKeys[3] === registered &&
            registeredAgain === registered &&
            registryKey === "shared-reflect-key" &&
            protoMatch === true &&
            swapped === true &&
            clearedProto === true &&
            wasExtensible === true &&
            prevented === true &&
            nowSealed === true &&
            rejectedPrototype === false &&
            constructedOk === true &&
            applied === 42 ? 42 : 0
"#;

const REFLECT_ERROR_SCRIPT: &str = r#"
        function throwsType(thunk) {
            try {
                thunk();
                return false;
            } catch (error) {
                return error instanceof TypeError;
            }
        }

        let getOnPrimitive = throwsType(function () { return Reflect.get(1, "x"); });
        let hasOnNull = throwsType(function () { return Reflect.has(null, "x"); });
        let ownKeysUndefined = throwsType(function () { return Reflect.ownKeys(undefined); });
        let applyNonCallable = throwsType(function () { return Reflect.apply({}, null, []); });
        let constructNonCtor = throwsType(function () { return Reflect.construct({}, []); });
        let proxyHasThrow = false;
        try {
            Reflect.has(new Proxy({}, {
                has: function () {
                    throw new TypeError("proxy has trap");
                }
            }), "x");
        } catch (error) {
            proxyHasThrow = error instanceof TypeError;
        }

        getOnPrimitive &&
            hasOnNull &&
            ownKeysUndefined &&
            applyNonCallable &&
            constructNonCtor &&
            proxyHasThrow ? 42 : 0
"#;

#[test]
fn exposes_reflect_namespace_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(REFLECT_BUILTIN_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "object get 2 3 [object Reflect]",
            "1 true false true true beta,gamma",
            "true true true true true true",
            "true 42",
        ],
    )
}

#[test]
fn reflect_rejects_invalid_targets_with_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(REFLECT_ERROR_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))
}

// The active Test262 fixtures are executed by the runner with a raw
// `context.eval(source)` (no Test262 harness) and must evaluate to 42 while
// producing no output. Mirror that execution here so the fixtures stay
// runner-compatible even without running the full corpus.
const REFLECT_TEST262_FIXTURES: &[&str] = &[
    "tests/corpora/test262/active/built-ins/Reflect/metadata.js",
    "tests/corpora/test262/active/built-ins/Reflect/property_ops.js",
    "tests/corpora/test262/active/built-ins/Reflect/prototype_extensibility.js",
    "tests/corpora/test262/active/built-ins/Reflect/apply_construct.js",
];

#[test]
fn reflect_test262_fixtures_evaluate_to_42_without_output() -> TestResult {
    for path in REFLECT_TEST262_FIXTURES {
        let source = std::fs::read_to_string(path)
            .map_err(|error| format!("failed to read fixture '{path}': {error}"))?;

        let runtime = Runtime::new();
        let mut context = runtime.context();
        let value = context
            .eval(&source)
            .map_err(|error| format!("fixture '{path}' failed to evaluate: {error}"))?;

        ensure_value(&value, &Value::Number(42.0))
            .map_err(|error| format!("fixture '{path}': {error}"))?;
        ensure_output(context.output(), &[])
            .map_err(|error| format!("fixture '{path}': {error}"))?;
    }

    Ok(())
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
