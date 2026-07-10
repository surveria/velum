use crate::{
    error::{Error, Result},
    value::Value,
};

use super::{
    Context, FunctionUpvalues, VmStorageKind, activation::ActivationFrame,
    binding::scope::BindingScope, function::FunctionSuperBinding,
};

impl Context {
    pub(super) fn push_call_activation(
        &mut self,
        upvalues: FunctionUpvalues,
        this_value: Value,
        new_target: Value,
        super_binding: Option<std::rc::Rc<FunctionSuperBinding>>,
    ) -> Result<usize> {
        self.storage_ledger
            .grow_count(VmStorageKind::Binding, upvalues.len())?;
        if let Err(error) = self
            .storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)
        {
            self.storage_ledger
                .release_count(VmStorageKind::Binding, upvalues.len())?;
            return Err(error);
        }
        let base = self.locals.len();
        self.activation_frames.push(ActivationFrame::call(
            base,
            upvalues,
            this_value,
            new_target,
            super_binding,
        ));
        Ok(base)
    }

    pub(super) fn pop_call_activation(&mut self, base: usize) -> Result<()> {
        let Some(frame) = self.activation_frames.last() else {
            return Err(Error::runtime("function activation frame disappeared"));
        };
        if !frame.is_call() || frame.local_base() != Some(base) {
            return Err(Error::runtime("function activation frame mismatch"));
        }
        if self.locals.len() != base {
            return Err(Error::runtime("function local scope stack mismatch"));
        }
        let Some(frame) = self.activation_frames.pop() else {
            return Err(Error::runtime("function activation frame disappeared"));
        };
        let upvalue_count = frame.upvalues().map_or(0, |upvalues| upvalues.len());
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
        self.activation_frames
            .push(ActivationFrame::temporary_this(value));
        Ok(())
    }

    pub(super) fn pop_temporary_this(&mut self) -> Result<()> {
        let Some(frame) = self.activation_frames.last() else {
            return Err(Error::runtime("class field this binding disappeared"));
        };
        if !frame.is_temporary_this() {
            return Err(Error::runtime("class field this activation mismatch"));
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
}
