use alloc::{collections::BTreeMap, rc::Rc};
use core::fmt;

use parking_lot::Mutex;

use crate::{
    binding_metadata::BindingLayout,
    compiled_module::{
        CompiledModule, DynamicModuleRequest, ModuleImportName, ModuleLoader, ModuleRequest,
    },
    error::{Error, Result},
    runtime::{
        Context,
        binding::{
            scope::{BindingCell, BindingScope},
            static_bindings::StaticBindingCacheHandle,
        },
        bytecode::BytecodeOutcome,
        control::{Completion, Suspension},
        promise::PromiseId,
        property::static_names::StaticNameAtomCacheHandle,
    },
    value::Value,
};

mod deferred;
mod evaluation;
mod namespace;
mod source;

#[derive(Clone)]
pub(super) struct DynamicModuleLoader {
    owner: Rc<Mutex<Box<dyn ModuleLoader>>>,
}

impl DynamicModuleLoader {
    fn new(loader: impl ModuleLoader + 'static) -> Self {
        Self {
            owner: Rc::new(Mutex::new(Box::new(loader))),
        }
    }
}

impl fmt::Debug for DynamicModuleLoader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DynamicModuleLoader")
    }
}

impl ModuleLoader for DynamicModuleLoader {
    fn load(&mut self, referrer: &str, request: &str) -> Result<crate::ModuleSource> {
        self.owner.lock().load(referrer, request)
    }

    fn load_dynamic(
        &mut self,
        referrer: &str,
        request: &DynamicModuleRequest,
    ) -> Result<crate::ModuleSource> {
        self.owner.lock().load_dynamic(referrer, request)
    }

    fn load_static(
        &mut self,
        referrer: &str,
        request: &ModuleRequest,
    ) -> Result<crate::ModuleSource> {
        self.owner.lock().load_static(referrer, request)
    }
}

#[derive(Debug, Clone, Copy)]
struct ModuleDependency {
    phase: crate::syntax::ImportPhase,
    index: usize,
}

#[derive(Debug)]
pub(super) struct ModuleRecord {
    name: String,
    script: crate::CompiledScript,
    dependencies: Box<[ModuleDependency]>,
    scope: Option<BindingScope>,
    namespace: Value,
    deferred_namespace: Value,
    module_source: Option<Value>,
    import_meta: Option<Value>,
    state: EvaluationState,
    evaluation_error: Option<Error>,
    evaluation_promise: Option<PromiseId>,
    evaluation_value: Option<Value>,
    pending_async_dependencies: usize,
    execution: Option<evaluation::DetachedModuleExecution>,
    canonical_module: Option<usize>,
    cycle_root: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum EvaluationState {
    Pending,
    Evaluating,
    Evaluated,
    Errored,
}

#[derive(Debug)]
struct PendingModule {
    name: String,
    module: CompiledModule,
    dependencies: Vec<(ModuleRequest, usize)>,
    scope: Option<BindingScope>,
    namespace: Option<Value>,
    namespace_binding: Option<BindingCell>,
    deferred_namespace: Option<Value>,
    deferred_namespace_binding: Option<BindingCell>,
    module_source_class_name: Option<String>,
    module_source: Option<Value>,
    module_source_binding: Option<BindingCell>,
    import_meta: Option<Value>,
    canonical_module: Option<usize>,
}

impl PendingModule {
    const fn new(
        name: String,
        module: CompiledModule,
        module_source_class_name: Option<String>,
    ) -> Self {
        Self {
            name,
            module,
            dependencies: Vec::new(),
            scope: None,
            namespace: None,
            namespace_binding: None,
            deferred_namespace: None,
            deferred_namespace_binding: None,
            module_source_class_name,
            module_source: None,
            module_source_binding: None,
            import_meta: None,
            canonical_module: None,
        }
    }

    fn dependency_for_specifier(&self, specifier: &str) -> Option<usize> {
        self.dependencies
            .iter()
            .find_map(|(request, index)| (request.specifier() == specifier).then_some(*index))
    }

    fn dependency_for_request(&self, request: &ModuleRequest) -> Option<usize> {
        self.dependencies
            .iter()
            .find_map(|(candidate, index)| (candidate == request).then_some(*index))
    }
}

impl Context {
    /// Installs the VM-owned loader used by dynamic module operations such as
    /// `ShadowRealm.prototype.importValue`.
    pub fn set_dynamic_module_loader(&mut self, loader: impl ModuleLoader + 'static) {
        self.dynamic_module_loader = Some(DynamicModuleLoader::new(loader));
    }

    #[must_use]
    pub const fn loaded_module_count(&self) -> usize {
        self.modules.len()
    }

    #[must_use]
    pub fn has_loaded_module(&self, source_name: &str) -> bool {
        self.modules.iter().any(|module| module.name == source_name)
    }

    /// Compiles, links, and evaluates one ECMAScript module graph with an
    /// embedder-controlled resolver and source loader.
    ///
    /// # Errors
    /// Fails when loading, compilation, linking, evaluation, or configured
    /// resource limits fail.
    pub fn eval_module_named<L: ModuleLoader>(
        &mut self,
        source_name: &str,
        source: &str,
        loader: &mut L,
    ) -> Result<Value> {
        let root = CompiledModule::compile_named(source_name, source, self.limits.clone())?;
        let mut graph = vec![PendingModule::new(source_name.to_owned(), root, None)];
        let mut indices = BTreeMap::from([(source_name.to_owned(), 0_usize)]);
        self.load_module_dependencies(0, &mut graph, &mut indices, loader)?;
        self.instantiate_module_graph(&mut graph)?;
        self.link_static_module_graph(&mut graph)?;
        let root = self.modules.len();
        self.persist_module_graph(graph)?;
        self.evaluate_persisted_module(root)
    }

    pub(in crate::runtime) fn load_dynamic_module_export(
        &mut self,
        request: &str,
        export_name: &str,
    ) -> Result<Value> {
        let referrer = self.active_module_name.clone().unwrap_or_default();
        let request = DynamicModuleRequest::new(
            request,
            crate::syntax::ImportPhase::Evaluation,
            Vec::<(String, String)>::new(),
        );
        let namespace = self.load_dynamic_module_namespace(&referrer, &request)?;
        let property = crate::runtime::property::DynamicPropertyKey::new(
            export_name.to_owned(),
            self.known_property_key(export_name),
        );
        if !self.has_own_property_value(&namespace, &property)? {
            return Err(Error::type_error(format!(
                "module '{}' does not export '{export_name}'",
                request.specifier()
            )));
        }
        self.get_named(&namespace, export_name)
    }

    pub(in crate::runtime) fn load_dynamic_module_namespace(
        &mut self,
        referrer: &str,
        request: &DynamicModuleRequest,
    ) -> Result<Value> {
        let mut loader = self
            .dynamic_module_loader
            .clone()
            .ok_or_else(|| Error::runtime("dynamic module loader is not installed"))?;
        let source = loader.load_dynamic(referrer, request)?;
        if request.phase() == crate::syntax::ImportPhase::Source {
            return Err(Error::exception(
                crate::value::ErrorName::SyntaxError,
                "source phase import is unavailable for source text modules",
            ));
        }
        let specifier = source.specifier().to_owned();
        if let Some(module_index) = self
            .modules
            .iter()
            .position(|module| module.name == specifier)
        {
            if request.phase() == crate::syntax::ImportPhase::Evaluation {
                self.evaluate_persisted_module(module_index)?;
            }
            let module = self
                .modules
                .get(module_index)
                .ok_or_else(|| Error::runtime("cached dynamic module disappeared"))?;
            return Ok(if request.phase() == crate::syntax::ImportPhase::Defer {
                module.deferred_namespace.clone()
            } else {
                module.namespace.clone()
            });
        }
        let root = CompiledModule::compile_named(&specifier, source.source(), self.limits.clone())?;
        let mut graph = vec![PendingModule::new(specifier.clone(), root, None)];
        let mut indices = BTreeMap::from([(specifier, 0_usize)]);
        self.load_module_dependencies(0, &mut graph, &mut indices, &mut loader)?;
        self.instantiate_module_graph(&mut graph)?;
        self.link_module_graph(&mut graph)?;
        self.evaluate_dynamic_module_graph(0, graph, request.phase())
    }

    pub(in crate::runtime) fn begin_dynamic_module_namespace(
        &mut self,
        referrer: &str,
        request: &DynamicModuleRequest,
    ) -> Result<(PromiseId, Value)> {
        if request.phase() != crate::syntax::ImportPhase::Evaluation {
            return Err(Error::runtime(
                "asynchronous module preparation requires evaluation phase",
            ));
        }
        let mut loader = self
            .dynamic_module_loader
            .clone()
            .ok_or_else(|| Error::runtime("dynamic module loader is not installed"))?;
        let source = loader.load_dynamic(referrer, request)?;
        let specifier = source.specifier().to_owned();
        if let Some(module_index) = self
            .modules
            .iter()
            .position(|module| module.name == specifier)
        {
            let promise = self.begin_persisted_module_evaluation(module_index)?;
            let namespace = self
                .modules
                .get(module_index)
                .ok_or_else(|| Error::runtime("cached dynamic module disappeared"))?
                .namespace
                .clone();
            return Ok((promise, namespace));
        }
        let root = CompiledModule::compile_named(&specifier, source.source(), self.limits.clone())?;
        let mut graph = vec![PendingModule::new(specifier.clone(), root, None)];
        let mut indices = BTreeMap::from([(specifier, 0_usize)]);
        self.load_module_dependencies(0, &mut graph, &mut indices, &mut loader)?;
        self.instantiate_module_graph(&mut graph)?;
        self.link_module_graph(&mut graph)?;
        let root = self.modules.len();
        self.persist_module_graph(graph)?;
        let promise = self.begin_persisted_module_evaluation(root)?;
        let namespace = self
            .modules
            .get(root)
            .ok_or_else(|| Error::runtime("dynamic module namespace owner is missing"))?
            .namespace
            .clone();
        Ok((promise, namespace))
    }

    fn with_module_evaluation<T>(
        &mut self,
        evaluate: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        self.module_evaluation_depth = self
            .module_evaluation_depth
            .checked_add(1)
            .ok_or_else(|| Error::limit("module evaluation depth overflowed"))?;
        let result = evaluate(self);
        self.module_evaluation_depth = self
            .module_evaluation_depth
            .checked_sub(1)
            .ok_or_else(|| Error::runtime("module evaluation depth underflowed"))?;
        result
    }

    fn load_module_dependencies<L: ModuleLoader>(
        &self,
        index: usize,
        graph: &mut Vec<PendingModule>,
        indices: &mut BTreeMap<String, usize>,
        loader: &mut L,
    ) -> Result<()> {
        let current = graph
            .get(index)
            .ok_or_else(|| Error::runtime("module graph index is missing"))?;
        let referrer = current.name.clone();
        let requests = current.module.module_requests().to_vec();
        for request in requests {
            let source_record = loader.load_static(&referrer, &request)?;
            let canonical = source_record.specifier().to_owned();
            let module_source_class_name =
                source_record.module_source_class_name().map(str::to_owned);
            if request.phase() == crate::syntax::ImportPhase::Source
                && module_source_class_name.is_none()
            {
                return Err(Self::module_source_unavailable(&canonical));
            }
            let dependency = if let Some(existing) = indices.get(&canonical).copied() {
                if let Some(class_name) = module_source_class_name {
                    let pending = graph
                        .get_mut(existing)
                        .ok_or_else(|| Error::runtime("existing module graph entry disappeared"))?;
                    if pending.module_source_class_name.is_none() {
                        pending.module_source_class_name = Some(class_name);
                    }
                }
                existing
            } else {
                let compiled = CompiledModule::compile_named(
                    &canonical,
                    source_record.source(),
                    self.limits.clone(),
                )?;
                let dependency = graph.len();
                indices.insert(canonical.clone(), dependency);
                graph.push(PendingModule::new(
                    canonical,
                    compiled,
                    module_source_class_name,
                ));
                self.load_module_dependencies(dependency, graph, indices, loader)?;
                dependency
            };
            let current = graph
                .get_mut(index)
                .ok_or_else(|| Error::runtime("module graph index disappeared"))?;
            current.dependencies.push((request, dependency));
        }
        Ok(())
    }

    fn instantiate_module_graph(&mut self, graph: &mut [PendingModule]) -> Result<()> {
        for pending in graph {
            let import_meta = self.create_import_meta()?;
            pending.scope = Some(self.instantiate_module_scope(
                &pending.name,
                import_meta.clone(),
                &pending.module,
            )?);
            pending.import_meta = Some(import_meta);
        }
        Ok(())
    }

    fn instantiate_module_scope(
        &mut self,
        name: &str,
        import_meta: Value,
        module: &CompiledModule,
    ) -> Result<BindingScope> {
        let script = module.script();
        let atom_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
            script.usage().static_call_site_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.push_lexical_scope()?;
        let previous_module = self.active_module_name.replace(name.to_owned());
        let previous_import_meta = self.active_import_meta.replace(import_meta);
        let result = self.with_static_name_caches(
            atom_cache,
            binding_cache,
            script.binding_layout().clone(),
            |context| context.hoist_bytecode_declarations(script.bytecode().hoist_plan()),
        );
        self.active_import_meta = previous_import_meta;
        self.active_module_name = previous_module;
        let scope = self
            .pop_lexical_scope()?
            .ok_or_else(|| Error::runtime("module scope disappeared during instantiation"))?;
        result?;
        Ok(scope)
    }

    fn link_module_graph(&mut self, graph: &mut [PendingModule]) -> Result<()> {
        self.initialize_module_source_objects(graph)?;
        self.initialize_module_namespaces(graph)?;
        self.alias_canonical_module_graph_bindings(graph)?;
        self.validate_indirect_exports(graph)?;
        self.populate_module_namespaces(graph)?;
        for module_index in 0..graph.len() {
            let imports = graph
                .get(module_index)
                .ok_or_else(|| Error::runtime("module graph index is missing"))?
                .module
                .imports()
                .to_vec();
            let mut linked = Vec::new();
            for import in imports {
                let dependency = graph
                    .get(module_index)
                    .and_then(|module| module.dependency_for_request(import.module_request()))
                    .ok_or_else(|| Error::runtime("module dependency was not loaded"))?;
                let cell = match import.import_name() {
                    ModuleImportName::Name(name) => {
                        self.required_export_cell(graph, dependency, name)?
                    }
                    ModuleImportName::Namespace => {
                        let module = graph
                            .get(dependency)
                            .ok_or_else(|| Error::runtime("module namespace owner is missing"))?;
                        if import.phase() == crate::syntax::ImportPhase::Defer {
                            module.deferred_namespace_binding.clone().ok_or_else(|| {
                                Error::runtime("deferred module namespace binding is missing")
                            })?
                        } else {
                            module.namespace_binding.clone().ok_or_else(|| {
                                Error::runtime("module namespace binding is missing")
                            })?
                        }
                    }
                    ModuleImportName::Source => {
                        Self::required_module_source_binding(graph, dependency)?
                    }
                };
                linked.push((import.local_name().to_owned(), cell));
            }
            for (local_name, cell) in linked {
                let atom = self.intern_atom(&local_name)?;
                let scope = graph
                    .get_mut(module_index)
                    .and_then(|module| module.scope.as_mut())
                    .ok_or_else(|| Error::runtime("module scope is not instantiated"))?;
                let import_cell = scope
                    .get(atom)
                    .ok_or_else(|| Error::runtime("module import binding is not declared"))?;
                import_cell.alias_to(cell)?;
            }
        }
        Ok(())
    }

    fn link_static_module_graph(&mut self, graph: &mut [PendingModule]) -> Result<()> {
        let result = self.link_module_graph(graph);
        if let Err(error) = result {
            if let Some(metadata) = error.javascript_error_request()
                && metadata.error_name() == crate::value::ErrorName::SyntaxError
            {
                return Err(Error::runtime(metadata.to_string()));
            }
            return Err(error);
        }
        Ok(())
    }

    fn evaluate_module_script_suspending(
        &mut self,
        script: &crate::CompiledScript,
    ) -> Result<BytecodeOutcome> {
        let atom_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
            script.usage().static_call_site_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.with_static_name_caches(
            atom_cache,
            binding_cache,
            BindingLayout::clone(script.binding_layout()),
            |context| context.eval_bytecode_program_suspending(script.bytecode()),
        )
    }

    fn resume_module_script(
        &mut self,
        script: &crate::CompiledScript,
        activation_base: usize,
        resume: Completion,
    ) -> Result<BytecodeOutcome> {
        let atom_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
            script.usage().static_call_site_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.with_static_name_caches(
            atom_cache,
            binding_cache,
            BindingLayout::clone(script.binding_layout()),
            |context| context.resume_top_level_bytecode(activation_base, resume),
        )
    }

    fn module_outcome_value(&self, outcome: BytecodeOutcome) -> Result<Value> {
        let span = outcome.span();
        match outcome.completion() {
            Completion::Normal(value) => Ok(value),
            Completion::Throw(value) => {
                let metadata = if let Value::Object(id) = &value {
                    self.objects.error_metadata(*id)?.cloned()
                } else {
                    None
                };
                Err(Error::javascript_with_metadata(
                    self.identity.clone(),
                    value,
                    metadata,
                    span,
                ))
            }
            Completion::Return(_) | Completion::ReturnDirect(_) => {
                Err(Error::runtime("return completion escaped module"))
            }
            Completion::TailCall(_) => Err(Error::runtime("tail call escaped module")),
            Completion::Break { .. } | Completion::Continue { .. } => {
                Err(Error::runtime("invalid abrupt completion escaped module"))
            }
            Completion::Suspend(Suspension::Await(_)) => Err(Error::runtime(
                "top-level await module evaluation is not settled yet",
            )),
            Completion::Suspend(
                Suspension::GeneratorStart | Suspension::Yield(_) | Suspension::DelegatedYield(_),
            ) => Err(Error::runtime("generator completion escaped module")),
        }
    }
}
