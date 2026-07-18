use crate::{
    error::{Error, Result},
    value::Value,
};

use super::{
    Context, FunctionUpvalues, VmStorageKind,
    activation::{
        ActivationFrame, ActivationFrameStorageFootprint, FunctionCallActivation,
        FunctionEnvironmentPhase,
    },
    binding::scope::BindingScope,
    function::FunctionSuperBinding,
    private::PrivateEnvironment,
};

impl Context {
    pub(in crate::runtime) fn activate_frame_storage(
        &self,
        footprint: ActivationFrameStorageFootprint,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::Binding, footprint.binding_count())?;
        if let Err(error) = self.storage_ledger.grow_count(
            VmStorageKind::ExecutionFrame,
            footprint.execution_frame_count(),
        ) {
            self.storage_ledger
                .release_count(VmStorageKind::Binding, footprint.binding_count())?;
            return Err(error);
        }
        Ok(())
    }

    pub(in crate::runtime) fn release_frame_storage(
        &self,
        footprint: ActivationFrameStorageFootprint,
    ) -> Result<()> {
        self.storage_ledger
            .release_count(VmStorageKind::Binding, footprint.binding_count())?;
        if let Err(error) = self.storage_ledger.release_count(
            VmStorageKind::ExecutionFrame,
            footprint.execution_frame_count(),
        ) {
            self.storage_ledger
                .grow_count(VmStorageKind::Binding, footprint.binding_count())?;
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn discard_execution_suffix(
        &mut self,
        local_base: usize,
        activation_base: usize,
    ) -> Result<()> {
        while self.locals.len() > local_base {
            if self.pop_lexical_scope()?.is_none() {
                return Err(Error::runtime("discarded local scope disappeared"));
            }
        }
        let frames = self.activation_frames.split_off(activation_base);
        let footprint = frames.iter().try_fold(
            ActivationFrameStorageFootprint::default(),
            |footprint, frame| footprint.checked_add(frame.storage_footprint()?),
        )?;
        self.release_frame_storage(footprint)
    }

    pub(super) fn push_call_activation(
        &mut self,
        activation: FunctionCallActivation,
    ) -> Result<usize> {
        let base = self.locals.len();
        let frame = ActivationFrame::call(base, activation);
        self.activate_frame_storage(frame.storage_footprint()?)?;
        self.activation_frames.push(frame);
        Ok(base)
    }

    pub(super) fn pop_call_activation(&mut self, base: usize) -> Result<()> {
        let Some(frame) = self.activation_frames.last() else {
            return Err(Error::runtime("function activation frame disappeared"));
        };
        if !frame.is_call() || frame.local_base() != Some(base) {
            return Err(Error::runtime(format!(
                "function activation frame mismatch: expected local base {base}, actual {:?}, call {}",
                frame.local_base(),
                frame.is_call()
            )));
        }
        if self.locals.len() != base {
            return Err(Error::runtime("function local scope stack mismatch"));
        }
        if frame
            .continuation()
            .is_some_and(|continuation| !continuation.is_settled())
        {
            return Err(Error::runtime(
                "function structured control stack did not unwind",
            ));
        }
        let Some(frame) = self.activation_frames.pop() else {
            return Err(Error::runtime("function activation frame disappeared"));
        };
        self.release_frame_storage(frame.storage_footprint()?)
    }

    pub(super) fn set_current_function_environment_phase(
        &mut self,
        phase: FunctionEnvironmentPhase,
    ) -> Result<()> {
        let frame = self
            .activation_frames
            .last_mut()
            .ok_or_else(|| Error::runtime("function activation frame disappeared"))?;
        let current = frame
            .function_environment_phase_mut()
            .ok_or_else(|| Error::runtime("active frame is not a function call"))?;
        *current = phase;
        Ok(())
    }

    pub(super) fn leave_function_local_frame(&mut self, base: usize) -> Result<()> {
        while self.locals.len() > base {
            if self.pop_lexical_scope()?.is_none() {
                return Err(Error::runtime("function local scope disappeared"));
            }
        }
        Ok(())
    }

    pub(crate) fn push_lexical_scope(&mut self) -> Result<()> {
        self.push_lexical_scope_with(BindingScope::new())
    }

    pub(crate) fn push_lexical_scope_with(&mut self, mut scope: BindingScope) -> Result<()> {
        scope.activate_storage(self.storage_ledger.clone())?;
        if let Err(error) = self
            .storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)
        {
            scope.deactivate_storage()?;
            return Err(error);
        }
        self.locals.push(scope);
        Ok(())
    }

    pub(crate) fn pop_lexical_scope(&mut self) -> Result<Option<BindingScope>> {
        let Some(mut scope) = self.locals.pop() else {
            return Ok(None);
        };
        scope.deactivate_storage()?;
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)?;
        Ok(Some(scope))
    }

    pub(crate) fn freshen_lexical_scope(&mut self) -> Result<()> {
        let fresh = self
            .locals
            .last()
            .ok_or_else(|| Error::runtime("lexical scope disappeared before iteration"))?
            .fresh_iteration_copy()?;
        let Some(previous) = self.pop_lexical_scope()? else {
            return Err(Error::runtime(
                "lexical scope disappeared while starting iteration",
            ));
        };
        if let Err(error) = self.push_lexical_scope_with(fresh) {
            self.push_lexical_scope_with(previous)?;
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn push_class_evaluation(
        &mut self,
        value: Value,
        super_binding: alloc::rc::Rc<FunctionSuperBinding>,
        private_environment: Option<alloc::rc::Rc<PrivateEnvironment>>,
        class_field_initializer: bool,
    ) -> Result<()> {
        let frame = ActivationFrame::temporary_this(
            value,
            super_binding,
            private_environment,
            class_field_initializer,
        );
        self.activate_frame_storage(frame.storage_footprint()?)?;
        self.activation_frames.push(frame);
        Ok(())
    }

    pub(super) fn pop_temporary_this(&mut self) -> Result<()> {
        let Some(frame) = self.activation_frames.last() else {
            return Err(Error::runtime("class field this binding disappeared"));
        };
        if !frame.is_temporary_this() {
            return Err(Error::runtime("class field this activation mismatch"));
        }
        if frame
            .continuation()
            .is_some_and(|continuation| !continuation.is_settled())
        {
            return Err(Error::runtime(
                "temporary-this structured control stack did not unwind",
            ));
        }
        let Some(frame) = self.activation_frames.pop() else {
            return Err(Error::runtime("class field this binding disappeared"));
        };
        self.release_frame_storage(frame.storage_footprint()?)
    }

    pub(in crate::runtime) fn push_eval_activation_boundary(&mut self) -> Result<usize> {
        let base = self.locals.len();
        let frame = ActivationFrame::eval_boundary(base);
        self.activate_frame_storage(frame.storage_footprint()?)?;
        self.activation_frames.push(frame);
        Ok(base)
    }

    pub(in crate::runtime) fn pop_eval_activation_boundary(&mut self, base: usize) -> Result<()> {
        let Some(frame) = self.activation_frames.last() else {
            return Err(Error::runtime("evaluation activation boundary disappeared"));
        };
        if !frame.is_eval_boundary() || frame.local_base() != Some(base) {
            return Err(Error::runtime("evaluation activation boundary mismatch"));
        }
        if self.locals.len() != base {
            return Err(Error::runtime(
                "evaluation local scope escaped its boundary",
            ));
        }
        if frame
            .continuation()
            .is_some_and(|continuation| !continuation.is_settled())
        {
            return Err(Error::runtime(
                "evaluation structured control stack did not unwind",
            ));
        }
        let Some(frame) = self.activation_frames.pop() else {
            return Err(Error::runtime("evaluation activation boundary disappeared"));
        };
        self.release_frame_storage(frame.storage_footprint()?)
    }

    pub(in crate::runtime) fn current_function_variable_scope_index(
        &self,
    ) -> Result<Option<usize>> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return Ok(None);
            }
            let Some(function_id) = frame.function_id() else {
                continue;
            };
            let phase = frame
                .function_environment_phase()
                .ok_or_else(|| Error::runtime("function environment phase is unavailable"))?;
            if phase == FunctionEnvironmentPhase::ParameterInitialization {
                return Ok(None);
            }
            if phase == FunctionEnvironmentPhase::Setup {
                return Err(Error::runtime(
                    "function variable environment is not initialized",
                ));
            }
            let base = frame
                .local_base()
                .ok_or_else(|| Error::runtime("function local base is unavailable"))?;
            let function = self.function(function_id)?;
            let base_scope_offset = usize::from(function.self_binding.is_some())
                .checked_add(usize::from(function.arguments_binding.is_some()))
                .ok_or_else(|| Error::limit("function variable scope index overflowed"))?;
            let mut index = base
                .checked_add(base_scope_offset)
                .ok_or_else(|| Error::limit("function variable scope index overflowed"))?;
            if phase.has_separate_body_scope() {
                index = index
                    .checked_add(1)
                    .ok_or_else(|| Error::limit("function variable scope index overflowed"))?;
            }
            if self.locals.get(index).is_none() {
                return Err(Error::runtime(
                    "function variable environment scope disappeared",
                ));
            }
            return Ok(Some(index));
        }
        Ok(None)
    }

    pub(in crate::runtime) fn current_function_contains_sloppy_direct_eval(&self) -> Result<bool> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return Ok(false);
            }
            let Some(function_id) = frame.function_id() else {
                continue;
            };
            let bytecode = &self.function(function_id)?.bytecode;
            return Ok(!bytecode.strict() && bytecode.contains_direct_eval());
        }
        Ok(false)
    }

    pub(in crate::runtime) fn current_activation_this(&self) -> Option<&Value> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return None;
            }
            if let Some(value) = frame.this_value() {
                return Some(value);
            }
        }
        None
    }

    pub(in crate::runtime) fn current_activation_new_target(&self) -> Option<&Value> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return None;
            }
            if let Some(value) = frame.new_target() {
                return Some(value);
            }
        }
        None
    }

    pub(in crate::runtime) fn direct_eval_allows_new_target(&self) -> Result<bool> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return Ok(false);
            }
            let Some(function_id) = frame.function_id() else {
                continue;
            };
            return Ok(match self.function(function_id)?.new_target {
                super::FunctionNewTarget::Own => true,
                super::FunctionNewTarget::Lexical {
                    allows_direct_eval, ..
                } => allows_direct_eval,
            });
        }
        Ok(false)
    }

    pub(in crate::runtime) fn current_activation_super(
        &self,
    ) -> Option<alloc::rc::Rc<FunctionSuperBinding>> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return None;
            }
            if frame.is_call() || frame.is_temporary_this() {
                return frame.super_binding().cloned();
            }
        }
        None
    }

    pub(in crate::runtime) fn current_activation_upvalues(&self) -> Option<&FunctionUpvalues> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return None;
            }
            if let Some(upvalues) = frame.upvalues() {
                return Some(upvalues);
            }
        }
        None
    }

    pub(in crate::runtime) fn current_private_environment(
        &self,
    ) -> Option<alloc::rc::Rc<PrivateEnvironment>> {
        self.activation_frames
            .last()
            .and_then(ActivationFrame::private_environment)
            .cloned()
    }

    pub(in crate::runtime) fn set_current_private_environment(
        &mut self,
        environment: Option<alloc::rc::Rc<PrivateEnvironment>>,
    ) -> Result<()> {
        let frame = self
            .activation_frames
            .last_mut()
            .ok_or_else(|| Error::runtime("private environment activation disappeared"))?;
        *frame.private_environment_mut() = environment;
        Ok(())
    }
}
