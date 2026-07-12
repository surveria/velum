use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    rc::Rc,
};

use parking_lot::Mutex;

use crate::{
    binding_metadata::BindingLayout,
    compiled_module::{CompiledModule, ModuleExport, ModuleImportName, ModuleLoader},
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        binding::{
            scope::{BindingCell, BindingScope},
            static_bindings::StaticBindingCacheHandle,
        },
        bytecode::BytecodeOutcome,
        control::Completion,
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, OBJECT_CONSTRUCTOR_PROPERTY,
            PropertyConfigurable, PropertyEnumerable, PropertyKey, PropertyUpdate,
            PropertyWritable,
        },
        property::static_names::StaticNameAtomCacheHandle,
    },
    value::Value,
};

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
}

#[derive(Debug)]
pub(super) struct ModuleRecord {
    name: String,
    scope: BindingScope,
    namespace: Value,
}

impl ModuleRecord {
    pub(super) const fn scope(&self) -> &BindingScope {
        &self.scope
    }

    pub(super) const fn namespace(&self) -> &Value {
        &self.namespace
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum EvaluationState {
    Pending,
    Evaluating,
    Evaluated,
}

#[derive(Debug)]
enum ExportResolution {
    Found(BindingCell),
    NotFound,
    Ambiguous,
}

#[derive(Debug)]
struct PendingModule {
    name: String,
    module: CompiledModule,
    dependencies: BTreeMap<String, usize>,
    scope: Option<BindingScope>,
    namespace: Option<Value>,
    namespace_binding: Option<BindingCell>,
    state: EvaluationState,
}

impl PendingModule {
    const fn new(name: String, module: CompiledModule) -> Self {
        Self {
            name,
            module,
            dependencies: BTreeMap::new(),
            scope: None,
            namespace: None,
            namespace_binding: None,
            state: EvaluationState::Pending,
        }
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
        let mut graph = vec![PendingModule::new(source_name.to_owned(), root)];
        let mut indices = BTreeMap::from([(source_name.to_owned(), 0_usize)]);
        self.load_module_dependencies(0, &mut graph, &mut indices, loader)?;
        self.instantiate_module_graph(&mut graph)?;
        self.link_module_graph(&mut graph)?;
        let result =
            self.with_module_evaluation(|context| context.evaluate_module(0, &mut graph))?;
        self.persist_module_graph(graph)?;
        Ok(result)
    }

    pub(in crate::runtime) fn load_dynamic_module_export(
        &mut self,
        request: &str,
        export_name: &str,
    ) -> Result<Value> {
        let referrer = self.active_module_name.clone().unwrap_or_default();
        let mut loader = self
            .dynamic_module_loader
            .clone()
            .ok_or_else(|| Error::runtime("dynamic module loader is not installed"))?;
        let source = loader.load(&referrer, request)?;
        let specifier = source.specifier().to_owned();
        self.eval_module_named(&specifier, source.source(), &mut loader)?;
        let namespace = self
            .modules
            .iter()
            .rev()
            .find(|module| module.name == specifier)
            .map(|module| module.namespace.clone())
            .ok_or_else(|| Error::runtime("dynamic module namespace was not persisted"))?;
        let property = crate::runtime::property::DynamicPropertyKey::new(
            export_name.to_owned(),
            self.known_property_key(export_name),
        );
        if !self.has_own_property_value(&namespace, &property)? {
            return Err(Error::type_error(format!(
                "module '{specifier}' does not export '{export_name}'"
            )));
        }
        self.get_named(&namespace, export_name)
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
        let requests = current.module.requests().to_vec();
        for request in requests {
            let source_record = loader.load(&referrer, &request)?;
            let canonical = source_record.specifier().to_owned();
            let dependency = if let Some(existing) = indices.get(&canonical).copied() {
                existing
            } else {
                let compiled = CompiledModule::compile_named(
                    &canonical,
                    source_record.source(),
                    self.limits.clone(),
                )?;
                let dependency = graph.len();
                indices.insert(canonical.clone(), dependency);
                graph.push(PendingModule::new(canonical, compiled));
                self.load_module_dependencies(dependency, graph, indices, loader)?;
                dependency
            };
            let current = graph
                .get_mut(index)
                .ok_or_else(|| Error::runtime("module graph index disappeared"))?;
            current.dependencies.insert(request, dependency);
        }
        Ok(())
    }

    fn instantiate_module_graph(&mut self, graph: &mut [PendingModule]) -> Result<()> {
        for pending in graph {
            pending.scope = Some(self.instantiate_module_scope(&pending.module)?);
        }
        Ok(())
    }

    fn instantiate_module_scope(&mut self, module: &CompiledModule) -> Result<BindingScope> {
        let script = module.script();
        let atom_cache = StaticNameAtomCacheHandle::new(
            script.usage().static_name_count(),
            script.usage().static_property_access_count(),
            script.usage().static_call_site_count(),
        );
        let binding_cache = StaticBindingCacheHandle::new(script.binding_layout().operand_count());
        self.push_lexical_scope()?;
        let result = self.with_static_name_caches(
            atom_cache,
            binding_cache,
            script.binding_layout().clone(),
            |context| context.hoist_bytecode_declarations(script.bytecode().hoist_plan()),
        );
        let scope = self
            .pop_lexical_scope()?
            .ok_or_else(|| Error::runtime("module scope disappeared during instantiation"))?;
        result?;
        Ok(scope)
    }

    fn link_module_graph(&mut self, graph: &mut [PendingModule]) -> Result<()> {
        self.initialize_module_namespaces(graph)?;
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
                    .and_then(|module| module.dependencies.get(import.request()))
                    .copied()
                    .ok_or_else(|| Error::runtime("module dependency was not loaded"))?;
                let cell = match import.import_name() {
                    ModuleImportName::Name(name) => {
                        self.required_export_cell(graph, dependency, name)?
                    }
                    ModuleImportName::Namespace => graph
                        .get(dependency)
                        .and_then(|module| module.namespace_binding.clone())
                        .ok_or_else(|| Error::runtime("module namespace binding is missing"))?,
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

    fn validate_indirect_exports(&mut self, graph: &[PendingModule]) -> Result<()> {
        for module_index in 0..graph.len() {
            let exports = graph
                .get(module_index)
                .ok_or_else(|| Error::runtime("module export owner is missing"))?
                .module
                .exports()
                .to_vec();
            for export in exports {
                let ModuleExport::Indirect { export_name, .. } = export else {
                    continue;
                };
                self.required_export_cell(graph, module_index, &export_name)?;
            }
        }
        Ok(())
    }

    fn initialize_module_namespaces(&mut self, graph: &mut [PendingModule]) -> Result<()> {
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        for pending in graph {
            let namespace = self.objects.create(
                Vec::new(),
                constructor_key,
                self.limits.max_objects,
                self.limits.max_object_properties,
            )?;
            let Value::Object(namespace_id) = namespace else {
                return Err(Error::runtime("module namespace is not an object"));
            };
            self.objects
                .set_prototype_value(namespace_id, &Value::Null)?;
            let namespace = Value::Object(namespace_id);
            pending.namespace_binding = Some(BindingCell::new(
                namespace.clone(),
                false,
                crate::syntax::DeclKind::Const,
            ));
            pending.namespace = Some(namespace);
        }
        Ok(())
    }

    fn populate_module_namespaces(&mut self, graph: &[PendingModule]) -> Result<()> {
        for module_index in 0..graph.len() {
            let names = Self::module_export_names(graph, module_index, &mut BTreeSet::new())?;
            let namespace = graph
                .get(module_index)
                .and_then(|module| module.namespace.as_ref())
                .ok_or_else(|| Error::runtime("module namespace object is missing"))?;
            let Value::Object(namespace_id) = namespace else {
                return Err(Error::runtime("module namespace value is not an object"));
            };
            for name in &names {
                let cell =
                    match self.resolve_export(graph, module_index, name, &mut BTreeSet::new())? {
                        ExportResolution::Found(cell) => cell,
                        ExportResolution::Ambiguous => continue,
                        ExportResolution::NotFound => {
                            return Err(Error::runtime(format!(
                                "module namespace export '{name}' could not be resolved"
                            )));
                        }
                    };
                let getter_name = format!("%module-namespace:{module_index}:{name}%");
                let binding_name = name.clone();
                let getter = self.create_internal_host_function(getter_name, move |_call| {
                    cell.value(&binding_name)
                })?;
                let key = self.intern_property_key(name)?;
                self.objects.define_property(
                    *namespace_id,
                    key,
                    name,
                    PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                        Some(getter),
                        None,
                        Some(PropertyEnumerable::Yes),
                        Some(PropertyConfigurable::No),
                    )),
                    self.limits.max_object_properties,
                )?;
            }
            self.define_module_namespace_to_string_tag(*namespace_id)?;
            self.objects.prevent_extensions(*namespace_id)?;
        }
        Ok(())
    }

    fn define_module_namespace_to_string_tag(
        &mut self,
        namespace: crate::value::ObjectId,
    ) -> Result<()> {
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(tag) = self.get_named(&symbol, "toStringTag")? else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value("Module")?;
        self.objects.define_property(
            namespace,
            PropertyKey::symbol(tag.id()),
            "[Symbol.toStringTag]",
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::No),
            )),
            self.limits.max_object_properties,
        )
    }

    fn module_export_names(
        graph: &[PendingModule],
        module_index: usize,
        visiting: &mut BTreeSet<usize>,
    ) -> Result<BTreeSet<String>> {
        if !visiting.insert(module_index) {
            return Ok(BTreeSet::new());
        }
        let module = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("module namespace owner is missing"))?;
        let mut names = BTreeSet::new();
        for export in module.module.exports() {
            match export {
                ModuleExport::Local { export_name, .. }
                | ModuleExport::Indirect { export_name, .. }
                | ModuleExport::Namespace { export_name, .. } => {
                    names.insert(export_name.clone());
                }
                ModuleExport::Star { request } => {
                    let dependency =
                        module.dependencies.get(request).copied().ok_or_else(|| {
                            Error::runtime("star namespace dependency is missing")
                        })?;
                    let dependency_names = Self::module_export_names(graph, dependency, visiting)?;
                    names.extend(
                        dependency_names
                            .into_iter()
                            .filter(|name| name != "default"),
                    );
                }
            }
        }
        visiting.remove(&module_index);
        Ok(names)
    }

    fn required_export_cell(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        export_name: &str,
    ) -> Result<BindingCell> {
        match self.resolve_export(graph, module_index, export_name, &mut BTreeSet::new())? {
            ExportResolution::Found(cell) => Ok(cell),
            ExportResolution::NotFound => {
                let module_name = graph
                    .get(module_index)
                    .map_or("<missing>", |module| module.name.as_str());
                Err(Error::runtime(format!(
                    "module '{module_name}' does not export '{export_name}'"
                )))
            }
            ExportResolution::Ambiguous => Err(Error::runtime(format!(
                "module export '{export_name}' is ambiguous"
            ))),
        }
    }

    fn resolve_export(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        export_name: &str,
        resolving: &mut BTreeSet<(usize, String)>,
    ) -> Result<ExportResolution> {
        let key = (module_index, export_name.to_owned());
        if !resolving.insert(key.clone()) {
            return Ok(ExportResolution::NotFound);
        }
        let module = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("module export owner is missing"))?;
        let mut star_result = None;
        for export in module.module.exports() {
            match export {
                ModuleExport::Local {
                    export_name: candidate,
                    local_name,
                } if candidate == export_name => {
                    let atom = self.intern_atom(local_name)?;
                    let result = module
                        .scope
                        .as_ref()
                        .and_then(|scope| scope.get(atom))
                        .ok_or_else(|| Error::runtime("local module export is not declared"));
                    resolving.remove(&key);
                    return result.map(ExportResolution::Found);
                }
                ModuleExport::Indirect {
                    export_name: candidate,
                    import_name,
                    request,
                } if candidate == export_name => {
                    let dependency =
                        module.dependencies.get(request).copied().ok_or_else(|| {
                            Error::runtime("indirect export dependency is missing")
                        })?;
                    let result = self.resolve_export(graph, dependency, import_name, resolving);
                    resolving.remove(&key);
                    return result;
                }
                ModuleExport::Namespace {
                    export_name: candidate,
                    request,
                } if candidate == export_name => {
                    let dependency =
                        module.dependencies.get(request).copied().ok_or_else(|| {
                            Error::runtime("namespace export dependency is missing")
                        })?;
                    let namespace = graph
                        .get(dependency)
                        .and_then(|pending| pending.namespace_binding.clone())
                        .ok_or_else(|| {
                            Error::runtime("exported module namespace binding is missing")
                        })?;
                    resolving.remove(&key);
                    return Ok(ExportResolution::Found(namespace));
                }
                ModuleExport::Star { request } if export_name != "default" => {
                    let dependency = module
                        .dependencies
                        .get(request)
                        .copied()
                        .ok_or_else(|| Error::runtime("star export dependency is missing"))?;
                    match self.resolve_export(graph, dependency, export_name, resolving)? {
                        ExportResolution::Found(cell) => {
                            if star_result
                                .as_ref()
                                .is_some_and(|existing: &BindingCell| !existing.same_cell(&cell))
                            {
                                resolving.remove(&key);
                                return Ok(ExportResolution::Ambiguous);
                            }
                            star_result = Some(cell);
                        }
                        ExportResolution::Ambiguous => {
                            resolving.remove(&key);
                            return Ok(ExportResolution::Ambiguous);
                        }
                        ExportResolution::NotFound => {}
                    }
                }
                ModuleExport::Local { .. }
                | ModuleExport::Indirect { .. }
                | ModuleExport::Namespace { .. }
                | ModuleExport::Star { .. } => {}
            }
        }
        resolving.remove(&key);
        Ok(star_result.map_or(ExportResolution::NotFound, ExportResolution::Found))
    }

    fn evaluate_module(
        &mut self,
        module_index: usize,
        graph: &mut [PendingModule],
    ) -> Result<Value> {
        let state = graph
            .get(module_index)
            .map(|module| module.state)
            .ok_or_else(|| Error::runtime("module evaluation index is missing"))?;
        match state {
            EvaluationState::Evaluated | EvaluationState::Evaluating => {
                return Ok(Value::Undefined);
            }
            EvaluationState::Pending => {}
        }
        graph
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("module evaluation index disappeared"))?
            .state = EvaluationState::Evaluating;
        let dependencies = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("module evaluation index disappeared"))?
            .dependencies
            .values()
            .copied()
            .collect::<Vec<_>>();
        for dependency in dependencies {
            self.evaluate_module(dependency, graph)?;
        }

        let (name, script, scope) = {
            let module = graph
                .get_mut(module_index)
                .ok_or_else(|| Error::runtime("module evaluation index disappeared"))?;
            let scope = module
                .scope
                .take()
                .ok_or_else(|| Error::runtime("module scope is not instantiated"))?;
            (module.name.clone(), module.module.script().clone(), scope)
        };
        self.push_lexical_scope_with(scope)?;
        let previous_module = self.active_module_name.replace(name);
        let outcome = self.evaluate_module_script(&script);
        self.active_module_name = previous_module;
        let scope = self
            .pop_lexical_scope()?
            .ok_or_else(|| Error::runtime("module scope disappeared after evaluation"))?;
        let module = graph
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("module evaluation index disappeared"))?;
        module.scope = Some(scope);
        let value = self.module_outcome_value(outcome?)?;
        module.state = EvaluationState::Evaluated;
        Ok(value)
    }

    fn evaluate_module_script(
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
            |context| context.eval_bytecode_program_with_jobs(script.bytecode()),
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
            Completion::Break { .. } | Completion::Continue { .. } => {
                Err(Error::runtime("invalid abrupt completion escaped module"))
            }
            Completion::Suspended(_) => Err(Error::runtime(
                "top-level await module evaluation is not settled yet",
            )),
            Completion::GeneratorStart
            | Completion::Yielded(_)
            | Completion::YieldedIteratorResult(_) => {
                Err(Error::runtime("generator completion escaped module"))
            }
        }
    }

    fn persist_module_graph(&mut self, graph: Vec<PendingModule>) -> Result<()> {
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Module, graph.len())?;
        let mut records = Vec::with_capacity(graph.len());
        for mut pending in graph {
            let Some(mut scope) = pending.scope.take() else {
                Self::deactivate_module_records(&mut records)?;
                return Err(Error::runtime("persisted module scope is missing"));
            };
            let Some(namespace) = pending.namespace.take() else {
                Self::deactivate_module_records(&mut records)?;
                return Err(Error::runtime("persisted module namespace is missing"));
            };
            if let Err(error) = scope.activate_storage(self.storage_ledger.clone()) {
                Self::deactivate_module_records(&mut records)?;
                return Err(error);
            }
            records.push(ModuleRecord {
                name: pending.name,
                scope,
                namespace,
            });
        }
        if let Err(error) = reservation.commit() {
            Self::deactivate_module_records(&mut records)?;
            return Err(error);
        }
        self.modules.extend(records);
        Ok(())
    }

    fn deactivate_module_records(records: &mut [ModuleRecord]) -> Result<()> {
        for record in records.iter_mut().rev() {
            record.scope.deactivate_storage()?;
        }
        Ok(())
    }
}
