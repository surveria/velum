use crate::{
    error::{Error, Result},
    runtime::{
        Context, VmStorageKind,
        activation::{ActivationFrame, ActivationFrameStorageFootprint},
        async_trace::VmAsyncEdgeKind,
        binding::scope::{BindingScope, BindingScopeStorageFootprint},
        control::Completion,
        promise::PromiseId,
        roots::{DirectRootVisitor, VmRootKind},
        storage_ledger::VmStorageLedger,
        trace::{StrongEdgeReference, StrongEdgeVisitor},
    },
    value::{FunctionId, Value},
};

impl Context {
    pub(in crate::runtime) fn detach_function_execution(
        &mut self,
        function: FunctionId,
    ) -> Result<DetachedFunctionExecution> {
        let activation_index =
            self.activation_frames
                .iter()
                .rposition(|frame| {
                    frame.is_call()
                        && frame.continuation().and_then(
                            crate::runtime::bytecode::BytecodeContinuationFrame::function_id,
                        ) == Some(function)
                })
                .ok_or_else(|| Error::runtime("suspended function activation disappeared"))?;
        let local_base = self
            .activation_frames
            .get(activation_index)
            .and_then(ActivationFrame::local_base)
            .ok_or_else(|| Error::runtime("suspended function local base disappeared"))?;
        let locals = self.locals.split_off(local_base);
        let activations = self.activation_frames.split_off(activation_index);
        Ok(DetachedFunctionExecution::new(
            function,
            locals,
            activations,
        ))
    }

    pub(in crate::runtime) fn detach_suspended_async_function(
        &mut self,
        function: FunctionId,
        result_promise: PromiseId,
    ) -> Result<SuspendedAsyncFunction> {
        let execution = self.detach_function_execution(function)?;
        Ok(SuspendedAsyncFunction::new(result_promise, execution))
    }

    pub(in crate::runtime) fn resume_suspended_async_function(
        &mut self,
        continuation: SuspendedAsyncFunction,
        resume: Completion,
    ) -> Result<Completion> {
        self.resume_function_execution(continuation.take_execution(), resume)
    }

    pub(in crate::runtime) fn resume_function_execution(
        &mut self,
        continuation: DetachedFunctionExecution,
        resume: Completion,
    ) -> Result<Completion> {
        let realm = self.function(continuation.function())?.realm;
        self.with_realm(realm, |context| {
            context.resume_function_execution_in_active_realm(continuation, resume)
        })
    }

    fn resume_function_execution_in_active_realm(
        &mut self,
        continuation: DetachedFunctionExecution,
        resume: Completion,
    ) -> Result<Completion> {
        let function = continuation.function();
        let local_base = self.locals.len();
        let activation_base = self.activation_frames.len();
        let (mut locals, mut activations) = continuation.take_owners();
        let call = activations
            .first_mut()
            .ok_or_else(|| Error::runtime("suspended async activation is empty"))?;
        call.rebase_local_base(local_base)
            .map_err(|()| Error::runtime("suspended async call owner mismatch"))?;
        self.locals.append(&mut locals);
        self.activation_frames.append(&mut activations);

        let setup = self.function_call_setup(function)?;
        let body = setup.bytecode.body().clone();
        let result = match (
            setup.static_name_atom_cache,
            setup.static_binding_cache,
            setup.static_binding_layout,
        ) {
            (Some(atom_cache), Some(binding_cache), Some(binding_layout)) => self
                .with_static_name_caches(atom_cache, binding_cache, binding_layout, |context| {
                    context.resume_bytecode_activation(function, &body, Some(resume))
                }),
            (Some(atom_cache), _, _) => self.with_static_name_atom_cache(atom_cache, |context| {
                context.resume_bytecode_activation(function, &body, Some(resume))
            }),
            (None, _, _) => self.resume_bytecode_activation(function, &body, Some(resume)),
        };
        if result.as_ref().is_ok_and(Completion::suspends_execution) {
            return result;
        }
        if result.is_err() {
            self.discard_execution_suffix(local_base, activation_base)?;
            return result;
        }
        let expected_activation_count = activation_base
            .checked_add(1)
            .ok_or_else(|| Error::limit("suspended activation count overflowed"))?;
        if self.activation_frames.len() != expected_activation_count {
            return Err(Error::runtime(format!(
                "suspended function completed with parked child activations: completion {:?}, expected {expected_activation_count}, actual {}",
                result.as_ref().ok(),
                self.activation_frames.len()
            )));
        }
        let mut result = result;
        if let Ok(completion) = result {
            result = self.dispose_active_binding_scope(completion);
        }
        let binding_result = self.pop_function_binding_storage(
            local_base,
            setup.arguments_binding.is_some(),
            setup.self_binding.is_some(),
        );
        let activation_result = self.pop_call_activation(local_base);
        binding_result?;
        activation_result?;
        result
    }
}

/// Detached ownership for one async function parked at `await`.
///
/// Storage charges remain active while these frames live in a Promise
/// reaction or queued resume job. Reattaching only moves the same owners back
/// to the Context execution stacks.
#[derive(Debug)]
pub(in crate::runtime) struct SuspendedAsyncFunction {
    result_promise: PromiseId,
    execution: DetachedFunctionExecution,
}

#[derive(Debug)]
pub(in crate::runtime) struct DetachedFunctionExecution {
    function: FunctionId,
    locals: Vec<BindingScope>,
    activations: Vec<ActivationFrame>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) struct SuspendedExecutionStorageFootprint {
    bindings: usize,
    cache_entries: usize,
    execution_frames: usize,
}

impl SuspendedExecutionStorageFootprint {
    pub(in crate::runtime) const fn binding_count(self) -> usize {
        self.bindings
    }

    pub(in crate::runtime) const fn cache_entry_count(self) -> usize {
        self.cache_entries
    }

    pub(in crate::runtime) const fn execution_frame_count(self) -> usize {
        self.execution_frames
    }

    pub(in crate::runtime) fn checked_add(self, other: Self) -> Result<Self> {
        let bindings = self
            .bindings
            .checked_add(other.bindings)
            .ok_or_else(suspended_storage_overflow)?;
        let cache_entries = self
            .cache_entries
            .checked_add(other.cache_entries)
            .ok_or_else(suspended_storage_overflow)?;
        let execution_frames = self
            .execution_frames
            .checked_add(other.execution_frames)
            .ok_or_else(suspended_storage_overflow)?;
        Ok(Self {
            bindings,
            cache_entries,
            execution_frames,
        })
    }

    fn with_scope(self, footprint: BindingScopeStorageFootprint) -> Result<Self> {
        self.checked_add(Self {
            bindings: footprint.binding_count(),
            cache_entries: footprint.cache_entry_count(),
            execution_frames: 1,
        })
    }

    fn with_activation(self, footprint: ActivationFrameStorageFootprint) -> Result<Self> {
        self.checked_add(Self {
            bindings: footprint.binding_count(),
            cache_entries: 0,
            execution_frames: footprint.execution_frame_count(),
        })
    }
}

fn suspended_storage_overflow() -> Error {
    Error::limit("suspended execution storage footprint overflowed")
}

impl SuspendedAsyncFunction {
    pub(super) const fn new(
        result_promise: PromiseId,
        execution: DetachedFunctionExecution,
    ) -> Self {
        Self {
            result_promise,
            execution,
        }
    }

    pub(in crate::runtime) const fn function(&self) -> FunctionId {
        self.execution.function()
    }

    pub(in crate::runtime) const fn result_promise(&self) -> PromiseId {
        self.result_promise
    }

    pub(super) fn take_execution(self) -> DetachedFunctionExecution {
        self.execution
    }

    pub(in crate::runtime) fn storage_footprint(
        &self,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        self.execution.storage_footprint()
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
    ) -> Result<()> {
        visitor.visit_promise(VmRootKind::QueuedJob, self.result_promise)?;
        self.execution
            .visit_direct_roots(visitor, VmRootKind::QueuedJob)
    }

    pub(in crate::runtime) fn visit_strong_edges<V>(&self, visitor: &mut V) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            VmAsyncEdgeKind::PromiseReaction,
            StrongEdgeReference::Promise(self.result_promise),
        )?;
        self.execution
            .visit_strong_edges(visitor, VmAsyncEdgeKind::PromiseReaction)
    }

    pub(in crate::runtime) fn cancel_storage(self, storage_ledger: &VmStorageLedger) -> Result<()> {
        self.execution.cancel_storage(storage_ledger)
    }
}

impl DetachedFunctionExecution {
    const fn new(
        function: FunctionId,
        locals: Vec<BindingScope>,
        activations: Vec<ActivationFrame>,
    ) -> Self {
        Self {
            function,
            locals,
            activations,
        }
    }

    pub(in crate::runtime) const fn function(&self) -> FunctionId {
        self.function
    }

    pub(in crate::runtime) fn has_yield_delegate(&self) -> bool {
        self.activations.iter().any(|frame| {
            frame
                .continuation()
                .is_some_and(super::super::bytecode::BytecodeContinuationFrame::has_yield_delegate)
        })
    }

    fn take_owners(self) -> (Vec<BindingScope>, Vec<ActivationFrame>) {
        (self.locals, self.activations)
    }

    fn activation_storage_footprint(&self) -> Result<ActivationFrameStorageFootprint> {
        self.activations.iter().try_fold(
            ActivationFrameStorageFootprint::default(),
            |footprint, frame| footprint.checked_add(frame.storage_footprint()?),
        )
    }

    fn storage_footprint_with_activations(
        &self,
        activations: ActivationFrameStorageFootprint,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        let footprint = self.locals.iter().try_fold(
            SuspendedExecutionStorageFootprint::default(),
            |footprint, scope| footprint.with_scope(scope.storage_footprint()?),
        )?;
        footprint.with_activation(activations)
    }

    pub(in crate::runtime) fn storage_footprint(
        &self,
    ) -> Result<SuspendedExecutionStorageFootprint> {
        self.storage_footprint_with_activations(self.activation_storage_footprint()?)
    }

    pub(in crate::runtime) fn visit_direct_roots<V: DirectRootVisitor>(
        &self,
        visitor: &mut V,
        kind: VmRootKind,
    ) -> Result<()> {
        visitor.visit_value(kind, &Value::Function(self.function))?;
        for scope in &self.locals {
            for cell in scope.cells() {
                if let Some(result) =
                    cell.with_initialized_value(|value| visitor.visit_value(kind, value))
                {
                    result?;
                }
            }
            for stack in scope.resource_stacks() {
                visitor.visit_value(kind, stack.value())?;
            }
        }
        for frame in &self.activations {
            if let Some(environments) = frame.dynamic_environments() {
                for environment in environments {
                    environment.for_each_value(|value| visitor.visit_value(kind, value))?;
                }
            }
            if let Some(upvalues) = frame.upvalues() {
                for cell in upvalues.iter() {
                    if let Some(result) =
                        cell.with_initialized_value(|value| visitor.visit_value(kind, value))
                    {
                        result?;
                    }
                }
            }
            if let Some(value) = frame.this_value() {
                visitor.visit_value(kind, value)?;
            }
            if let Some(value) = frame.new_target() {
                visitor.visit_value(kind, value)?;
            }
            if let Some(super_binding) = frame.super_binding() {
                if let Some(constructor) = &super_binding.constructor {
                    visitor.visit_value(kind, constructor)?;
                }
                visitor.visit_value(kind, &super_binding.home_object)?;
                if let Some(this_value) = super_binding.this_value.borrow().as_ref() {
                    visitor.visit_value(kind, this_value)?;
                }
            }
            if let Some(continuation) = frame.continuation() {
                if let Some(function) = continuation.function_id() {
                    visitor.visit_value(kind, &Value::Function(function))?;
                }
                for value in continuation.root_values() {
                    visitor.visit_value(kind, value)?;
                }
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn visit_strong_edges<V>(
        &self,
        visitor: &mut V,
        kind: VmAsyncEdgeKind,
    ) -> Result<()>
    where
        V: StrongEdgeVisitor<VmAsyncEdgeKind>,
    {
        visitor.visit(
            kind,
            StrongEdgeReference::Value(&Value::Function(self.function)),
        )?;
        for scope in &self.locals {
            for cell in scope.cells() {
                if let Some(result) = cell.with_initialized_value(|value| {
                    visitor.visit(kind, StrongEdgeReference::Value(value))
                }) {
                    result?;
                }
            }
            for stack in scope.resource_stacks() {
                visitor.visit(kind, StrongEdgeReference::Value(stack.value()))?;
            }
        }
        for frame in &self.activations {
            if let Some(environments) = frame.dynamic_environments() {
                for environment in environments {
                    environment.for_each_value(|value| {
                        visitor.visit(kind, StrongEdgeReference::Value(value))
                    })?;
                }
            }
            if let Some(upvalues) = frame.upvalues() {
                for cell in upvalues.iter() {
                    if let Some(result) = cell.with_initialized_value(|value| {
                        visitor.visit(kind, StrongEdgeReference::Value(value))
                    }) {
                        result?;
                    }
                }
            }
            for value in [frame.this_value(), frame.new_target()]
                .into_iter()
                .flatten()
            {
                visitor.visit(kind, StrongEdgeReference::Value(value))?;
            }
            if let Some(super_binding) = frame.super_binding() {
                if let Some(constructor) = &super_binding.constructor {
                    visitor.visit(kind, StrongEdgeReference::Value(constructor))?;
                }
                visitor.visit(kind, StrongEdgeReference::Value(&super_binding.home_object))?;
                if let Some(this_value) = super_binding.this_value.borrow().as_ref() {
                    visitor.visit(kind, StrongEdgeReference::Value(this_value))?;
                }
            }
            if let Some(continuation) = frame.continuation() {
                if let Some(function) = continuation.function_id() {
                    visitor.visit(kind, StrongEdgeReference::Value(&Value::Function(function)))?;
                }
                for value in continuation.root_values() {
                    visitor.visit(kind, StrongEdgeReference::Value(value))?;
                }
            }
        }
        Ok(())
    }

    pub(in crate::runtime) fn cancel_storage(
        mut self,
        storage_ledger: &VmStorageLedger,
    ) -> Result<()> {
        let activation_footprint = self.activation_storage_footprint()?;
        let footprint = self.storage_footprint_with_activations(activation_footprint)?;
        for scope in &mut self.locals {
            scope.deactivate_storage()?;
        }
        storage_ledger
            .release_count(VmStorageKind::Binding, activation_footprint.binding_count())?;
        storage_ledger.release_count(
            VmStorageKind::ExecutionFrame,
            footprint.execution_frame_count(),
        )
    }
}
