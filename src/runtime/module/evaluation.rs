use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        activation::{ActivationFrame, ActivationFrameStorageFootprint},
        binding::scope::BindingScope,
        control::Suspension,
        function::SuspendedExecutionStorageFootprint,
        promise::{PromiseId, PromiseReaction, PromiseSettledState},
        roots::{DirectRootVisitor, VmRootKind},
        storage_ledger::VmStorageLedger,
    },
    syntax::ImportPhase,
    value::Value,
};

use super::{EvaluationState, ModuleDependency, ModuleRecord};

enum StartedModuleDependencies {
    Pending(Vec<(PromiseId, usize)>),
    Rejected(Error),
}

#[derive(Debug)]
pub(in crate::runtime) struct DetachedModuleExecution {
    locals: Vec<BindingScope>,
    activations: Vec<ActivationFrame>,
}

impl DetachedModuleExecution {
    fn detach(context: &mut Context, local_base: usize, activation_base: usize) -> Self {
        Self {
            locals: context.locals.split_off(local_base),
            activations: context.activation_frames.split_off(activation_base),
        }
    }

    fn attach(self, context: &mut Context) -> (usize, usize) {
        let local_base = context.locals.len();
        let activation_base = context.activation_frames.len();
        context.locals.extend(self.locals);
        context.activation_frames.extend(self.activations);
        (local_base, activation_base)
    }

    fn activation_storage_footprint(&self) -> Result<ActivationFrameStorageFootprint> {
        self.activations.iter().try_fold(
            ActivationFrameStorageFootprint::default(),
            |footprint, frame| footprint.checked_add(frame.storage_footprint()?),
        )
    }

    pub(super) fn storage_footprint(&self) -> Result<SuspendedExecutionStorageFootprint> {
        let (bindings, cache_entries) = self.locals.iter().try_fold(
            (0_usize, 0_usize),
            |(bindings, cache_entries), scope| {
                let footprint = scope.storage_footprint()?;
                let bindings = bindings
                    .checked_add(footprint.binding_count())
                    .ok_or_else(|| Error::limit("module binding footprint overflowed"))?;
                let cache_entries = cache_entries
                    .checked_add(footprint.cache_entry_count())
                    .ok_or_else(|| Error::limit("module cache footprint overflowed"))?;
                Ok::<_, Error>((bindings, cache_entries))
            },
        )?;
        let activation = self.activation_storage_footprint()?;
        let bindings = bindings
            .checked_add(activation.binding_count())
            .ok_or_else(|| Error::limit("module binding footprint overflowed"))?;
        let execution_frames = self
            .locals
            .len()
            .checked_add(activation.execution_frame_count())
            .ok_or_else(|| Error::limit("module execution footprint overflowed"))?;
        Ok(SuspendedExecutionStorageFootprint::from_counts(
            bindings,
            cache_entries,
            execution_frames,
        ))
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        for scope in &self.locals {
            for cell in scope.cells() {
                if let Some(result) = cell.with_initialized_value(|value| {
                    visitor.visit_value(VmRootKind::ModuleBinding, value)
                }) {
                    result?;
                }
            }
            for stack in scope.resource_stacks() {
                visitor.visit_value(VmRootKind::ModuleBinding, stack.value())?;
            }
        }
        for frame in &self.activations {
            if let Some(environments) = frame.dynamic_environments() {
                for environment in environments {
                    environment.for_each_value(|value| {
                        visitor.visit_value(VmRootKind::ModuleBinding, value)
                    })?;
                }
            }
            if let Some(upvalues) = frame.upvalues() {
                for cell in upvalues.iter() {
                    if let Some(result) = cell.with_initialized_value(|value| {
                        visitor.visit_value(VmRootKind::ModuleBinding, value)
                    }) {
                        result?;
                    }
                }
            }
            for value in [frame.this_value(), frame.new_target()]
                .into_iter()
                .flatten()
            {
                visitor.visit_value(VmRootKind::ModuleBinding, value)?;
            }
            if let Some(super_binding) = frame.super_binding() {
                if let Some(constructor) = &super_binding.constructor {
                    visitor.visit_value(VmRootKind::ModuleBinding, constructor)?;
                }
                visitor.visit_value(VmRootKind::ModuleBinding, &super_binding.home_object)?;
                if let Some(this_value) = super_binding.this_value.borrow().as_ref() {
                    visitor.visit_value(VmRootKind::ModuleBinding, this_value)?;
                }
            }
            if let Some(continuation) = frame.continuation() {
                if let Some(function) = continuation.function_id() {
                    visitor.visit_value(VmRootKind::ModuleBinding, &Value::Function(function))?;
                }
                for value in continuation.root_values() {
                    visitor.visit_value(VmRootKind::ModuleBinding, value)?;
                }
            }
        }
        Ok(())
    }

    fn cancel_storage(mut self, storage_ledger: &VmStorageLedger) -> Result<()> {
        let activation = self.activation_storage_footprint()?;
        let local_count = self.locals.len();
        for scope in &mut self.locals {
            scope.deactivate_storage()?;
        }
        storage_ledger.release_count(
            crate::runtime::VmStorageKind::Binding,
            activation.binding_count(),
        )?;
        let execution_frames = local_count
            .checked_add(activation.execution_frame_count())
            .ok_or_else(|| Error::limit("cancelled module execution footprint overflowed"))?;
        storage_ledger.release_count(
            crate::runtime::VmStorageKind::ExecutionFrame,
            execution_frames,
        )
    }
}

impl ModuleRecord {
    pub(in crate::runtime) const fn evaluation_promise(&self) -> Option<PromiseId> {
        self.evaluation_promise
    }

    pub(in crate::runtime) const fn evaluation_value(&self) -> Option<&Value> {
        self.evaluation_value.as_ref()
    }

    pub(in crate::runtime) const fn execution(&self) -> Option<&DetachedModuleExecution> {
        self.execution.as_ref()
    }
}

impl Context {
    pub(super) fn evaluate_persisted_module(&mut self, module_index: usize) -> Result<Value> {
        self.begin_persisted_module_evaluation(module_index)?;
        loop {
            let module = self
                .modules
                .get(module_index)
                .ok_or_else(|| Error::runtime("persisted module index is missing"))?;
            match module.state {
                EvaluationState::Evaluated => {
                    return Ok(module.evaluation_value.clone().unwrap_or(Value::Undefined));
                }
                EvaluationState::Errored => {
                    return Err(module.evaluation_error.clone().ok_or_else(|| {
                        Error::runtime("persisted module evaluation error is missing")
                    })?);
                }
                EvaluationState::Pending => {
                    return Err(Error::runtime("persisted module evaluation did not start"));
                }
                EvaluationState::Evaluating => {}
            }
            if self.run_jobs()? == 0 {
                return Err(Error::runtime(
                    "top-level await remained pending after the module job queue drained",
                ));
            }
        }
    }

    pub(in crate::runtime) fn begin_persisted_module_evaluation(
        &mut self,
        module_index: usize,
    ) -> Result<PromiseId> {
        self.start_module_evaluation(module_index, &mut Vec::new())
    }

    fn start_module_evaluation(
        &mut self,
        module_index: usize,
        visiting: &mut Vec<usize>,
    ) -> Result<PromiseId> {
        let state = self
            .modules
            .get(module_index)
            .map(|module| module.state)
            .ok_or_else(|| Error::runtime("persisted module index is missing"))?;
        if state != EvaluationState::Pending {
            return self.module_evaluation_promise(module_index);
        }
        if let Some(canonical) = self
            .modules
            .get(module_index)
            .and_then(|module| module.canonical_module)
        {
            return self.start_canonical_module_alias(module_index, canonical, visiting);
        }
        if let Some(error) = self.cached_module_evaluation_error(module_index)? {
            let promise = self.initialize_module_evaluation(module_index)?;
            self.reject_module_with_error(module_index, error)?;
            return Ok(promise);
        }

        let promise = self.initialize_module_evaluation(module_index)?;
        visiting.push(module_index);
        let dependencies = self
            .modules
            .get(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
            .dependencies
            .to_vec();
        let dependency_state =
            self.start_module_dependencies(module_index, visiting, dependencies)?;
        if visiting.pop() != Some(module_index) {
            return Err(Error::runtime("module evaluation stack mismatch"));
        }
        let pending = match dependency_state {
            StartedModuleDependencies::Pending(pending) => pending,
            StartedModuleDependencies::Rejected(error) => {
                self.reject_module_with_error(module_index, error)?;
                return Ok(promise);
            }
        };
        let pending_count = pending.len();
        self.modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
            .pending_async_dependencies = pending_count;
        for (dependency_promise, dependency) in pending {
            self.add_promise_reaction(
                dependency_promise,
                PromiseReaction::module_dependency(module_index, dependency),
            )?;
        }
        if pending_count == 0 {
            self.execute_module_body(module_index)?;
        }
        Ok(promise)
    }

    fn start_module_dependencies(
        &mut self,
        module_index: usize,
        visiting: &mut Vec<usize>,
        dependencies: Vec<ModuleDependency>,
    ) -> Result<StartedModuleDependencies> {
        let mut pending = Vec::new();
        for dependency in dependencies {
            match dependency.phase {
                ImportPhase::Source => {}
                ImportPhase::Defer => {
                    let mut asynchronous = Vec::new();
                    self.gather_persisted_async_transitive_dependencies(
                        dependency.index,
                        &mut std::collections::BTreeSet::new(),
                        &mut asynchronous,
                    )?;
                    for asynchronous_dependency in asynchronous {
                        self.start_module_evaluation(asynchronous_dependency, visiting)?;
                    }
                }
                ImportPhase::Evaluation => {
                    if let Some(cycle_start) = visiting
                        .iter()
                        .position(|candidate| *candidate == dependency.index)
                    {
                        self.mark_module_cycle(visiting, cycle_start, dependency.index)?;
                        continue;
                    }
                    self.start_module_evaluation(dependency.index, visiting)?;
                    let wait_index =
                        self.module_dependency_wait_target(module_index, dependency.index)?;
                    let dependency_promise = self.module_evaluation_promise(wait_index)?;
                    let dependency_module = self
                        .modules
                        .get(wait_index)
                        .ok_or_else(|| Error::runtime("persisted module dependency disappeared"))?;
                    match dependency_module.state {
                        EvaluationState::Evaluated => {}
                        EvaluationState::Errored => {
                            let error =
                                dependency_module.evaluation_error.clone().ok_or_else(|| {
                                    Error::runtime("dependency evaluation error is missing")
                                })?;
                            return Ok(StartedModuleDependencies::Rejected(error));
                        }
                        EvaluationState::Evaluating => {
                            pending.push((dependency_promise, wait_index));
                        }
                        EvaluationState::Pending => {
                            return Err(Error::runtime(
                                "persisted module dependency did not start",
                            ));
                        }
                    }
                }
            }
        }
        Ok(StartedModuleDependencies::Pending(pending))
    }

    fn mark_module_cycle(
        &mut self,
        visiting: &[usize],
        cycle_start: usize,
        cycle_root: usize,
    ) -> Result<()> {
        for module_index in visiting.iter().skip(cycle_start) {
            self.modules
                .get_mut(*module_index)
                .ok_or_else(|| Error::runtime("module cycle member disappeared"))?
                .cycle_root = cycle_root;
        }
        Ok(())
    }

    fn module_dependency_wait_target(
        &self,
        module_index: usize,
        dependency_index: usize,
    ) -> Result<usize> {
        let module_cycle = self
            .modules
            .get(module_index)
            .map(|module| module.cycle_root)
            .ok_or_else(|| Error::runtime("module dependency owner disappeared"))?;
        let dependency_cycle = self
            .modules
            .get(dependency_index)
            .map(|module| module.cycle_root)
            .ok_or_else(|| Error::runtime("module dependency disappeared"))?;
        if module_cycle == dependency_cycle {
            return Ok(dependency_index);
        }
        Ok(dependency_cycle)
    }

    fn start_canonical_module_alias(
        &mut self,
        module_index: usize,
        canonical: usize,
        visiting: &mut Vec<usize>,
    ) -> Result<PromiseId> {
        let promise = self.initialize_module_evaluation(module_index)?;
        let canonical_promise = self.start_module_evaluation(canonical, visiting)?;
        let canonical_module = self
            .modules
            .get(canonical)
            .ok_or_else(|| Error::runtime("canonical module record disappeared"))?;
        match canonical_module.state {
            EvaluationState::Evaluated => {
                let value = canonical_module
                    .evaluation_value
                    .clone()
                    .unwrap_or(Value::Undefined);
                self.fulfill_module_evaluation(module_index, value)?;
            }
            EvaluationState::Errored => {
                let error = canonical_module.evaluation_error.clone().ok_or_else(|| {
                    Error::runtime("canonical module evaluation error is missing")
                })?;
                self.reject_module_with_error(module_index, error)?;
            }
            EvaluationState::Evaluating => {
                self.add_promise_reaction(
                    canonical_promise,
                    PromiseReaction::module_alias(module_index, canonical),
                )?;
            }
            EvaluationState::Pending => {
                return Err(Error::runtime("canonical module evaluation did not start"));
            }
        }
        Ok(promise)
    }

    fn initialize_module_evaluation(&mut self, module_index: usize) -> Result<PromiseId> {
        let (promise, _object) = self.create_pending_promise()?;
        let module = self
            .modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
        module.state = EvaluationState::Evaluating;
        module.evaluation_promise = Some(promise);
        Ok(promise)
    }

    fn module_evaluation_promise(&self, module_index: usize) -> Result<PromiseId> {
        self.modules
            .get(module_index)
            .and_then(|module| module.evaluation_promise)
            .ok_or_else(|| Error::runtime("module evaluation promise is missing"))
    }

    fn execute_module_body(&mut self, module_index: usize) -> Result<()> {
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
            if module.pending_async_dependencies != 0 || module.execution.is_some() {
                return Err(Error::runtime("module body is not ready for execution"));
            }
            let scope = module
                .scope
                .take()
                .ok_or_else(|| Error::runtime("persisted module scope is unavailable"))?;
            (module.name.clone(), module.script.clone(), scope)
        };
        scope.deactivate_storage()?;
        let local_base = self.locals.len();
        let activation_base = self.activation_frames.len();
        self.push_lexical_scope_with(scope)?;
        let previous_module = self.active_module_name.replace(name);
        let previous_import_meta = self.active_import_meta.replace(import_meta);
        let outcome = self
            .with_module_evaluation(|context| context.evaluate_module_script_suspending(&script));
        self.active_import_meta = previous_import_meta;
        self.active_module_name = previous_module;
        self.handle_module_outcome(module_index, local_base, activation_base, outcome)
    }

    fn handle_module_outcome(
        &mut self,
        module_index: usize,
        local_base: usize,
        activation_base: usize,
        outcome: Result<crate::runtime::bytecode::BytecodeOutcome>,
    ) -> Result<()> {
        match outcome {
            Ok(crate::runtime::bytecode::BytecodeOutcome::Suspended {
                suspension: Suspension::Await(awaited),
                ..
            }) => {
                let execution = DetachedModuleExecution::detach(self, local_base, activation_base);
                self.modules
                    .get_mut(module_index)
                    .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
                    .execution = Some(execution);
                self.add_promise_reaction(awaited, PromiseReaction::module_await(module_index))
            }
            Ok(outcome) => {
                let scope = self.restore_module_scope(local_base, activation_base)?;
                self.modules
                    .get_mut(module_index)
                    .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
                    .scope = Some(scope);
                match self.module_outcome_value(outcome) {
                    Ok(value) => self.fulfill_module_evaluation(module_index, value),
                    Err(error) => self.reject_module_with_error(module_index, error),
                }
            }
            Err(error) => {
                let scope = self.restore_module_scope(local_base, activation_base)?;
                self.modules
                    .get_mut(module_index)
                    .ok_or_else(|| Error::runtime("persisted module index disappeared"))?
                    .scope = Some(scope);
                self.reject_module_with_error(module_index, error)
            }
        }
    }

    fn restore_module_scope(
        &mut self,
        local_base: usize,
        activation_base: usize,
    ) -> Result<BindingScope> {
        let root_count = local_base
            .checked_add(1)
            .ok_or_else(|| Error::limit("module local scope index overflowed"))?;
        while self.locals.len() > root_count {
            if self.pop_lexical_scope()?.is_none() {
                return Err(Error::runtime("module nested scope disappeared"));
            }
        }
        let frames = self.activation_frames.split_off(activation_base);
        let footprint = frames.iter().try_fold(
            ActivationFrameStorageFootprint::default(),
            |footprint, frame| footprint.checked_add(frame.storage_footprint()?),
        )?;
        self.release_frame_storage(footprint)?;
        let Some(mut scope) = self.pop_lexical_scope()? else {
            return Err(Error::runtime("persisted module scope disappeared"));
        };
        scope.activate_storage(self.storage_ledger.clone())?;
        Ok(scope)
    }

    pub(in crate::runtime) fn resume_module_await(
        &mut self,
        module_index: usize,
        state: PromiseSettledState,
    ) -> Result<()> {
        if self
            .modules
            .get(module_index)
            .is_none_or(|module| module.state != EvaluationState::Evaluating)
        {
            return Ok(());
        }
        let (name, import_meta, script, execution) = {
            let module = self
                .modules
                .get_mut(module_index)
                .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
            let import_meta = module
                .import_meta
                .clone()
                .ok_or_else(|| Error::runtime("module import.meta object is missing"))?;
            let execution = module
                .execution
                .take()
                .ok_or_else(|| Error::runtime("module continuation is missing"))?;
            (
                module.name.clone(),
                import_meta,
                module.script.clone(),
                execution,
            )
        };
        let (local_base, activation_base) = execution.attach(self);
        let previous_module = self.active_module_name.replace(name);
        let previous_import_meta = self.active_import_meta.replace(import_meta);
        let completion = state.into_completion();
        let outcome = self.with_module_evaluation(|context| {
            context.resume_module_script(&script, activation_base, completion)
        });
        self.active_import_meta = previous_import_meta;
        self.active_module_name = previous_module;
        self.handle_module_outcome(module_index, local_base, activation_base, outcome)
    }

    pub(in crate::runtime) fn resume_module_dependency(
        &mut self,
        module_index: usize,
        dependency_index: usize,
        state: &PromiseSettledState,
    ) -> Result<()> {
        if self
            .modules
            .get(module_index)
            .is_none_or(|module| module.state != EvaluationState::Evaluating)
        {
            return Ok(());
        }
        if let Some(reason) = state.rejection_value() {
            if let Some(error) = self
                .modules
                .get(dependency_index)
                .and_then(|module| module.evaluation_error.clone())
            {
                return self.reject_module_with_error(module_index, error);
            }
            return self.reject_module_with_value(module_index, reason.clone());
        }
        let remaining = {
            let module = self
                .modules
                .get_mut(module_index)
                .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
            module.pending_async_dependencies = module
                .pending_async_dependencies
                .checked_sub(1)
                .ok_or_else(|| Error::runtime("module dependency count underflowed"))?;
            module.pending_async_dependencies
        };
        if remaining == 0 {
            self.execute_module_body(module_index)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn resume_module_alias(
        &mut self,
        module_index: usize,
        canonical: usize,
        state: &PromiseSettledState,
    ) -> Result<()> {
        if self
            .modules
            .get(module_index)
            .is_none_or(|module| module.state != EvaluationState::Evaluating)
        {
            return Ok(());
        }
        if let Some(reason) = state.rejection_value() {
            if let Some(error) = self
                .modules
                .get(canonical)
                .and_then(|module| module.evaluation_error.clone())
            {
                return self.reject_module_with_error(module_index, error);
            }
            return self.reject_module_with_value(module_index, reason.clone());
        }
        let value = self
            .modules
            .get(canonical)
            .and_then(|module| module.evaluation_value.clone())
            .unwrap_or(Value::Undefined);
        self.fulfill_module_evaluation(module_index, value)
    }

    fn fulfill_module_evaluation(&mut self, module_index: usize, value: Value) -> Result<()> {
        let promise = self.module_evaluation_promise(module_index)?;
        let module = self
            .modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
        module.state = EvaluationState::Evaluated;
        module.evaluation_value = Some(value);
        self.fulfill_promise(promise, Value::Undefined)
    }

    fn reject_module_with_error(&mut self, module_index: usize, error: Error) -> Result<()> {
        let reason = self.dynamic_import_error_value(&error)?;
        let promise = self.module_evaluation_promise(module_index)?;
        let module = self
            .modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
        module.state = EvaluationState::Errored;
        module.evaluation_error = Some(error);
        self.reject_promise(promise, reason)
    }

    fn reject_module_with_value(&mut self, module_index: usize, reason: Value) -> Result<()> {
        let error = Error::javascript_local(self.identity.clone(), reason.clone());
        let promise = self.module_evaluation_promise(module_index)?;
        let module = self
            .modules
            .get_mut(module_index)
            .ok_or_else(|| Error::runtime("persisted module index disappeared"))?;
        module.state = EvaluationState::Errored;
        module.evaluation_error = Some(error);
        self.reject_promise(promise, reason)
    }

    fn cached_module_evaluation_error(&self, module_index: usize) -> Result<Option<Error>> {
        let name = self
            .modules
            .get(module_index)
            .map(|module| module.name.as_str())
            .ok_or_else(|| Error::runtime("persisted module index is missing"))?;
        Ok(self
            .modules
            .iter()
            .enumerate()
            .find(|(index, module)| {
                *index != module_index
                    && module.name == name
                    && module.state == EvaluationState::Errored
            })
            .and_then(|(_, module)| module.evaluation_error.clone()))
    }

    pub(in crate::runtime) fn cancel_module_evaluation(
        &mut self,
        module_index: usize,
    ) -> Result<()> {
        let Some(module) = self.modules.get_mut(module_index) else {
            return Ok(());
        };
        if module.state != EvaluationState::Evaluating {
            return Ok(());
        }
        let execution = module.execution.take();
        module.state = EvaluationState::Errored;
        module.evaluation_error = Some(Error::runtime("module evaluation was cancelled"));
        if let Some(execution) = execution {
            execution.cancel_storage(&self.storage_ledger)?;
        }
        Ok(())
    }

    pub(in crate::runtime) fn suspended_module_execution_storage_footprint(
        &self,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        self.modules.iter().try_fold(
            SuspendedExecutionStorageFootprint::default(),
            |footprint, module| {
                let Some(execution) = &module.execution else {
                    return Ok(footprint);
                };
                footprint.checked_add(execution.storage_footprint()?)
            },
        )
    }
}
