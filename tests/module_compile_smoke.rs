use std::collections::BTreeMap;

use rs_quickjs::{
    Error, ModuleExport, ModuleImportName, ModuleLoader, ModuleSource, Runtime, Value, VmRootKind,
};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn compiles_module_requests_imports_and_exports() -> TestResult {
    let runtime = Runtime::new();
    let module = runtime.compile_module_named(
        "app/main.js",
        r#"
            import primary, { value as localValue } from "./dep.js";
            import * as tools from "./tools.js";
            export { localValue as renamed };
            export * from "./extra.js";
            export * as namespace from "./namespace.js";
            export const answer = primary + tools.offset;
        "#,
    )?;

    ensure(
        module.requests() == ["./dep.js", "./tools.js", "./extra.js", "./namespace.js"],
        "unexpected module requests",
    )?;
    ensure(module.imports().len() == 3, "unexpected import count")?;
    ensure(
        matches!(
        module
            .imports()
            .first()
            .map(rs_quickjs::ModuleImport::import_name),
        Some(ModuleImportName::Name(name)) if name == "default"
        ),
        "default import metadata is missing",
    )?;
    ensure(
        module.exports().iter().any(|entry| {
            matches!(
                entry,
                ModuleExport::Local {
                    export_name,
                    local_name,
                } if export_name == "answer" && local_name == "answer"
            )
        }),
        "local export metadata is missing",
    )?;
    Ok(())
}

#[test]
fn enforces_module_specific_early_errors() -> TestResult {
    let runtime = Runtime::new();
    let duplicate = runtime.compile_module_named(
        "duplicate.js",
        "const first = 1; export { first as value, first as value };",
    );
    ensure(duplicate.is_err(), "duplicate exports must fail")?;

    let await_binding = runtime.compile_module_named("await.js", "let await = 1;");
    ensure(await_binding.is_err(), "await module binding must fail")?;
    Ok(())
}

#[test]
fn keeps_module_compilation_independent_between_runtimes() -> TestResult {
    let first = Runtime::new().compile_module_named("same.js", "export default 1;")?;
    let second = Runtime::new().compile_module_named("same.js", "export default 2;")?;

    ensure(
        first.source_id() != second.source_id(),
        "distinct module sources need distinct source ids",
    )?;
    ensure(
        first.exports() == second.exports(),
        "equivalent export shapes should match",
    )?;
    Ok(())
}

#[test]
fn links_and_evaluates_named_imports_with_live_cells() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([("dep.js", "export let value = 40; value += 2;".to_owned())]);
    let value = context.eval_module_named(
        "main.js",
        "import { value } from 'dep.js'; value;",
        &mut loader,
    )?;

    ensure(value == Value::Number(42.0), "linked import value mismatch")?;
    ensure(
        context.loaded_module_count() == 2,
        "module record count mismatch",
    )?;
    ensure(
        context.has_loaded_module("dep.js"),
        "dependency module record is missing",
    )?;
    ensure(
        context.root_snapshot()?.count(VmRootKind::ModuleBinding) > 0,
        "module bindings are not rooted",
    )?;
    Ok(())
}

struct MapLoader {
    sources: BTreeMap<String, String>,
}

impl MapLoader {
    fn new<const N: usize>(sources: [(&str, String); N]) -> Self {
        Self {
            sources: sources
                .into_iter()
                .map(|(name, source)| (name.to_owned(), source))
                .collect(),
        }
    }
}

impl ModuleLoader for MapLoader {
    fn load(&mut self, _referrer: &str, request: &str) -> rs_quickjs::Result<ModuleSource> {
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
