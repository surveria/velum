use std::rc::Rc;

use super::FunctionSuperBinding;

use crate::runtime::private::{PrivateNameId, PrivateSlot, PrivateSlotValue};
use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::call::RuntimeCallArgs,
    runtime::control::Completion,
    value::{FunctionId, Value},
};

/// One resolved instance field initialized during construction.
#[derive(Debug)]
pub(in crate::runtime) enum ResolvedClassField {
    Public {
        key: crate::runtime::object::PropertyKey,
        name: String,
        initializer: Option<crate::bytecode::BytecodeBlock>,
    },
    Private {
        name: PrivateNameId,
        initializer: Option<crate::bytecode::BytecodeBlock>,
    },
}

impl ResolvedClassField {
    pub(in crate::runtime) const fn property_key(
        &self,
    ) -> Option<crate::runtime::object::PropertyKey> {
        match self {
            Self::Public { key, .. } => Some(*key),
            Self::Private { .. } => None,
        }
    }
}

impl Context {
    pub(in crate::runtime) fn add_function_private_slot(
        &mut self,
        id: FunctionId,
        name: PrivateNameId,
        value: PrivateSlotValue,
    ) -> Result<()> {
        let function = self.function(id)?;
        if function.private_slots.iter().any(|slot| slot.id == name) {
            return Err(Error::type_error("private slot is already defined"));
        }
        let property_count = function
            .properties
            .storage_property_count()?
            .checked_add(function.private_slots.len())
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| Error::limit("function property count overflowed"))?;
        if property_count > self.limits.max_object_properties {
            return Err(Error::limit(
                "function property count exceeded configured limit",
            ));
        }
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::ObjectProperty, 1)?;
        self.function_mut(id)?
            .private_slots
            .push(PrivateSlot { id: name, value });
        reservation.commit()
    }

    pub(in crate::runtime) fn function_private_slot(
        &self,
        id: FunctionId,
        name: PrivateNameId,
    ) -> Result<Option<PrivateSlotValue>> {
        Ok(self
            .function(id)?
            .private_slots
            .iter()
            .find(|slot| slot.id == name)
            .map(|slot| slot.value.clone()))
    }

    pub(in crate::runtime) fn set_function_private_field(
        &mut self,
        id: FunctionId,
        name: PrivateNameId,
        value: Value,
    ) -> Result<bool> {
        let Some(slot) = self
            .function_mut(id)?
            .private_slots
            .iter_mut()
            .find(|slot| slot.id == name)
        else {
            return Ok(false);
        };
        let PrivateSlotValue::Field(current) = &mut slot.value else {
            return Ok(false);
        };
        *current = value;
        Ok(true)
    }

    pub(in crate::runtime) fn replace_function_private_slot(
        &mut self,
        id: FunctionId,
        name: PrivateNameId,
        value: PrivateSlotValue,
    ) -> Result<()> {
        let slot = self
            .function_mut(id)?
            .private_slots
            .iter_mut()
            .find(|slot| slot.id == name)
            .ok_or_else(|| Error::runtime("private slot disappeared"))?;
        slot.value = value;
        Ok(())
    }

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
            Completion::Normal(_) | Completion::Return(_) | Completion::ReturnDirect(_) => {
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

    pub(in crate::runtime) fn set_function_class_private_slots(
        &mut self,
        id: FunctionId,
        slots: Rc<[PrivateSlot]>,
    ) -> Result<()> {
        let previous_count = self
            .function(id)?
            .class_private_slots
            .as_ref()
            .map_or(0, |existing| existing.len());
        let additional_count = slots.len().saturating_sub(previous_count);
        let removed_count = previous_count.saturating_sub(slots.len());
        let reservation = self
            .storage_ledger
            .reserve_count(crate::runtime::VmStorageKind::CacheEntry, additional_count)?;
        reservation.commit()?;
        self.function_mut(id)?.class_private_slots = Some(slots);
        self.storage_ledger
            .release_count(crate::runtime::VmStorageKind::CacheEntry, removed_count)
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
        let private_slots = self.function(id)?.class_private_slots.clone();
        let fields = self.function(id)?.class_fields.clone();
        if let Some(private_slots) = private_slots {
            for slot in private_slots.iter() {
                self.add_private_slot_to_value(instance, slot.id, slot.value.clone())?;
            }
        }
        let Some(fields) = fields else {
            return Ok(());
        };
        for field in fields.iter() {
            self.push_temporary_this(instance.clone())?;
            let initializer = match field {
                ResolvedClassField::Public { initializer, .. }
                | ResolvedClassField::Private { initializer, .. } => initializer,
            };
            let value = initializer
                .as_ref()
                .map_or(Ok(Completion::Normal(Value::Undefined)), |initializer| {
                    self.eval_bytecode_block(initializer)
                });
            self.pop_temporary_this()?;
            let value = value?.into_result()?;
            match field {
                ResolvedClassField::Public { key, name, .. } => {
                    let Value::Object(object_id) = instance else {
                        return Err(Error::type_error("class field receiver is not an object"));
                    };
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
                        *key,
                        name,
                        update,
                        self.limits.max_object_properties,
                    )?;
                }
                ResolvedClassField::Private { name, .. } => {
                    self.add_private_slot_to_value(
                        instance,
                        *name,
                        PrivateSlotValue::Field(value),
                    )?;
                }
            }
        }
        Ok(())
    }
}
