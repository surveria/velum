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

    let missing_export = runtime.compile_module_named("missing.js", "export { missing };");
    ensure(
        missing_export.is_err(),
        "unbound local exports must fail during compilation",
    )?;

    let restricted_import = runtime.compile_module_named("restricted.js", "import eval from 'x';");
    ensure(
        restricted_import.is_err(),
        "strict restricted import bindings must fail",
    )?;

    let template_request =
        runtime.compile_module_named("template-request.js", "import value from `dependency`; ");
    ensure(
        template_request.is_err(),
        "module requests must use StringLiteral grammar",
    )?;

    let duplicate_function =
        runtime.compile_module_named("duplicate-function.js", "function f() {} function f() {}");
    ensure(
        duplicate_function.is_err(),
        "duplicate module function declarations must fail",
    )?;

    let duplicate_default = runtime.compile_module_named(
        "duplicate-default.js",
        "class F {} export default function F() {}",
    );
    ensure(
        duplicate_default.is_err(),
        "named default declaration must conflict with an existing lexical binding",
    )?;

    let escaped_default =
        runtime.compile_module_named("escaped-default.js", r"export d\u0065fault 0;");
    ensure(
        escaped_default.is_err(),
        "escaped default export keyword must fail",
    )?;

    let invoked_anonymous =
        runtime.compile_module_named("invoked-anonymous.js", "export default function() {}();");
    ensure(
        invoked_anonymous.is_err(),
        "invoked anonymous function must not parse as a default declaration",
    )?;
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

#[test]
fn namespace_import_properties_read_live_export_cells() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([("dep.js", "export let value = 1; value = 2;".to_owned())]);
    let value = context.eval_module_named(
        "main.js",
        "import * as namespace from 'dep.js'; namespace.value;",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(2.0),
        "namespace export did not stay live",
    )
}

#[test]
fn namespace_re_exports_share_sealed_null_prototype_objects() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([
        ("dep.js", "export let value = 7;".to_owned()),
        (
            "bridge.js",
            "export * as dependency from 'dep.js';".to_owned(),
        ),
    ]);
    let value = context.eval_module_named(
        "main.js",
        r"
            import { dependency } from 'bridge.js';
            import * as direct from 'dep.js';
            const descriptor = Object.getOwnPropertyDescriptor(dependency, 'value');
            dependency === direct &&
                Object.getPrototypeOf(dependency) === null &&
                !Object.isExtensible(dependency) &&
                descriptor.enumerable &&
                !descriptor.configurable &&
                dependency.value === 7;
        ",
        &mut loader,
    )?;

    ensure(
        value == Value::Bool(true),
        "module namespace exotic invariants mismatch",
    )
}

#[test]
fn evaluates_cycles_without_replaying_module_bodies() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([
        (
            "a.js",
            "import { b } from 'b.js'; export let a = 1; export function total() { return a + b; }"
                .to_owned(),
        ),
        (
            "b.js",
            "import { a } from 'a.js'; export let b = 2; export function readA() { return a; }"
                .to_owned(),
        ),
    ]);
    let value = context.eval_module_named(
        "main.js",
        "import { total } from 'a.js'; import { readA } from 'b.js'; total() + readA();",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(4.0),
        "cyclic module graph produced the wrong bindings",
    )
}

#[test]
fn accepts_diamond_star_exports_that_resolve_to_one_binding() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([
        ("base.js", "export const value = 9;".to_owned()),
        ("left.js", "export * from 'base.js';".to_owned()),
        ("right.js", "export * from 'base.js';".to_owned()),
        (
            "diamond.js",
            "export * from 'left.js'; export * from 'right.js';".to_owned(),
        ),
    ]);
    let value = context.eval_module_named(
        "main.js",
        "import { value } from 'diamond.js'; value;",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(9.0),
        "diamond star export was treated as ambiguous",
    )
}

#[test]
fn propagates_re_exported_import_bindings_without_ambiguity() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([
        ("base.js", "export const value = 11;".to_owned()),
        ("direct.js", "export { value } from 'base.js';".to_owned()),
        (
            "imported.js",
            "import { value } from 'base.js'; export { value };".to_owned(),
        ),
        (
            "aggregator.js",
            "export * from 'direct.js'; export * from 'imported.js';".to_owned(),
        ),
    ]);
    let value = context.eval_module_named(
        "main.js",
        "import { value } from 'aggregator.js'; value;",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(11.0),
        "re-exported import binding was treated as ambiguous",
    )
}

#[test]
fn omits_ambiguous_star_names_but_rejects_indirect_exports() -> TestResult {
    let sources = [
        (
            "first.js",
            "export const first = 1; export const both = 1;".to_owned(),
        ),
        (
            "second.js",
            "export const second = 2; export const both = 2;".to_owned(),
        ),
        (
            "aggregator.js",
            "export * from 'first.js'; export * from 'second.js';".to_owned(),
        ),
    ];
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new(sources.clone());
    let value = context.eval_module_named(
        "main.js",
        "import * as ns from 'aggregator.js'; 'first' in ns && 'second' in ns && !('both' in ns);",
        &mut loader,
    )?;
    ensure(
        value == Value::Bool(true),
        "ambiguous star name was not omitted from the namespace",
    )?;

    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new(sources);
    let result = context.eval_module_named(
        "invalid.js",
        "export { both } from 'aggregator.js';",
        &mut loader,
    );
    ensure(
        matches!(result, Err(Error::Runtime { .. })),
        "ambiguous indirect export did not fail during static linking",
    )
}

#[test]
fn calls_exported_functions_after_module_evaluation() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([(
        "dep.js",
        "export let value = 40; export function answer() { return value + 2; }".to_owned(),
    )]);
    let value = context.eval_module_named(
        "main.js",
        "import { answer } from 'dep.js'; answer();",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(42.0),
        "exported module function did not retain its environment",
    )
}

#[test]
fn names_anonymous_default_export_functions() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([(
        "dep.js",
        "export default function() { return 42; }".to_owned(),
    )]);
    let value = context.eval_module_named(
        "main.js",
        "import answer from 'dep.js'; answer.name + ':' + answer();",
        &mut loader,
    )?;

    ensure(
        value == Value::from("default:42"),
        "anonymous default export received the wrong function name",
    )
}

#[test]
fn exposes_named_default_declaration_bindings() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([(
        "dep.js",
        "export default function F() { return 1; } F.extra = 41;".to_owned(),
    )]);
    let value = context.eval_module_named(
        "main.js",
        "import F from 'dep.js'; F() + F.extra;",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(42.0),
        "named default declaration binding was not initialized",
    )
}

#[test]
fn settles_top_level_await_through_module_jobs() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([]);
    let value = context.eval_module_named(
        "main.js",
        "let value = 0; for await (const item of [await Promise.resolve(40)]) { value = item; await 0; } for (const key in await Promise.resolve({ delta: 2 })) { value += 2; } value;",
        &mut loader,
    )?;

    ensure(
        value == Value::Number(42.0),
        "top-level await did not resume to completion",
    )?;
    ensure(
        context.pending_job_count() == 0,
        "module evaluation left settled jobs queued",
    )
}

#[test]
fn module_top_level_this_is_undefined() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let mut loader = MapLoader::new([]);
    let value = context.eval_module_named("main.js", "this;", &mut loader)?;

    ensure(
        value == Value::Undefined,
        "module top-level this must be undefined",
    )
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
