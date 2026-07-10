use std::rc::Rc;

use crate::{
    error::{Error, Result},
    value::Value,
};

use super::{Context, VmStorageKind, binding::scope::BindingScope, function::FunctionSuperBinding};

impl Context {
    pub(crate) fn enter_function_local_frame(&mut self) -> Result<usize> {
        self.storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        let base = self.locals.len();
        self.local_frame_bases.push(base);
        Ok(base)
    }

    pub(crate) fn leave_function_local_frame(&mut self, base: usize) -> Result<()> {
        while self.locals.len() > base {
            if self.pop_lexical_scope()?.is_none() {
                return Err(Error::runtime("function local scope disappeared"));
            }
        }
        let removed = self.local_frame_bases.pop();
        if removed != Some(base) {
            return Err(Error::runtime("function local frame base disappeared"));
        }
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)
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

    pub(super) fn push_call_execution_state(
        &mut self,
        this_value: Value,
        new_target: Value,
        super_binding: Option<Rc<FunctionSuperBinding>>,
    ) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 3)?;
        self.this_values.push(this_value);
        self.new_target_values.push(new_target);
        self.super_frames.push(super_binding);
        Ok(())
    }

    pub(super) fn pop_call_execution_state(&mut self) -> Result<()> {
        let removed_super = self.super_frames.pop();
        let removed_new_target = self.new_target_values.pop();
        let removed_this = self.this_values.pop();
        let released = usize::from(removed_super.is_some())
            .checked_add(usize::from(removed_new_target.is_some()))
            .and_then(|count| count.checked_add(usize::from(removed_this.is_some())))
            .ok_or_else(|| Error::limit("execution frame release count overflowed"))?;
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, released)?;
        if removed_this.is_none() {
            return Err(Error::runtime("function this binding disappeared"));
        }
        if removed_super.is_none() {
            return Err(Error::runtime("function super frame disappeared"));
        }
        if removed_new_target.is_none() {
            return Err(Error::runtime("function new.target binding disappeared"));
        }
        Ok(())
    }

    pub(super) fn push_temporary_this(&mut self, value: Value) -> Result<()> {
        self.storage_ledger
            .grow_count(VmStorageKind::ExecutionFrame, 1)?;
        self.this_values.push(value);
        Ok(())
    }

    pub(super) fn pop_temporary_this(&mut self) -> Result<()> {
        let Some(_value) = self.this_values.pop() else {
            return Err(Error::runtime("class field this binding disappeared"));
        };
        self.storage_ledger
            .release_count(VmStorageKind::ExecutionFrame, 1)
    }
}
