use crate::{
    error::{Error, Result},
    value::{FunctionId, Value},
};

use super::{
    Context, FunctionActivationEnvironment, FunctionUpvalues, VmStorageKind,
    activation::ActivationFrame, binding::scope::BindingScope, function::FunctionSuperBinding,
    private::PrivateEnvironment,
};

impl Context {
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
        let upvalue_count = frames.iter().try_fold(0_usize, |count, frame| {
            count
                .checked_add(frame.upvalues().map_or(0, |upvalues| upvalues.len()))
                .and_then(|count| {
                    count.checked_add(frame.with_environments().map_or(0, <[Value]>::len))
                })
                .ok_or_else(|| Error::limit("discarded upvalue count overflowed"))
        })?;
        let execution_frame_count = frames.iter().try_fold(frames.len(), |count, frame| {
            count
                .checked_add(frame.continuation().map_or(
                    0,
                    crate::runtime::bytecode::BytecodeContinuationFrame::control_count,
                ))
                .ok_or_else(|| Error::limit("discarded execution frame count overflowed"))
        })?;
        self.storage_ledger
            .release_count(VmStorageKind::Binding, upvalue_count)?;
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, execution_frame_count)
    }

    pub(super) fn push_call_activation(
        &mut self,
        function: FunctionId,
        environment: FunctionActivationEnvironment,
        this_value: Value,
        new_target: Value,
        super_binding: Option<std::rc::Rc<FunctionSuperBinding>>,
        private_environment: Option<std::rc::Rc<PrivateEnvironment>>,
    ) -> Result<usize> {
        let (upvalues, with_environments) = environment;
        let binding_count = upvalues
            .len()
            .checked_add(with_environments.len())
            .ok_or_else(|| Error::limit("call captured binding count overflowed"))?;
        self.storage_ledger
            .grow_count(VmStorageKind::Binding, binding_count)?;
        if let Err(error) = self
            .storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)
        {
            self.storage_ledger
                .release_count(VmStorageKind::Binding, binding_count)?;
            return Err(error);
        }
        let base = self.locals.len();
        self.activation_frames.push(ActivationFrame::call(
            function,
            base,
            (upvalues, with_environments),
            this_value,
            new_target,
            super_binding,
            private_environment,
        ));
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
        let upvalue_count = frame
            .upvalues()
            .map_or(0, |upvalues| upvalues.len())
            .checked_add(frame.with_environments().map_or(0, <[Value]>::len))
            .ok_or_else(|| Error::limit("call captured binding count overflowed"))?;
        self.storage_ledger
            .release_count(VmStorageKind::Binding, upvalue_count)?;
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)
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

    pub(super) fn push_temporary_this(&mut self, value: Value) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        let private_environment = self.current_private_environment();
        self.activation_frames
            .push(ActivationFrame::temporary_this(value, private_environment));
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
        let Some(_frame) = self.activation_frames.pop() else {
            return Err(Error::runtime("class field this binding disappeared"));
        };
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)
    }

    pub(in crate::runtime) fn push_eval_activation_boundary(&mut self) -> Result<usize> {
        self.storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        let base = self.locals.len();
        self.activation_frames
            .push(ActivationFrame::eval_boundary(base));
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
        let Some(_frame) = self.activation_frames.pop() else {
            return Err(Error::runtime("evaluation activation boundary disappeared"));
        };
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)
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

    pub(in crate::runtime) fn current_activation_super(
        &self,
    ) -> Option<std::rc::Rc<FunctionSuperBinding>> {
        for frame in self.activation_frames.iter().rev() {
            if frame.is_eval_boundary() {
                return None;
            }
            if frame.is_call() {
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
    ) -> Option<std::rc::Rc<PrivateEnvironment>> {
        self.activation_frames
            .last()
            .and_then(ActivationFrame::private_environment)
            .cloned()
    }

    pub(in crate::runtime) fn set_current_private_environment(
        &mut self,
        environment: Option<std::rc::Rc<PrivateEnvironment>>,
    ) -> Result<()> {
        let frame = self
            .activation_frames
            .last_mut()
            .ok_or_else(|| Error::runtime("private environment activation disappeared"))?;
        *frame.private_environment_mut() = environment;
        Ok(())
    }
}
