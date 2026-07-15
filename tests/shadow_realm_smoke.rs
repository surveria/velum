use std::collections::BTreeMap;

use velum::{Error, ModuleLoader, ModuleSource, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn evaluates_in_isolated_realms_and_wraps_callable_values() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        const realm = new ShadowRealm();
        globalThis.marker = 3;
        realm.evaluate("globalThis.marker = 7");
        const multiply = realm.evaluate("x => y => x * y");
        marker === 3 && realm.evaluate("marker") === 7 && multiply(6)(7) === 42;
        "#,
    )?;
    ensure(
        value == Value::Bool(true),
        "ShadowRealm evaluation did not preserve isolation or callable wrapping",
    )
}

#[test]
fn imports_values_through_the_vm_owned_dynamic_loader() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.set_dynamic_module_loader(MapLoader::new([
        ("value.js", "export const answer = 42;"),
        (
            "callable.js",
            "export const multiply = (left, right) => left * right;",
        ),
    ]));
    context.eval(
        r#"
        var importedAnswer;
        var importedProduct;
        const realm = new ShadowRealm();
        realm.importValue("value.js", "answer").then(value => {
            importedAnswer = value;
        });
        realm.importValue("callable.js", "multiply").then(value => {
            importedProduct = value(6, 7);
        });
        "#,
    )?;
    context.run_jobs()?;
    let value = context.eval("importedAnswer === 42 && importedProduct === 42")?;
    ensure(
        value == Value::Bool(true),
        "ShadowRealm importValue did not transfer module exports",
    )
}

#[test]
fn rejects_module_failures_with_caller_realm_type_errors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.set_dynamic_module_loader(MapLoader::new([
        ("value.js", "export const answer = 42;"),
        ("syntax.js", "This is not valid module source."),
        ("throw.js", "throw new Error('child failure');"),
    ]));
    context.eval(
        r#"
        var missingRejected;
        var syntaxRejected;
        var throwRejected;
        const realm = new ShadowRealm();
        realm.importValue("value.js", "missing").then(undefined, error => {
            missingRejected = Object.getPrototypeOf(error) === TypeError.prototype;
        });
        realm.importValue("syntax.js", "answer").then(undefined, error => {
            syntaxRejected = Object.getPrototypeOf(error) === TypeError.prototype;
        });
        realm.importValue("throw.js", "answer").then(undefined, error => {
            throwRejected = Object.getPrototypeOf(error) === TypeError.prototype;
        });
        "#,
    )?;
    context.run_jobs()?;
    let value = context.eval("missingRejected && syntaxRejected && throwRejected")?;
    ensure(
        value == Value::Bool(true),
        "ShadowRealm import failures did not reject with caller-realm TypeError objects",
    )
}

#[derive(Clone)]
struct MapLoader {
    sources: BTreeMap<String, String>,
}

impl MapLoader {
    fn new<const N: usize>(sources: [(&str, &str); N]) -> Self {
        Self {
            sources: sources
                .into_iter()
                .map(|(name, source)| (name.to_owned(), source.to_owned()))
                .collect(),
        }
    }
}

impl ModuleLoader for MapLoader {
    fn load(&mut self, _referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        let source = self
            .sources
            .get(request)
            .cloned()
            .ok_or_else(|| Error::runtime(format!("missing test module '{request}'")))?;
        Ok(ModuleSource::new(request, source))
    }
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.into())
}
