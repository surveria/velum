use crate::{
    error::{Error, Result},
    runtime::{Context, Function, VmStorageKind, binding::scope::BindingScope},
    value::Value,
};

use super::expected_function_local_count;

const NAMED_BINDING_METADATA_ENTRY_COUNT: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::runtime) struct JavaScriptFunctionStorageFootprint {
    function_count: usize,
    binding_count: usize,
    object_property_count: usize,
    metadata_cache_entry_count: usize,
    cache_entry_count: usize,
    source_record_count: usize,
    source_record_bytes: usize,
}

impl JavaScriptFunctionStorageFootprint {
    pub(in crate::runtime) const fn function_count(self) -> usize {
        self.function_count
    }

    pub(in crate::runtime) const fn binding_count(self) -> usize {
        self.binding_count
    }

    pub(in crate::runtime) const fn object_property_count(self) -> usize {
        self.object_property_count
    }

    pub(in crate::runtime) const fn metadata_cache_entry_count(self) -> usize {
        self.metadata_cache_entry_count
    }

    pub(in crate::runtime) const fn cache_entry_count(self) -> usize {
        self.cache_entry_count
    }

    pub(in crate::runtime) const fn source_record_count(self) -> usize {
        self.source_record_count
    }

    pub(in crate::runtime) const fn source_record_bytes(self) -> usize {
        self.source_record_bytes
    }
}

impl Function {
    pub(in crate::runtime) fn storage_footprint(
        &self,
    ) -> Result<JavaScriptFunctionStorageFootprint> {
        let dynamic_binding_count =
            self.dynamic_environments
                .iter()
                .try_fold(0_usize, |count, environment| {
                    count
                        .checked_add(environment.storage_binding_count()?)
                        .ok_or_else(function_storage_overflow)
                })?;
        let binding_count =
            checked_function_storage_sum([self.upvalues.len(), dynamic_binding_count])?;
        let mut metadata_cache_entry_count = checked_function_storage_sum([
            self.param_binding_ids.len(),
            self.param_atoms.len(),
            self.param_frames.len(),
            if self.self_binding.is_some() {
                NAMED_BINDING_METADATA_ENTRY_COUNT
            } else {
                0
            },
            if self.arguments_binding.is_some() {
                NAMED_BINDING_METADATA_ENTRY_COUNT
            } else {
                0
            },
            self.class_fields.as_ref().map_or(0, |fields| fields.len()),
            self.class_private_slots
                .as_ref()
                .map_or(0, |slots| slots.len()),
            usize::from(self.fast_path.is_some()),
        ])?;
        if let Some(template) = &self.scope_template {
            metadata_cache_entry_count = metadata_cache_entry_count
                .checked_add(template.storage_entry_count()?)
                .ok_or_else(function_storage_overflow)?;
        }
        let object_property_count = self
            .properties
            .storage_property_count()?
            .checked_add(self.private_slots.len())
            .ok_or_else(function_storage_overflow)?;
        let cache_entry_count = metadata_cache_entry_count
            .checked_add(self.properties.storage_cache_entry_count())
            .ok_or_else(function_storage_overflow)?;
        Ok(JavaScriptFunctionStorageFootprint {
            function_count: 1,
            binding_count,
            object_property_count,
            metadata_cache_entry_count,
            cache_entry_count,
            source_record_count: usize::from(self.source.is_some()),
            source_record_bytes: self.source.as_deref().map_or(0, str::len),
        })
    }
}

fn checked_function_storage_sum<const N: usize>(counts: [usize; N]) -> Result<usize> {
    counts.into_iter().try_fold(0_usize, |total, count| {
        total
            .checked_add(count)
            .ok_or_else(function_storage_overflow)
    })
}

fn function_storage_overflow() -> Error {
    Error::limit("function storage footprint overflowed")
}

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

    pub(super) fn activate_function_storage(&self, function: &mut Function) -> Result<()> {
        let footprint = function.storage_footprint()?;
        let function_reservation = self.storage_ledger.reserve_count(
            VmStorageKind::JavaScriptFunction,
            footprint.function_count(),
        )?;
        let binding_reservation = self
            .storage_ledger
            .reserve_count(VmStorageKind::Binding, footprint.binding_count())?;
        let cache_reservation = self.storage_ledger.reserve_count(
            VmStorageKind::CacheEntry,
            footprint.metadata_cache_entry_count(),
        )?;
        let source_reservation = self.storage_ledger.reserve(
            VmStorageKind::SourceRecord,
            footprint.source_record_count(),
            footprint.source_record_bytes(),
        )?;
        function
            .properties
            .activate_storage(self.storage_ledger.clone())?;
        function_reservation.commit()?;
        binding_reservation.commit()?;
        cache_reservation.commit()?;
        source_reservation.commit()?;
        Ok(())
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
        let has_separate_body_scope = self
            .activation_frames
            .last()
            .and_then(crate::runtime::activation::ActivationFrame::function_environment_phase)
            .is_some_and(
                crate::runtime::activation::FunctionEnvironmentPhase::has_separate_body_scope,
            );
        let expected_local_count = expected_function_local_count(
            local_base,
            has_arguments_binding,
            has_self_binding,
            has_separate_body_scope,
        )?;
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
