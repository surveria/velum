use crate::{
    error::{Error, Result},
    runtime::{Context, VmStorageKind, binding::scope::BindingScope},
    value::Value,
};

use super::{FunctionProperties, FunctionScopeTemplate, expected_function_local_count};

impl Context {
    pub(super) fn function_metadata_cache_count(
        param_binding_count: usize,
        param_atom_count: usize,
        param_frame_count: usize,
        has_fast_path: bool,
        scope_template: Option<&FunctionScopeTemplate>,
    ) -> Result<usize> {
        let count = param_binding_count
            .checked_add(param_atom_count)
            .and_then(|count| count.checked_add(param_frame_count))
            .and_then(|count| count.checked_add(usize::from(has_fast_path)))
            .ok_or_else(|| Error::limit("function metadata cache count overflowed"))?;
        if let Some(template) = scope_template {
            return count
                .checked_add(template.storage_entry_count()?)
                .ok_or_else(|| Error::limit("function metadata cache count overflowed"));
        }
        Ok(count)
    }

    pub(super) fn activate_function_storage(
        &self,
        upvalue_count: usize,
        metadata_cache_count: usize,
        mut properties: FunctionProperties,
    ) -> Result<FunctionProperties> {
        let function_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::JavaScriptFunction, 1)?;
        let binding_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Binding, upvalue_count)?;
        let cache_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::CacheEntry, metadata_cache_count)?;
        properties.activate_storage(self.storage_ledger.clone())?;
        function_reservation.commit()?;
        binding_reservation.commit()?;
        cache_reservation.commit()?;
        Ok(properties)
    }

    pub(super) fn push_function_binding_storage(
        &mut self,
        local_base: usize,
        scope: BindingScope,
        original_args: Option<&[Value]>,
    ) -> Result<()> {
        if let Some(original_args) = original_args {
            let wrapper = match self.arguments_wrapper_scope(original_args) {
                Ok(wrapper) => wrapper,
                Err(error) => {
                    self.leave_function_local_frame(local_base)?;
                    return Err(error);
                }
            };
            if let Err(error) = self.push_lexical_scope_with(wrapper) {
                self.leave_function_local_frame(local_base)?;
                return Err(error);
            }
        }
        if let Err(error) = self.push_lexical_scope_with(scope) {
            self.leave_function_local_frame(local_base)?;
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn pop_function_binding_storage(
        &mut self,
        local_base: usize,
        binds_arguments: bool,
    ) -> Result<()> {
        let expected_local_count = expected_function_local_count(local_base, binds_arguments)?;
        let local_scope_stack_ok = self.locals.len() == expected_local_count;
        self.leave_function_local_frame(local_base)?;
        if !local_scope_stack_ok {
            return Err(Error::runtime("function local scope stack mismatch"));
        }
        Ok(())
    }
}
