use crate::{
    error::{Error, Result},
    runtime::{Context, VmStorageKind, binding::scope::BindingScope},
    value::Value,
};

use super::{FunctionProperties, FunctionScopeTemplate, expected_function_local_count};

impl Context {
    pub(crate) fn activate_host_function_property_storage(
        &mut self,
        id: crate::value::HostFunctionId,
    ) -> Result<()> {
        let storage_ledger = self.storage_ledger.clone();
        self.host_function_mut(id)?
            .properties_mut()
            .activate_storage(storage_ledger)
    }

    pub(super) fn function_metadata_cache_count(
        param_binding_count: usize,
        param_atom_count: usize,
        param_frame_count: usize,
        has_fast_path: bool,
        scope_template: Option<&FunctionScopeTemplate>,
        has_self_binding: bool,
        has_arguments_binding: bool,
    ) -> Result<usize> {
        let count = param_binding_count
            .checked_add(param_atom_count)
            .and_then(|count| count.checked_add(param_frame_count))
            .and_then(|count| count.checked_add(usize::from(has_fast_path)))
            .and_then(|count| count.checked_add(usize::from(has_self_binding).saturating_mul(2)))
            .and_then(|count| {
                count.checked_add(usize::from(has_arguments_binding).saturating_mul(2))
            })
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
        binding_count: usize,
        metadata_cache_count: usize,
        mut properties: FunctionProperties,
    ) -> Result<FunctionProperties> {
        let function_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::JavaScriptFunction, 1)?;
        let binding_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Binding, binding_count)?;
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
        arguments_scope: Option<BindingScope>,
        scope: BindingScope,
    ) -> Result<()> {
        if let Some(arguments_scope) = arguments_scope
            && let Err(error) = self.push_lexical_scope_with(arguments_scope)
        {
            self.leave_function_local_frame(local_base)?;
            return Err(error);
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
        has_arguments_binding: bool,
        has_self_binding: bool,
    ) -> Result<()> {
        let expected_local_count =
            expected_function_local_count(local_base, has_arguments_binding, has_self_binding)?;
        let actual_local_count = self.locals.len();
        let local_scope_stack_ok = actual_local_count == expected_local_count;
        self.leave_function_local_frame(local_base)?;
        if !local_scope_stack_ok {
            return Err(Error::runtime(format!(
                "function local scope stack mismatch: expected {expected_local_count}, actual {actual_local_count}"
            )));
        }
        Ok(())
    }

    pub(super) fn named_function_self_scope(
        &self,
        function: crate::value::FunctionId,
        binding: super::FunctionSelfBinding,
    ) -> Result<BindingScope> {
        self.ensure_extra_binding_capacity(1)?;
        let frame = binding.frame();
        let scope = frame
            .scope()
            .ok_or_else(|| Error::runtime("named function binding scope is not local"))?;
        if frame.slot().index() != 0 {
            return Err(Error::runtime(
                "named function binding is not the first self-scope slot",
            ));
        }
        BindingScope::from_compiled_slots(
            scope,
            vec![(
                binding.atom(),
                crate::runtime::binding::scope::BindingCell::named_function(Value::Function(
                    function,
                )),
            )],
        )
    }
}
