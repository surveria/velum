use velum::{Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn supports_async_class_methods_and_super() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        class Base {
            async value(input) {
                return input + 1;
            }
        }
        class Derived extends Base {
            async value(input = 40) {
                return await super.value(input) + 1;
            }
            static async create() {
                return new this();
            }
        }
        Derived.create().then(function(instance) {
            return instance.value();
        }).then(function(value) {
            trace = value + ":" + (Object.getPrototypeOf(Derived) === Base);
        });
        "#,
    )?;
    ensure_value(&context.eval("trace")?, &Value::from("42:true"))
}

#[test]
fn supports_generator_and_async_generator_class_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        class Streams {
            *syncValues() {
                yield 20;
                return 22;
            }
            async *asyncValues() {
                yield await Promise.resolve(40);
                return 42;
            }
            static async *staticValues() {
                yield 41;
                return 43;
            }
        }
        const sync = new Streams().syncValues();
        const asynchronous = new Streams().asyncValues();
        const staticAsynchronous = Streams.staticValues();
        Promise.all([
            asynchronous.next(),
            asynchronous.next(),
            staticAsynchronous.next()
        ]).then(function(results) {
            trace = sync.next().value + ":" + sync.next().value + ":" +
                results[0].value + ":" + results[1].value + ":" +
                results[2].value;
        });
        "#,
    )?;
    ensure_value(&context.eval("trace")?, &Value::from("20:22:40:42:41"))
}

#[test]
fn supports_computed_and_private_async_class_methods() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        let trace = "";
        const key = "computed";
        class Methods {
            async [key]() {
                return await this.#secret();
            }
            async #secret() {
                return 40;
            }
            static async #staticSecret() {
                return 2;
            }
            static async total(instance) {
                return await instance.computed() + await this.#staticSecret();
            }
        }
        Methods.total(new Methods()).then(function(value) {
            trace = value + ":" + Methods.prototype.computed.name;
        });
        "#,
    )?;
    ensure_value(&context.eval("trace")?, &Value::from("42:computed"))
}

#[test]
fn exposes_class_method_kinds_and_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        class Kinds {
            async plain(first, second = 2) {}
            *generator() {}
            async *asynchronousGenerator() {}
        }
        const plain = Kinds.prototype.plain;
        const generator = Kinds.prototype.generator;
        const asynchronousGenerator = Kinds.prototype.asynchronousGenerator;
        const descriptor = Object.getOwnPropertyDescriptor(Kinds.prototype, "plain");
        plain.name === "plain" && plain.length === 1 &&
            Object.getPrototypeOf(plain) === Object.getPrototypeOf(async function() {}) &&
            Object.getPrototypeOf(generator) === Object.getPrototypeOf(function* () {}) &&
            Object.getPrototypeOf(asynchronousGenerator) ===
                Object.getPrototypeOf(async function* () {}) &&
            descriptor.writable && !descriptor.enumerable && descriptor.configurable
        "#,
    )?;
    ensure_value(&value, &Value::Bool(true))
}

#[test]
fn line_terminator_prevents_async_class_method_prefix() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r"
        class Separated {
            async
            value() { return 42; }
        }
        const instance = new Separated();
        instance.async === undefined && instance.value() === 42
        ",
    )?;
    ensure_value(&value, &Value::Bool(true))
}

fn ensure_value(actual: &Value, expected: &Value) -> TestResult {
    if actual == expected {
        return Ok(());
    }
    Err(format!("expected value {expected:?}, got {actual:?}").into())
}
