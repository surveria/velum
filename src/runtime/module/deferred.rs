use std::collections::BTreeSet;

use crate::{
    error::{Error, Result},
    runtime::{Context, VmStorageKind, object::PropertyLookup},
    syntax::ImportPhase,
    value::{ObjectId, Value},
};

use super::{EvaluationState, ModuleDependency, ModuleRecord, PendingModule};

impl ModuleRecord {
    pub(in crate::runtime) const fn scope(
        &self,
    ) -> Option<&crate::runtime::binding::scope::BindingScope> {
        self.scope.as_ref()
    }

    pub(in crate::runtime) const fn namespace(&self) -> &Value {
        &self.namespace
    }

    pub(in crate::runtime) const fn deferred_namespace(&self) -> &Value {
        &self.deferred_namespace
    }

    pub(in crate::runtime) const fn module_source(&self) -> Option<&Value> {
        self.module_source.as_ref()
    }

    pub(in crate::runtime) const fn import_meta(&self) -> Option<&Value> {
        self.import_meta.as_ref()
    }

    pub(in crate::runtime) fn evaluation_error_value(&self) -> Option<&Value> {
        self.evaluation_error
            .as_ref()
            .and_then(Error::javascript_value)
    }
}

impl Context {
    pub(super) fn evaluate_dynamic_module_graph(
        &mut self,
        root: usize,
        graph: Vec<PendingModule>,
        phase: ImportPhase,
    ) -> Result<Value> {
        let base = self.modules.len();
        let root = base
            .checked_add(root)
            .ok_or_else(|| Error::limit("persisted dynamic module index overflowed"))?;
        self.persist_module_graph(graph)?;
        match phase {
            ImportPhase::Evaluation => {
                self.evaluate_persisted_module(root)?;
            }
            ImportPhase::Defer => {
                let mut asynchronous = Vec::new();
                self.gather_persisted_async_transitive_dependencies(
                    root,
                    &mut BTreeSet::new(),
                    &mut asynchronous,
                )?;
                for module in asynchronous {
                    self.evaluate_persisted_module(module)?;
                }
            }
            ImportPhase::Source => {
                return Err(Error::runtime(
                    "source phase module graph reached evaluation",
                ));
            }
        }
        let module = self
            .modules
            .get(root)
            .ok_or_else(|| Error::runtime("dynamic module namespace owner is missing"))?;
        let namespace = if phase == ImportPhase::Defer {
            module.deferred_namespace.clone()
        } else {
            module.namespace.clone()
        };
        Ok(namespace)
    }

    pub(in crate::runtime) fn evaluate_deferred_module_namespace_property(
        &mut self,
        namespace: ObjectId,
        property: PropertyLookup<'_>,
    ) -> Result<()> {
        if !self.objects.is_module_namespace(namespace)?
            || property.key().is_some_and(|key| key.symbol_id().is_some())
        {
            return Ok(());
        }
        if property.name() == "then" {
            return Ok(());
        }
        self.evaluate_deferred_module_namespace(namespace)
    }

    pub(in crate::runtime) fn evaluate_deferred_module_namespace(
        &mut self,
        namespace: ObjectId,
    ) -> Result<()> {
        let Some(module_index) = self.modules.iter().position(
            |module| matches!(module.deferred_namespace, Value::Object(id) if id == namespace),
        ) else {
            return Ok(());
        };
        match self.modules.get(module_index).map(|module| module.state) {
            Some(EvaluationState::Pending) => {
                if !self
                    .persisted_module_ready_for_sync_execution(module_index, &mut BTreeSet::new())?
                {
                    return Err(Error::type_error(
                        "deferred module is not ready for synchronous evaluation",
                    ));
                }
            }
            Some(EvaluationState::Evaluating) => {
                return Err(Error::type_error(
                    "deferred module cannot be evaluated synchronously while evaluating",
                ));
            }
            Some(EvaluationState::Errored) => {}
            Some(EvaluationState::Evaluated) | None => return Ok(()),
        }
        self.evaluate_persisted_module(module_index)?;
        Ok(())
    }

    fn persisted_module_ready_for_sync_execution(
        &self,
        module_index: usize,
        seen: &mut BTreeSet<usize>,
    ) -> Result<bool> {
        if !seen.insert(module_index) {
            return Ok(true);
        }
        let module = self
            .modules
            .get(module_index)
            .ok_or_else(|| Error::runtime("persisted deferred module is missing"))?;
        match module.state {
            EvaluationState::Evaluated | EvaluationState::Errored => return Ok(true),
            EvaluationState::Evaluating => return Ok(false),
            EvaluationState::Pending => {}
        }
        if module.script.has_top_level_await() {
            return Ok(false);
        }
        for dependency in &module.dependencies {
            if !self.persisted_module_ready_for_sync_execution(dependency.index, seen)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn gather_persisted_async_transitive_dependencies(
        &self,
        module_index: usize,
        seen: &mut BTreeSet<usize>,
        result: &mut Vec<usize>,
    ) -> Result<()> {
        if !seen.insert(module_index) {
            return Ok(());
        }
        let module = self
            .modules
            .get(module_index)
            .ok_or_else(|| Error::runtime("persisted deferred module is missing"))?;
        if !matches!(module.state, EvaluationState::Pending) {
            return Ok(());
        }
        if module.script.has_top_level_await() {
            result.push(module_index);
            return Ok(());
        }
        for dependency in &module.dependencies {
            self.gather_persisted_async_transitive_dependencies(dependency.index, seen, result)?;
        }
        Ok(())
    }

    pub(super) fn persist_module_graph(&mut self, graph: Vec<PendingModule>) -> Result<()> {
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Module, graph.len())?;
        let base = self.modules.len();
        let mut records = Vec::with_capacity(graph.len());
        for mut pending in graph {
            let canonical_module = pending.canonical_module;
            let Some(mut scope) = pending.scope.take() else {
                Self::deactivate_module_records(&mut records)?;
                return Err(Error::runtime("persisted module scope is missing"));
            };
            let Some(namespace) = pending.namespace.take() else {
                Self::deactivate_module_records(&mut records)?;
                return Err(Error::runtime("persisted module namespace is missing"));
            };
            let Some(deferred_namespace) = pending.deferred_namespace.take() else {
                Self::deactivate_module_records(&mut records)?;
                return Err(Error::runtime(
                    "persisted deferred module namespace is missing",
                ));
            };
            let dependencies = pending
                .dependencies
                .iter()
                .map(|(request, index)| {
                    base.checked_add(*index)
                        .map(|index| ModuleDependency {
                            phase: request.phase(),
                            index,
                        })
                        .ok_or_else(|| Error::limit("persisted module index overflowed"))
                })
                .collect::<Result<Vec<_>>>()?
                .into_boxed_slice();
            if let Err(error) = scope.activate_storage(self.storage_ledger.clone()) {
                Self::deactivate_module_records(&mut records)?;
                return Err(error);
            }
            records.push(ModuleRecord {
                name: pending.name,
                script: pending.module.script().clone(),
                dependencies,
                scope: Some(scope),
                namespace,
                deferred_namespace,
                module_source: pending.module_source,
                import_meta: pending.import_meta,
                state: EvaluationState::Pending,
                evaluation_error: None,
                evaluation_promise: None,
                evaluation_value: None,
                pending_async_dependencies: 0,
                execution: None,
                canonical_module,
                cycle_root: base
                    .checked_add(records.len())
                    .ok_or_else(|| Error::limit("module cycle root index overflowed"))?,
            });
        }
        if let Err(error) = reservation.commit() {
            Self::deactivate_module_records(&mut records)?;
            return Err(error);
        }
        self.modules.extend(records);
        Ok(())
    }

    pub(super) fn alias_canonical_module_graph_bindings(
        &mut self,
        graph: &mut [PendingModule],
    ) -> Result<()> {
        for pending in graph {
            let canonical = self
                .modules
                .iter()
                .position(|module| module.name == pending.name);
            pending.canonical_module = canonical;
            if let Some(canonical) = canonical {
                self.alias_canonical_module_bindings(pending, canonical)?;
            }
        }
        Ok(())
    }

    fn alias_canonical_module_bindings(
        &mut self,
        pending: &PendingModule,
        canonical: usize,
    ) -> Result<()> {
        for export in pending.module.exports() {
            let crate::ModuleExport::Local { local_name, .. } = export else {
                continue;
            };
            let atom = self.intern_atom(local_name)?;
            let canonical_cell = self
                .modules
                .get(canonical)
                .and_then(|module| module.scope.as_ref())
                .and_then(|scope| scope.get(atom))
                .ok_or_else(|| Error::runtime("canonical module export binding is missing"))?;
            let pending_cell = pending
                .scope
                .as_ref()
                .and_then(|scope| scope.get(atom))
                .ok_or_else(|| Error::runtime("duplicate module export binding is missing"))?;
            pending_cell.redirect_to_terminal(canonical_cell)?;
        }
        Ok(())
    }

    fn deactivate_module_records(records: &mut [ModuleRecord]) -> Result<()> {
        for record in records.iter_mut().rev() {
            if let Some(scope) = record.scope.as_mut() {
                scope.deactivate_storage()?;
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn import_meta_value(&self) -> Result<Value> {
        if let Some(import_meta) = self.active_script_or_module_import_meta() {
            return Ok(import_meta);
        }
        let Some(name) = self.active_script_or_module_name() else {
            return Err(Error::runtime("import.meta has no active module"));
        };
        self.modules
            .iter()
            .rev()
            .find(|module| module.name == name)
            .and_then(|module| module.import_meta.clone())
            .ok_or_else(|| Error::runtime("import.meta object is unavailable"))
    }

    pub(super) fn create_import_meta(&mut self) -> Result<Value> {
        self.objects
            .create_with_exact_prototype(None, self.limits.max_objects)
    }
}
