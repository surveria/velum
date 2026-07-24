use std::{collections::BTreeMap, sync::Arc};

use parking_lot::Mutex;
use velum::{
    DynamicModuleRequest, Engine, Error, ModuleLoader, ModuleRequest, ModuleSource, OwnedValue,
};
use velum_tokio::VmRuntime;

#[derive(Clone)]
struct AppLoader {
    sources: BTreeMap<String, String>,
    requests: Arc<Mutex<Vec<String>>>,
}

impl AppLoader {
    fn new() -> Self {
        Self {
            sources: BTreeMap::from([
                (
                    "app/a.js".to_owned(),
                    r#"
                    import { b } from "./b.js";
                    export const a = "a";
                    export function cycle() { return a + b; }
                    export function stableMeta() {
                        return import.meta === import.meta
                            && Object.getPrototypeOf(import.meta) === null;
                    }
                    "#
                    .to_owned(),
                ),
                (
                    "app/b.js".to_owned(),
                    r#"
                    import { a } from "./a.js";
                    export const b = "b";
                    export function fromA() { return a; }
                    "#
                    .to_owned(),
                ),
                (
                    "app/dynamic.js".to_owned(),
                    "export const answer = 84;".to_owned(),
                ),
            ]),
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn resolve(referrer: &str, request: &str) -> velum::Result<String> {
        let canonical = if request.starts_with("app/") {
            request.to_owned()
        } else if let Some(relative) = request.strip_prefix("./") {
            referrer.rsplit_once('/').map_or_else(
                || relative.to_owned(),
                |(parent, _)| format!("{parent}/{relative}"),
            )
        } else {
            return Err(Error::runtime(format!(
                "module policy rejected bare specifier '{request}'"
            )));
        };
        if !canonical.starts_with("app/") || canonical.contains("..") {
            return Err(Error::runtime(format!(
                "module policy rejected canonical path '{canonical}'"
            )));
        }
        Ok(canonical)
    }

    fn source(&self, referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        let canonical = Self::resolve(referrer, request)?;
        let source = self
            .sources
            .get(&canonical)
            .cloned()
            .ok_or_else(|| Error::runtime(format!("module '{canonical}' was not found")))?;
        Ok(ModuleSource::new(canonical, source))
    }

    fn validate_attributes(request: &ModuleRequest) -> velum::Result<()> {
        if request
            .attributes()
            .iter()
            .all(|(name, value)| name == "type" && value == "javascript")
        {
            return Ok(());
        }
        Err(Error::runtime("module policy rejected import attributes"))
    }
}

impl ModuleLoader for AppLoader {
    fn load(&mut self, referrer: &str, request: &str) -> velum::Result<ModuleSource> {
        self.requests
            .lock()
            .push(format!("load {referrer} -> {request}"));
        self.source(referrer, request)
    }

    fn load_static(
        &mut self,
        referrer: &str,
        request: &ModuleRequest,
    ) -> velum::Result<ModuleSource> {
        Self::validate_attributes(request)?;
        self.requests.lock().push(format!(
            "static {:?} {referrer} -> {} {:?}",
            request.phase(),
            request.specifier(),
            request.attributes()
        ));
        self.source(referrer, request.specifier())
    }

    fn load_dynamic(
        &mut self,
        referrer: &str,
        request: &DynamicModuleRequest,
    ) -> velum::Result<ModuleSource> {
        Self::validate_attributes(request)?;
        self.requests.lock().push(format!(
            "dynamic {:?} {referrer} -> {} {:?}",
            request.phase(),
            request.specifier(),
            request.attributes()
        ));
        self.source(referrer, request.specifier())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = VmRuntime::new(Engine::new())?;
    let vm = runtime.spawn_vm().await?;
    let mut loader = AppLoader::new();
    let requests = Arc::clone(&loader.requests);
    vm.run(move |vm| {
        vm.set_dynamic_module_loader(loader.clone());
        vm.eval_module_named(
            "app/main.js",
            r#"
            import { cycle, stableMeta } from "./a.js" with { type: "javascript" };
            globalThis.moduleResult = `${cycle()}:${stableMeta()}`;
            globalThis.dynamicResult = "pending";
            import("./dynamic.js", { with: { type: "javascript" } }).then(module => {
                globalThis.dynamicResult = module.answer;
            });
            "#,
            &mut loader,
        )?;
        Ok(())
    })
    .await?;
    vm.wait_idle().await?;
    let (static_result, dynamic_result) = vm
        .run(|vm| {
            let static_result = vm.eval_owned("globalThis.moduleResult")?;
            let dynamic_result = vm.eval_owned("globalThis.dynamicResult")?;
            if static_result != OwnedValue::String("ab:true".to_owned())
                || dynamic_result != OwnedValue::Number(84.0)
            {
                return Err(Error::runtime(format!(
                    "module results were {static_result:?} and {dynamic_result:?}"
                )));
            }
            Ok((format!("{static_result:?}"), format!("{dynamic_result:?}")))
        })
        .await?;
    println!("Static cycle and import.meta: {static_result}");
    println!("Dynamic import: {dynamic_result}");
    for request in requests.lock().iter() {
        println!("Loader: {request}");
    }
    Ok(())
}
