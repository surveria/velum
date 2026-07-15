use std::collections::BTreeMap;

use velum::{Error, ModuleLoader, ModuleSource, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn functions_retain_their_defining_module_import_meta() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([(
        "dependency.js",
        "export const meta = import.meta; export function getMeta() { return import.meta; }",
    )]);
    let value = context.eval_module_named(
        "main.js",
        r#"
            import { meta, getMeta } from "dependency.js";
            (meta !== import.meta ? 1 : 0) + (meta === getMeta() ? 2 : 0)
        "#,
        &mut loader,
    )?;

    ensure(
        value == Value::Number(3.0),
        &format!("function import.meta ownership result was {value:?}"),
    )
}

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
