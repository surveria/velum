use std::collections::{BTreeMap, BTreeSet};

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
        object::{OBJECT_CONSTRUCTOR_PROPERTY, ObjectPropertyInit},
        property::static_names::StaticNameAtomCacheHandle,
    },
    value::Value,
};

#[derive(Debug)]
pub(super) struct ModuleRecord {
    name: String,
    scope: BindingScope,
}

impl ModuleRecord {
    pub(super) const fn scope(&self) -> &BindingScope {
        &self.scope
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum EvaluationState {
    Pending,
    Evaluating,
    Evaluated,
}

#[derive(Debug)]
struct PendingModule {
    name: String,
    module: CompiledModule,
    dependencies: BTreeMap<String, usize>,
    scope: Option<BindingScope>,
    state: EvaluationState,
}

impl PendingModule {
    const fn new(name: String, module: CompiledModule) -> Self {
        Self {
            name,
            module,
            dependencies: BTreeMap::new(),
            scope: None,
            state: EvaluationState::Pending,
        }
    }
}

impl Context {
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
        let result = self.evaluate_module(0, &mut graph)?;
        self.persist_module_graph(graph)?;
        Ok(result)
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
                        self.resolve_export_cell(graph, dependency, name, &mut BTreeSet::new())?
                    }
                    ModuleImportName::Namespace => {
                        let namespace = self.create_module_namespace(graph, dependency)?;
                        BindingCell::new(namespace, false, crate::syntax::DeclKind::Const)
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
                scope.insert_or_replace(atom, cell)?;
            }
        }
        Ok(())
    }

    fn create_module_namespace(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
    ) -> Result<Value> {
        let names = Self::module_export_names(graph, module_index, &mut BTreeSet::new())?;
        let mut properties = Vec::with_capacity(names.len());
        for name in &names {
            let cell = self.resolve_export_cell(graph, module_index, name, &mut BTreeSet::new())?;
            let getter_name = format!("%module-namespace:{module_index}:{name}%");
            let binding_name = name.clone();
            let getter = self.create_internal_host_function(getter_name, move |_call| {
                cell.value(&binding_name)
            })?;
            let key = self.intern_property_key(name)?;
            properties.push(ObjectPropertyInit::new_accessor(
                key,
                name,
                getter,
                crate::syntax::AccessorKind::Getter,
            ));
        }
        let constructor_key = self.intern_property_key(OBJECT_CONSTRUCTOR_PROPERTY)?;
        self.objects.create(
            properties,
            constructor_key,
            self.limits.max_objects,
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

    fn resolve_export_cell(
        &mut self,
        graph: &[PendingModule],
        module_index: usize,
        export_name: &str,
        resolving: &mut BTreeSet<(usize, String)>,
    ) -> Result<BindingCell> {
        let key = (module_index, export_name.to_owned());
        if !resolving.insert(key.clone()) {
            return Err(Error::runtime("cyclic module export resolution"));
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
                    return result;
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
                    let result =
                        self.resolve_export_cell(graph, dependency, import_name, resolving);
                    resolving.remove(&key);
                    return result;
                }
                ModuleExport::Star { request } if export_name != "default" => {
                    let dependency = module
                        .dependencies
                        .get(request)
                        .copied()
                        .ok_or_else(|| Error::runtime("star export dependency is missing"))?;
                    if let Ok(cell) =
                        self.resolve_export_cell(graph, dependency, export_name, resolving)
                    {
                        if star_result.is_some() {
                            resolving.remove(&key);
                            return Err(Error::runtime("ambiguous star module export"));
                        }
                        star_result = Some(cell);
                    }
                }
                ModuleExport::Local { .. }
                | ModuleExport::Indirect { .. }
                | ModuleExport::Namespace { .. }
                | ModuleExport::Star { .. } => {}
            }
        }
        resolving.remove(&key);
        star_result.ok_or_else(|| {
            Error::runtime(format!(
                "module '{}' does not export '{export_name}'",
                module.name
            ))
        })
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

        let (script, scope) = {
            let module = graph
                .get_mut(module_index)
                .ok_or_else(|| Error::runtime("module evaluation index disappeared"))?;
            let scope = module
                .scope
                .take()
                .ok_or_else(|| Error::runtime("module scope is not instantiated"))?;
            (module.module.script().clone(), scope)
        };
        self.push_lexical_scope_with(scope)?;
        let outcome = self.evaluate_module_script(&script);
        let scope = self
            .pop_lexical_scope()?
            .ok_or_else(|| Error::runtime("module scope disappeared after evaluation"))?;
        let module = graph
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("module evaluation index disappeared"))?;
        module.scope = Some(scope);
        let value = Self::module_outcome_value(outcome?)?;
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
            |context| context.eval_bytecode_program(script.bytecode()),
        )
    }

    fn module_outcome_value(outcome: BytecodeOutcome) -> Result<Value> {
        match outcome.completion() {
            Completion::Normal(value) => Ok(value),
            Completion::Throw(value) => Err(Error::javascript(value)),
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
        let mut records = Vec::with_capacity(graph.len());
        for mut pending in graph {
            let Some(mut scope) = pending.scope.take() else {
                return Err(Error::runtime("persisted module scope is missing"));
            };
            scope.activate_storage(self.storage_ledger.clone())?;
            records.push(ModuleRecord {
                name: pending.name,
                scope,
            });
        }
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Module, records.len())?;
        reservation.commit()?;
        self.modules.extend(records);
        Ok(())
    }
}
