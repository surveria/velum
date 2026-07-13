use std::collections::BTreeSet;

use crate::{
    error::{Error, Result},
    runtime::{Context, VmStorageKind, object::PropertyLookup},
    syntax::ImportPhase,
    value::{ObjectId, Value},
};

use super::{EvaluationState, ModuleRecord, PendingModule};

impl Context {
    pub(super) fn evaluate_dynamic_module_graph(
        &mut self,
        root: usize,
        mut graph: Vec<PendingModule>,
        phase: ImportPhase,
    ) -> Result<Value> {
        match phase {
            ImportPhase::Evaluation => {
                self.with_module_evaluation(|context| context.evaluate_module(root, &mut graph))?;
            }
            ImportPhase::Defer => {
                let mut asynchronous = Vec::new();
                Self::gather_async_transitive_dependencies(
                    root,
                    &graph,
                    &mut BTreeSet::new(),
                    &mut asynchronous,
                )?;
                for module in asynchronous {
                    self.with_module_evaluation(|context| {
                        context.evaluate_module(module, &mut graph)
                    })?;
                }
            }
            ImportPhase::Source => {
                return Err(Error::runtime(
                    "source phase module graph reached evaluation",
                ));
            }
        }
        let namespace = graph
            .get(root)
            .and_then(|module| module.namespace.clone())
            .ok_or_else(|| Error::runtime("dynamic module namespace is missing"))?;
        self.persist_module_graph(graph)?;
        Ok(namespace)
    }

    fn gather_async_transitive_dependencies(
        module_index: usize,
        graph: &[PendingModule],
        seen: &mut BTreeSet<usize>,
        result: &mut Vec<usize>,
    ) -> Result<()> {
        if !seen.insert(module_index) {
            return Ok(());
        }
        let module = graph
            .get(module_index)
            .ok_or_else(|| Error::runtime("deferred module graph index is missing"))?;
        if !matches!(module.state, EvaluationState::Pending) {
            return Ok(());
        }
        if module.module.has_top_level_await() {
            result.push(module_index);
            return Ok(());
        }
        for dependency in module.dependencies.values().copied() {
            Self::gather_async_transitive_dependencies(dependency, graph, seen, result)?;
        }
        Ok(())
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
        let has_own = self.objects.has_own(namespace, property)?;
        if property.name() == "then" && !has_own {
            return Ok(());
        }
        let Some(module_index) = self
            .modules
            .iter()
            .position(|module| matches!(module.namespace, Value::Object(id) if id == namespace))
        else {
            return Ok(());
        };
        if !matches!(
            self.modules.get(module_index).map(|module| module.state),
            Some(EvaluationState::Pending)
        ) {
            return Ok(());
        }
        self.with_module_evaluation(|context| context.evaluate_persisted_module(module_index))?;
        Ok(())
    }

    fn evaluate_persisted_module(&mut self, module_index: usize) -> Result<Value> {
        let state = self
            .modules
            .get(module_index)
            .map(|module| module.state)
            .ok_or_else(|| Error::runtime("persisted module index is missing"))?;
        match state {
            EvaluationState::Evaluated | EvaluationState::Evaluating => {
                return Ok(Value::Undefined);
            }
            EvaluationState::Pending => {}
        }
        self.modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
            .state = EvaluationState::Evaluating;
        let dependencies = self
            .modules
            .get(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
            .dependencies
            .to_vec();
        for dependency in dependencies {
            if let Err(error) = self.evaluate_persisted_module(dependency) {
                self.restore_pending_module_state(module_index);
                return Err(error);
            }
        }
        let import_meta = if let Some(import_meta) = self
            .modules
            .get(module_index)
            .and_then(|module| module.import_meta.clone())
        {
            import_meta
        } else {
            let import_meta = self.create_import_meta()?;
            self.modules
                .get_mut(module_index)
                .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
                .import_meta = Some(import_meta.clone());
            import_meta
        };
        let (name, script, mut scope) = {
            let module = self
                .modules
                .get_mut(module_index)
                .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
            let scope = module
                .scope
                .take()
                .ok_or_else(|| Error::runtime("persisted module scope is unavailable"))?;
            (module.name.clone(), module.script.clone(), scope)
        };
        scope.deactivate_storage()?;
        if let Err(error) = self.push_lexical_scope_with(scope) {
            self.restore_pending_module_state(module_index);
            return Err(error);
        }
        let previous_module = self.active_module_name.replace(name);
        let previous_import_meta = self.active_import_meta.replace(import_meta);
        let outcome = self.evaluate_module_script(&script);
        self.active_import_meta = previous_import_meta;
        self.active_module_name = previous_module;
        let scope = self.pop_lexical_scope()?;
        let Some(mut scope) = scope else {
            self.restore_pending_module_state(module_index);
            return Err(Error::runtime(
                "persisted module scope disappeared after evaluation",
            ));
        };
        scope.activate_storage(self.storage_ledger.clone())?;
        self.modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
            .scope = Some(scope);
        let value = match outcome.and_then(|outcome| self.module_outcome_value(outcome)) {
            Ok(value) => value,
            Err(error) => {
                self.restore_pending_module_state(module_index);
                return Err(error);
            }
        };
        self.modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
            .state = EvaluationState::Evaluated;
        Ok(value)
    }

    fn restore_pending_module_state(&mut self, module_index: usize) {
        if let Some(module) = self.modules.get_mut(module_index) {
            module.state = EvaluationState::Pending;
        }
    }

    pub(super) fn persist_module_graph(&mut self, graph: Vec<PendingModule>) -> Result<()> {
        let reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Module, graph.len())?;
        let base = self.modules.len();
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
            let dependencies = pending
                .dependencies
                .values()
                .map(|index| {
                    base.checked_add(*index)
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
                import_meta: pending.import_meta,
                state: pending.state,
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
            if let Some(scope) = record.scope.as_mut() {
                scope.deactivate_storage()?;
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn import_meta_value(&mut self) -> Result<Value> {
        if let Some(import_meta) = &self.active_import_meta {
            return Ok(import_meta.clone());
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
