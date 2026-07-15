use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use velum::{DynamicModuleRequest, Error, ImportPhase, ModuleLoader, ModuleSource, Runtime, Value};

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

#[test]
fn resolves_named_script_imports_and_forwards_attributes() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (loader, requests) =
        RecordingLoader::new([("scripts/dependency.js", "export const answer = 42;")]);
    context.set_dynamic_module_loader(loader);

    context.eval_named(
        "scripts/main.js",
        r#"
        var imported;
        import("./dependency.js", { with: { type: "javascript" } })
            .then(namespace => { imported = namespace.answer; });
        "#,
    )?;
    let imported = context.eval("imported")?;
    ensure(
        imported == Value::Number(42.0),
        "dynamic import did not resolve",
    )?;

    let requests = requests.borrow();
    let Some(request) = requests.first() else {
        return Err("dynamic loader did not receive a request".into());
    };
    ensure(
        request.referrer == "scripts/main.js",
        "dynamic loader received the wrong referrer",
    )?;
    ensure(
        request.phase == ImportPhase::Evaluation,
        "dynamic loader received the wrong import phase",
    )?;
    ensure(
        request.attributes == [("type".to_owned(), "javascript".to_owned())],
        "dynamic loader received the wrong import attributes",
    )
}

#[test]
fn converts_abrupt_specifier_coercion_into_promise_rejection() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r"
        var marker = {};
        var rejected = false;
        var returned = false;
        const promise = import({ toString() { throw marker; } });
        returned = promise instanceof Promise;
        promise.catch(error => { rejected = error === marker; });
        ",
    )?;
    let value = context.eval("returned && rejected")?;
    ensure(
        value == Value::Bool(true),
        "abrupt specifier coercion did not reject the returned Promise",
    )
}

#[test]
fn rejects_source_phase_imports_from_source_text_modules_with_syntax_error() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (loader, _requests) = RecordingLoader::new([
        (
            "bridge.js",
            "import source value from './plain.js'; export { value };",
        ),
        ("plain.js", "export const value = 1;"),
    ]);
    context.set_dynamic_module_loader(loader);
    context.eval(
        "var sourcePhaseError; \
         import('./bridge.js').catch(error => { \
             sourcePhaseError = error; \
         });",
    )?;
    let value = context.eval(
        "sourcePhaseError instanceof SyntaxError ? true : \
         sourcePhaseError && sourcePhaseError.name + ': ' + sourcePhaseError.message",
    )?;
    if value == Value::Bool(true) {
        return Ok(());
    }
    Err(format!(
        "source text module did not reject source-phase linking with SyntaxError: {value:?}"
    )
    .into())
}

#[test]
fn propagates_abrupt_argument_evaluation_synchronously() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let value = context.eval(
        r#"
        var marker = {};
        var specifierCaught = false;
        var optionsCaught = false;
        const source = { get value() { throw marker; } };
        try { import(source.value); } catch (error) { specifierCaught = error === marker; }
        try { import("unused", (() => { throw marker; })()); }
        catch (error) { optionsCaught = error === marker; }
        specifierCaught && optionsCaught;
        "#,
    )?;
    ensure(
        value == Value::Bool(true),
        "argument evaluation did not preserve synchronous abrupt completion",
    )
}

#[test]
fn reuses_namespace_and_evaluates_a_module_once() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (loader, _requests) = RecordingLoader::new([(
        "dependency.js",
        r#"
        var global = Function("return this;")();
        if (global.dynamicImportEvaluationCount) {
            throw new Error("module was evaluated more than once");
        }
        global.dynamicImportEvaluationCount = 1;
        export const answer = 42;
        "#,
    )]);
    context.set_dynamic_module_loader(loader);
    context.eval_named(
        "main.js",
        r#"
        var importsMatch = false;
        Promise.all([import("./dependency.js"), import("./dependency.js")])
            .then(async values => {
                const third = await import("./dependency.js");
                const fourth = await import("./dependency.js");
                importsMatch = values[0] === values[1]
                    && values[0] === third
                    && third === fourth;
            });
        "#,
    )?;
    let result = context.eval("importsMatch && dynamicImportEvaluationCount === 1")?;
    ensure(
        result == Value::Bool(true),
        "dynamic imports did not reuse one evaluated namespace",
    )
}

#[test]
fn preserves_live_module_bindings_updated_through_the_global_object() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    context.eval(
        r#"
        var dynamicImportGlobalObject = Function("return this;")();
        function dynamicImportGlobal() { return dynamicImportGlobalObject; }
        "#,
    )?;
    let global = context.eval("dynamicImportGlobal() === globalThis")?;
    ensure(
        global == Value::Bool(true),
        "Function constructor did not preserve the script global object",
    )?;
    let (loader, _requests) = RecordingLoader::new([(
        "dependency.js",
        r#"
        var value = 1;
        export { value };
        Function("return this;")().updateDynamicImportValue = function() { value = 2; };
        "#,
    )]);
    context.set_dynamic_module_loader(loader);
    context.eval_named(
        "main.js",
        r#"
        var liveBindingWorked = false;
        import("./dependency.js").then(namespace => {
            const before = namespace.value;
            dynamicImportGlobal().updateDynamicImportValue();
            liveBindingWorked = before === 1 && namespace.value === 2;
        });
        "#,
    )?;
    let result = context.eval("liveBindingWorked")?;
    ensure(
        result == Value::Bool(true),
        "dynamic namespace did not expose the updated live binding",
    )
}

#[test]
fn keeps_named_default_function_exports_mutable() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (loader, _requests) = RecordingLoader::new([(
        "dependency.js",
        "export default function fn() { fn = 2; return 1; }",
    )]);
    context.set_dynamic_module_loader(loader);
    context.eval_named(
        "main.js",
        r#"
        var defaultBindingWorked = false;
        import("./dependency.js").then(namespace => {
            const before = namespace.default();
            defaultBindingWorked = before === 1 && namespace.default === 2;
        });
        "#,
    )?;
    let result = context.eval("defaultBindingWorked")?;
    ensure(
        result == Value::Bool(true),
        "named default function export was not mutable",
    )
}

#[test]
fn exposes_module_namespace_exotic_descriptors() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (loader, _requests) = RecordingLoader::new([(
        "dependency.js",
        "export let answer = 42; export function update() { answer = 43; }",
    )]);
    context.set_dynamic_module_loader(loader);
    context.eval_named(
        "main.js",
        r#"
        var namespaceDescriptorsWorked = false;
        import("./dependency.js").then(namespace => {
            const before = Object.getOwnPropertyDescriptor(namespace, "answer");
            namespace.update();
            const after = Object.getOwnPropertyDescriptor(namespace, "answer");
            namespaceDescriptorsWorked = before.value === 42
                && before.writable === true
                && before.enumerable === true
                && before.configurable === false
                && after.value === 43
                && Reflect.defineProperty(namespace, "answer", { value: 43 })
                && !Reflect.defineProperty(namespace, "answer", { value: 44 })
                && !Reflect.defineProperty(namespace, "missing", {})
                && !Reflect.set(namespace, "answer", 43)
                && Object.isSealed(namespace)
                && !Object.isFrozen(namespace);
        });
        "#,
    )?;
    let result = context.eval("namespaceDescriptorsWorked")?;
    ensure(
        result == Value::Bool(true),
        "module namespace exotic descriptor semantics did not match",
    )
}

#[test]
fn defers_sync_roots_but_evaluates_async_transitive_dependencies() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (mut loader, _requests) = RecordingLoader::new([
        ("setup.js", "globalThis.events = [];"),
        (
            "root.js",
            r#"
            import "./async.js";
            globalThis.events.push("root");
            export const answer = 42;
            "#,
        ),
        (
            "async.js",
            r#"
            import "./dependency.js";
            globalThis.events.push("async start");
            await Promise.resolve();
            globalThis.events.push("async end");
            "#,
        ),
        ("dependency.js", "globalThis.events.push('dependency');"),
    ]);
    context.set_dynamic_module_loader(loader.clone());
    context.eval_module_named(
        "main.js",
        r#"
        import "./setup.js";
        globalThis.deferBefore = "unset";
        globalThis.deferAnswer = "unset";
        import.defer("./root.js").then(namespace => {
            const before = globalThis.events.join(",");
            globalThis.deferBefore = before;
            const answer = namespace.answer;
            globalThis.deferAnswer = answer;
        });
        "#,
        &mut loader,
    )?;
    context.run_jobs()?;
    ensure(
        context.eval(r#"deferBefore === "dependency,async start,async end""#)? == Value::Bool(true),
        "import.defer did not eagerly evaluate async dependencies",
    )?;
    ensure(
        context.eval("deferAnswer === 42")? == Value::Bool(true),
        "import.defer namespace did not expose its export",
    )?;
    ensure(
        context.eval(r#"events.join(",") === "dependency,async start,async end,root""#)?
            == Value::Bool(true),
        "import.defer did not lazily evaluate the sync root",
    )
}

#[test]
fn exposes_stable_null_prototype_import_meta() -> TestResult {
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let (mut loader, _requests) = RecordingLoader::new([]);
    context.eval_module_named(
        "meta.js",
        r"
        globalThis.importMetaWorked = import.meta === import.meta
            && Object.getPrototypeOf(import.meta) === null;
        ",
        &mut loader,
    )?;
    let result = context.eval("globalThis.importMetaWorked")?;
    ensure(
        result == Value::Bool(true),
        "import.meta did not preserve module-local identity and prototype",
    )
}

#[derive(Clone)]
struct RecordingLoader {
    sources: BTreeMap<String, String>,
    requests: Rc<RefCell<Vec<RecordedRequest>>>,
}

impl RecordingLoader {
    fn new<const N: usize>(
        sources: [(&str, &str); N],
    ) -> (Self, Rc<RefCell<Vec<RecordedRequest>>>) {
        let requests = Rc::new(RefCell::new(Vec::new()));
        (
            Self {
                sources: sources
                    .into_iter()
                    .map(|(name, source)| (name.to_owned(), source.to_owned()))
                    .collect(),
                requests: Rc::clone(&requests),
            },
            requests,
        )
    }

    fn source(&self, specifier: &str) -> velum::Result<ModuleSource> {
        let source = self
            .sources
            .get(specifier)
            .cloned()
            .ok_or_else(|| Error::runtime(format!("missing test module '{specifier}'")))?;
        Ok(ModuleSource::new(specifier, source))
    }
}

impl ModuleLoader for RecordingLoader {
    fn load(&mut self, _referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        self.source(request.trim_start_matches("./"))
    }

    fn load_dynamic(
        &mut self,
        referrer: &str,
        request: &DynamicModuleRequest,
    ) -> velum::Result<ModuleSource> {
        let requested = request.specifier().trim_start_matches("./");
        let specifier = referrer.rsplit_once('/').map_or_else(
            || requested.to_owned(),
            |(parent, _)| format!("{parent}/{requested}"),
        );
        self.requests.borrow_mut().push(RecordedRequest {
            referrer: referrer.to_owned(),
            phase: request.phase(),
            attributes: request.attributes().to_vec(),
        });
        self.source(&specifier)
    }
}

struct RecordedRequest {
    referrer: String,
    phase: ImportPhase,
    attributes: Vec<(String, String)>,
}

fn ensure(condition: bool, message: &str) -> TestResult {
    if condition {
        return Ok(());
    }
    Err(message.into())
}
