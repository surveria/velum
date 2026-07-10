use rs_quickjs::{Engine, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn recognizes_callable_proxies_across_generic_consumers() -> TestResult {
    eval_is_42(
        r#"
        function add(left, right) { return left + right; }
        let applyCount = 0;
        let proxy = new Proxy(add, {
            apply: function (target, thisArg, args) {
                applyCount += 1;
                return Reflect.apply(target, thisArg, args);
            }
        });
        let callback = new Proxy(function (value) { return value * 2; }, {});
        let comparator = new Proxy(function (left, right) { return left - right; }, {});
        let mapped = [10, 11].map(callback);
        let sorted = [3, 1, 2].sort(comparator);
        let bound = Function.prototype.bind.call(proxy, null, 40);
        let reviver = new Proxy(function (key, value) {
            return key === "value" ? value + 2 : value;
        }, {});
        let revived = JSON.parse('{"value":40}', reviver);
        let replacer = new Proxy(function (key, value) { return value; }, {});
        let replaced = JSON.stringify({ value: 42 }, replacer);
        let revocable = Proxy.revocable(function () { return 1; }, {});
        let revoked = revocable.proxy;
        revocable.revoke();
        let revokedRejected = false;
        try {
            revoked();
        } catch (error) {
            revokedRejected = error instanceof TypeError;
        }

        let nonCallableTrapCount = 0;
        let nonCallable = new Proxy({}, {
            apply: function () {
                nonCallableTrapCount += 1;
                return 0;
            }
        });
        let rejected = false;
        try {
            Reflect.apply(nonCallable, null, []);
        } catch (error) {
            rejected = error instanceof TypeError;
        }

        Reflect.apply(proxy, null, [20, 22]) === 42 &&
            Function.prototype.call.call(proxy, null, 20, 22) === 42 &&
            bound(2) === 42 &&
            applyCount === 3 &&
            mapped.join(",") === "20,22" &&
            sorted.join(",") === "1,2,3" &&
            revived.value === 42 &&
            replaced === '{"value":42}' &&
            Object.prototype.toString.call(proxy) === "[object Function]" &&
            typeof revoked === "function" &&
            revokedRejected === true &&
            rejected === true &&
            nonCallableTrapCount === 0 ? 42 : 0
        "#,
    )
}

#[test]
fn preserves_proxy_and_reflect_new_target() -> TestResult {
    eval_is_42(
        r#"
        function Base(value) { this.value = value; }
        Base.prototype.base = true;
        function Alternate() {}
        Alternate.prototype = { marker: 35 };

        let direct = Reflect.construct(Base, [7], Alternate);
        let seen = [];
        let proxy = new Proxy(Base, {
            construct: function (target, args, newTarget) {
                seen.push(newTarget);
                return Reflect.construct(target, args, newTarget);
            }
        });
        let proxied = Reflect.construct(proxy, [7], Alternate);
        let nested = new Proxy(proxy, {});
        let nestedValue = new nested(42);

        let constructTrapCount = 0;
        let callableOnly = new Proxy(() => 1, {
            construct: function () {
                constructTrapCount += 1;
                return {};
            }
        });
        let nonConstructor = (() => {}).bind(null);
        let rejected = false;
        try {
            Reflect.construct(nonConstructor, []);
        } catch (error) {
            rejected = error instanceof TypeError;
        }

        Object.getPrototypeOf(direct) === Alternate.prototype &&
            direct.marker + direct.value === 42 &&
            seen[0] === Alternate &&
            seen[1] === nested &&
            Object.getPrototypeOf(proxied) === Alternate.prototype &&
            proxied.marker + proxied.value === 42 &&
            Object.getPrototypeOf(nestedValue) === Base.prototype &&
            nestedValue.value === 42 &&
            typeof callableOnly === "function" &&
            constructTrapCount === 0 &&
            rejected === true ? 42 : 0
        "#,
    )
}

#[test]
fn bound_functions_inherit_constructor_capability() -> TestResult {
    eval_is_42(
        r#"
        function Pair(left, right) { this.total = left + right; }
        Pair.prototype.kind = "pair";
        function Alternate() {}
        Alternate.prototype = { marker: 10 };

        let Bound = Pair.bind({ total: -1 }, 10);
        let direct = new Bound(32);
        let reflected = Reflect.construct(Bound, [32], Alternate);
        let proxiedTarget = new Proxy(Pair, {});
        let ProxyBound = Function.prototype.bind.call(proxiedTarget, null, 20);
        let proxied = new ProxyBound(22);
        let BoundArray = Array.bind(null, 3);
        let nativeBound = new BoundArray();

        direct.total === 42 &&
            direct.kind === "pair" &&
            direct instanceof Pair &&
            reflected.total === 42 &&
            reflected.marker === 10 &&
            Object.getPrototypeOf(reflected) === Alternate.prototype &&
            proxied.total === 42 &&
            proxied.kind === "pair" &&
            nativeBound.length === 3 ? 42 : 0
        "#,
    )
}

#[test]
fn keeps_host_function_proxy_callable_but_not_constructable() -> TestResult {
    let engine = Engine::new();
    let mut vm = engine.create_vm();
    vm.context()
        .register_host_function_typed("hostAnswer", |_call| Ok(42.0))?;
    let value = vm.context().eval(
        r"
        let proxy = new Proxy(hostAnswer, {});
        let constructRejected = false;
        try {
            Reflect.construct(proxy, []);
        } catch (error) {
            constructRejected = error instanceof TypeError;
        }
        Reflect.apply(proxy, null, []) === 42 &&
            Function.prototype.call.call(proxy, null) === 42 &&
            constructRejected === true ? 42 : 0
        ",
    )?;
    ensure_value(&value, &Value::Number(42.0))
}

fn eval_is_42(source: &str) -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(source)?;
    ensure_value(&value, &Value::Number(42.0))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
