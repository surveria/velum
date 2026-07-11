use std::rc::Rc;

use super::FunctionSuperBinding;

use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::control::Completion,
    value::{FunctionId, Value},
};

/// A resolved public instance field: the property key computed at class
/// definition time plus the lazily evaluated initializer block.
#[derive(Debug)]
pub(in crate::runtime) struct ResolvedClassField {
    pub(in crate::runtime) key: crate::runtime::object::PropertyKey,
    pub(in crate::runtime) name: String,
    pub(in crate::runtime) initializer: Option<crate::bytecode::BytecodeBlock>,
}

impl Context {
    /// Runs a parent class constructor for `super(...)`: the current `this`
    /// is initialized in place, the parent's return-object override is
    /// ignored, and throws propagate as completions.
    pub(in crate::runtime) fn eval_class_super_constructor_completion(
        &mut self,
        id: FunctionId,
        args: &[Value],
        this_value: &Value,
        new_target: Value,
    ) -> Result<Completion> {
        self.initialize_class_fields(id, this_value)?;
        match self.eval_function_completion_with_this_and_new_target(
            id,
            RuntimeCallArgs::values(args),
            this_value.clone(),
            new_target,
        )? {
            Completion::Normal(_) | Completion::Return(_) => {
                Ok(Completion::Normal(Value::Undefined))
            }
            completion @ Completion::Throw(_) => Ok(completion),
            Completion::Break { .. } => Err(Error::runtime("break statement outside loop")),
            Completion::Continue(_) => Err(Error::runtime("continue statement outside loop")),
            completion @ (Completion::Suspended(_)
            | Completion::GeneratorStart
            | Completion::Yielded(_)
            | Completion::YieldedIteratorResult(_)) => completion
                .into_function_result()
                .map(|_| Completion::Normal(Value::Undefined)),
        }
    }

    pub(in crate::runtime) fn set_function_super_binding(
        &mut self,
        id: FunctionId,
        binding: Rc<FunctionSuperBinding>,
    ) -> Result<()> {
        self.function_mut(id)?.super_binding = Some(binding);
        Ok(())
    }

    pub(in crate::runtime) fn set_function_class_fields(
        &mut self,
        id: FunctionId,
        fields: Rc<[ResolvedClassField]>,
    ) -> Result<()> {
        let previous_count = self
            .function(id)?
            .class_fields
            .as_ref()
            .map_or(0, |existing| existing.len());
        let additional_count = fields.len().saturating_sub(previous_count);
        let removed_count = previous_count.saturating_sub(fields.len());
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::CacheEntry, additional_count)?;
        reservation.commit()?;
        self.function_mut(id)?.class_fields = Some(fields);
        self.storage_ledger
            .release_count(crate::runtime::VmStorageKind::CacheEntry, removed_count)?;
        Ok(())
    }

    /// True when the function is a derived class constructor whose fields
    /// initialize after `super()` instead of at construction entry.
    pub(in crate::runtime) fn is_derived_class_constructor(&self, id: FunctionId) -> bool {
        self.function(id).is_ok_and(|function| {
            function
                .super_binding
                .as_ref()
                .is_some_and(|binding| binding.constructor.is_some())
        })
    }

    /// Defines the class instance fields on a freshly created object with
    /// `this` bound to it while initializers run, in declaration order.
    pub(in crate::runtime) fn initialize_class_fields(
        &mut self,
        id: FunctionId,
        instance: &Value,
    ) -> Result<()> {
        let Some(fields) = self.function(id)?.class_fields.clone() else {
            return Ok(());
        };
        let Value::Object(object_id) = instance else {
            return Ok(());
        };
        for field in fields.iter() {
            self.push_temporary_this(instance.clone())?;
            let value = field
                .initializer
                .as_ref()
                .map_or(Ok(Completion::Normal(Value::Undefined)), |initializer| {
                    self.eval_bytecode_block(initializer)
                });
            self.pop_temporary_this()?;
            let value = value?.into_result()?;
            let update = crate::runtime::object::PropertyUpdate::Data(
                crate::runtime::object::DataPropertyUpdate::new(
                    Some(value),
                    Some(crate::runtime::object::PropertyWritable::Yes),
                    Some(crate::runtime::object::PropertyEnumerable::Yes),
                    Some(crate::runtime::object::PropertyConfigurable::Yes),
                ),
            );
            self.objects.define_property(
                *object_id,
                field.key,
                &field.name,
                update,
                self.limits.max_object_properties,
            )?;
        }
        Ok(())
    }
}
