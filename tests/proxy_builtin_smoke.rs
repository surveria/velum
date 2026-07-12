use rs_quickjs::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

const PROXY_BUILTIN_SCRIPT: &str = r#"
        var target = { a: 1, b: 2 };
        var log = [];
        var proxy = new Proxy(target, {
            get: function (t, key) { log.push("get:" + key); return key === "a" ? 42 : t[key]; },
            set: function (t, key, value) { log.push("set:" + key); t[key] = value; return true; },
            has: function (t, key) { log.push("has:" + key); return key in t; },
            deleteProperty: function (t, key) { log.push("del:" + key); delete t[key]; return true; },
            ownKeys: function (t) { return Object.getOwnPropertyNames(t); },
            getOwnPropertyDescriptor: function (t, key) { return Object.getOwnPropertyDescriptor(t, key); }
        });

        var getA = proxy.a;
        proxy.c = 3;
        var hasA = "a" in proxy;
        var hasZ = "z" in proxy;
        var deleted = delete proxy.b;
        var names = Object.getOwnPropertyNames(proxy).join(",");

        function add(x, y) { return x + y; }
        var applied = new Proxy(add, {
            apply: function (t, thisArg, argsList) { return t.apply(thisArg, argsList) * 10; }
        });
        function Point(x) { this.x = x; }
        var constructed = new Proxy(Point, {
            construct: function (t, argsList) { return new t(argsList[0] * 2); }
        });

        var revocable = Proxy.revocable({ v: 5 }, { get: function () { return 99; } });
        var beforeRevoke = revocable.proxy.v;
        revocable.revoke();
        var afterRevokeThrew = false;
        try { revocable.proxy.v; } catch (error) { afterRevokeThrew = error instanceof TypeError; }

        print(typeof Proxy, Proxy.length, Proxy.name);
        print(getA, target.c, hasA, hasZ, deleted, names);
        print(applied(2, 3), new constructed(7).x);
        print(beforeRevoke, afterRevokeThrew, log.join(","));

        typeof Proxy === "function" &&
            Proxy.length === 2 &&
            getA === 42 &&
            target.c === 3 &&
            hasA === true &&
            hasZ === false &&
            deleted === true &&
            names === "a,c" &&
            applied(2, 3) === 50 &&
            new constructed(7).x === 14 &&
            beforeRevoke === 99 &&
            afterRevokeThrew === true ? 42 : 0
"#;

const PROXY_TEST262_FIXTURES: &[&str] = &[
    "tests/corpora/test262/active/built-ins/Proxy/constructor.js",
    "tests/corpora/test262/active/built-ins/Proxy/property_traps.js",
    "tests/corpora/test262/active/built-ins/Proxy/reflection_traps.js",
    "tests/corpora/test262/active/built-ins/Proxy/prototype_extensibility_traps.js",
    "tests/corpora/test262/active/built-ins/Proxy/callable.js",
];

#[test]
fn exposes_proxy_traps_through_public_api() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let value = context.eval(PROXY_BUILTIN_SCRIPT)?;

    ensure_value(&value, &Value::Number(42.0))?;
    ensure_output(
        context.output(),
        &[
            "function 2 Proxy",
            "42 3 true false true a,c",
            "50 14",
            "99 true get:a,set:c,has:a,has:z,del:b",
        ],
    )
}

#[test]
fn preserves_proxy_dispatch_across_fallbacks_and_prototype_chains() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
            var outer;
            var inner = new Proxy({}, {
                get: function (target, key, receiver) {
                    return key === "receiver" && receiver === outer ? 7 : undefined;
                }
            });
            outer = new Proxy(inner, {});

            var child;
            var prototype = new Proxy({}, {
                get: function (target, key, receiver) {
                    return key === "inherited" && receiver === child ? 5 : undefined;
                },
                has: function (target, key) { return key === "virtual"; }
            });
            child = Object.create(prototype);

            outer.receiver + child.inherited + ("virtual" in child ? 30 : 0)
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

#[test]
fn enforces_proxy_internal_method_invariants() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
            function throwsTypeError(operation) {
                try { operation(); } catch (error) { return error instanceof TypeError ? 1 : -100; }
                return 0;
            }

            var frozen = {};
            Object.defineProperty(frozen, "value", {
                value: 1, writable: false, configurable: false
            });
            var score = 0;
            score += throwsTypeError(function () {
                return new Proxy(frozen, { get: function () { return 2; } }).value;
            });
            score += throwsTypeError(function () {
                new Proxy(frozen, { set: function () { return true; } }).value = 2;
            });
            score += throwsTypeError(function () {
                return "value" in new Proxy(frozen, { has: function () { return false; } });
            });
            score += throwsTypeError(function () {
                delete new Proxy(frozen, { deleteProperty: function () { return true; } }).value;
            });

            var fixedPrototype = Object.preventExtensions({});
            score += throwsTypeError(function () {
                Object.getPrototypeOf(new Proxy(fixedPrototype, {
                    getPrototypeOf: function () { return {}; }
                }));
            });
            score += throwsTypeError(function () {
                Object.setPrototypeOf(new Proxy(fixedPrototype, {
                    setPrototypeOf: function () { return true; }
                }), {});
            });
            score += throwsTypeError(function () {
                Object.isExtensible(new Proxy({}, { isExtensible: function () { return false; } }));
            });
            score += throwsTypeError(function () {
                Object.preventExtensions(new Proxy({}, {
                    preventExtensions: function () { return true; }
                }));
            });

            var configurable = {};
            Object.defineProperty(configurable, "value", { configurable: true });
            score += throwsTypeError(function () {
                Object.defineProperty(new Proxy(configurable, {
                    defineProperty: function () { return true; }
                }), "value", { configurable: false });
            });
            score += throwsTypeError(function () {
                Object.getOwnPropertyDescriptor(new Proxy(Object.preventExtensions({}), {
                    getOwnPropertyDescriptor: function () {
                        return { value: 1, configurable: true };
                    }
                }), "value");
            });

            score === 10 ? 42 : score
        "#,
    )?;

    ensure_value(&value, &Value::Number(42.0))
}

// The active Test262 fixtures are executed by the runner with a raw
// `context.eval(source)` (no Test262 harness) and must evaluate to 42 while
// producing no output. Mirror that here so the fixtures stay runner-compatible.
#[test]
fn proxy_test262_fixtures_evaluate_to_42_without_output() -> TestResult {
    for path in PROXY_TEST262_FIXTURES {
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
