use rs_quickjs::{ModuleExport, ModuleImportName, Runtime};

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

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.into())
}
