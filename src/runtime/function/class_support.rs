use std::rc::Rc;

use crate::runtime::private::{PrivateNameId, PrivateSlot, PrivateSlotValue};
use crate::{
    error::{Error, Result},
    runtime::Context,
    runtime::control::{Completion, TailCallReturnMode},
    value::{FunctionId, Value},
};

use super::FunctionClassConstructor;

/// Super references available to a class constructor or method body.
#[derive(Debug)]
pub(in crate::runtime) struct FunctionSuperBinding {
    pub(in crate::runtime) constructor: Option<Value>,
    pub(in crate::runtime) home_object: Value,
    pub(in crate::runtime) own_constructor: Option<FunctionId>,
    pub(in crate::runtime) this_value: std::cell::RefCell<Option<Value>>,
    pub(in crate::runtime) allow_direct_eval_super_call: std::cell::Cell<bool>,
}

pub(super) fn activation_super_bindings(
    id: FunctionId,
    binding: Option<Rc<FunctionSuperBinding>>,
) -> (
    Option<Rc<FunctionSuperBinding>>,
    Option<Rc<FunctionSuperBinding>>,
) {
    let binding = binding.map(|binding| {
        if binding.own_constructor == Some(id) {
            binding.fresh_activation()
        } else {
            binding
        }
    });
    let derived = binding
        .as_ref()
        .filter(|binding| binding.constructor.is_some())
        .cloned();
    (binding, derived)
}

impl FunctionSuperBinding {
    pub(super) fn fresh_activation(&self) -> Rc<Self> {
        Rc::new(Self {
            constructor: self.constructor.clone(),
            home_object: self.home_object.clone(),
            own_constructor: self.own_constructor,
            this_value: std::cell::RefCell::new(None),
            allow_direct_eval_super_call: std::cell::Cell::new(
                self.allow_direct_eval_super_call.get(),
            ),
        })
    }
}

/// One resolved instance field initialized during construction.
#[derive(Debug)]
pub(in crate::runtime) enum ResolvedClassField {
    Public {
        key: crate::runtime::object::PropertyKey,
        name: String,
        infer_name: bool,
        initializer: Option<crate::bytecode::BytecodeBlock>,
        decorator_initializers: Rc<[Value]>,
    },
    Private {
        name: PrivateNameId,
        initializer: Option<crate::bytecode::BytecodeBlock>,
        decorator_initializers: Rc<[Value]>,
    },
    AutoAccessor {
        backing_name: PrivateNameId,
        initializer: Option<crate::bytecode::BytecodeBlock>,
        decorator_initializers: Rc<[Value]>,
    },
}

impl ResolvedClassField {
    pub(in crate::runtime) const fn traced_public_key(
        &self,
    ) -> Option<crate::runtime::object::PropertyKey> {
        match self {
            Self::Public { key, .. } => Some(*key),
            Self::Private { .. } | Self::AutoAccessor { .. } => None,
        }
    }

    pub(in crate::runtime) fn decorator_initializers(&self) -> &[Value] {
        match self {
            Self::Public {
                decorator_initializers,
                ..
            }
            | Self::Private {
                decorator_initializers,
                ..
            }
            | Self::AutoAccessor {
                decorator_initializers,
                ..
            } => decorator_initializers,
        }
    }
}

impl Context {
    pub(in crate::runtime) fn retain_function_class_name_environment(
        &mut self,
        id: FunctionId,
        binding: &crate::bytecode::BytecodeBinding,
    ) -> Result<()> {
        let atom = self.intern_static_name_atom(binding.name().name())?;
        let cell = self
            .get_binding_bytecode(binding)?
            .ok_or_else(|| Error::runtime("class name binding disappeared"))?;
        let environment = crate::runtime::activation::EvalBindingEnvironment::default();
        environment.insert(atom, cell, false)?;
        let dynamic = crate::runtime::activation::DynamicEnvironment::CapturedLexical(environment);
        let additional_bindings = dynamic.storage_binding_count()?;
        let mut environments = self.function(id)?.dynamic_environments.to_vec();
        environments.push(dynamic);
        self.storage_ledger
            .grow_count(crate::runtime::VmStorageKind::Binding, additional_bindings)?;
        self.function_mut(id)?.dynamic_environments = environments.into();
        Ok(())
    }

    pub(in crate::runtime) fn set_function_default_derived_constructor(
        &mut self,
        id: FunctionId,
        default_derived: bool,
    ) -> Result<()> {
        if default_derived {
            let function = self.function_mut(id)?;
            if !function.class_constructor.is_class() {
                return Err(Error::runtime(
                    "default derived constructor is not a class constructor",
                ));
            }
            function.class_constructor = FunctionClassConstructor::DefaultDerived;
        }
        Ok(())
    }

    pub(in crate::runtime) fn default_derived_constructor_super(
        &mut self,
        id: FunctionId,
    ) -> Result<Option<Value>> {
        if self.function(id)?.class_constructor != FunctionClassConstructor::DefaultDerived {
            return Ok(None);
        }
        self.function_inheritance_prototype_value(id).map(Some)
    }

    pub(in crate::runtime) fn current_class_field_initializer_context(&self) -> Result<bool> {
        for frame in self.activation_frames.iter().rev() {
            if let Some(context) = frame.class_field_initializer_context() {
                return Ok(context);
            }
            if let Some(id) = frame.function_id() {
                return Ok(self.function(id)?.class_field_initializer_context);
            }
        }
        Ok(false)
    }

    pub(super) fn function_return_mode(
        &self,
        id: FunctionId,
        binding: Option<&Rc<FunctionSuperBinding>>,
    ) -> Result<TailCallReturnMode> {
        if self.function(id)?.class_constructor != FunctionClassConstructor::Explicit {
            return Ok(TailCallReturnMode::Ordinary);
        }
        let Some(binding) = binding.filter(|binding| binding.constructor.is_some()) else {
            return Ok(TailCallReturnMode::Ordinary);
        };
        Ok(TailCallReturnMode::DerivedConstructor {
            this_value: binding.this_value.borrow().clone(),
        })
    }

    pub(super) fn normalize_tail_call_return(
        &self,
        completion: Completion,
        mode: TailCallReturnMode,
    ) -> Result<Completion> {
        match mode {
            TailCallReturnMode::Ordinary => Ok(completion),
            TailCallReturnMode::DerivedConstructor { this_value } => {
                self.normalize_derived_constructor_result(completion, this_value)
            }
        }
    }

    fn normalize_derived_constructor_result(
        &self,
        completion: Completion,
        this_value: Option<Value>,
    ) -> Result<Completion> {
        match completion {
            Completion::Return(value) | Completion::ReturnDirect(value)
                if self.semantic_object_ref(&value)?.is_some() =>
            {
                Ok(Completion::Return(value))
            }
            Completion::Return(Value::Undefined)
            | Completion::ReturnDirect(Value::Undefined)
            | Completion::Normal(_) => this_value.map(Completion::Return).ok_or_else(|| {
                Error::exception(
                    crate::value::ErrorName::ReferenceError,
                    "derived constructor did not initialize this",
                )
            }),
            Completion::Return(_) | Completion::ReturnDirect(_) => Err(Error::type_error(
                "derived constructor can only return an object or undefined",
            )),
            completion => Ok(completion),
        }
    }

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
        name: &PrivateNameId,
    ) -> Result<Option<PrivateSlotValue>> {
        Ok(self
            .function(id)?
            .private_slots
            .iter()
            .find(|slot| slot.id == *name)
            .map(|slot| slot.value.clone()))
    }

    pub(in crate::runtime) fn set_function_private_field(
        &mut self,
        id: FunctionId,
        name: &PrivateNameId,
        value: Value,
    ) -> Result<bool> {
        let Some(slot) = self
            .function_mut(id)?
            .private_slots
            .iter_mut()
            .find(|slot| slot.id == *name)
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
        name: &PrivateNameId,
        value: PrivateSlotValue,
    ) -> Result<()> {
        let slot = self
            .function_mut(id)?
            .private_slots
            .iter_mut()
            .find(|slot| slot.id == *name)
            .ok_or_else(|| Error::runtime("private slot disappeared"))?;
        slot.value = value;
        Ok(())
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
            .map_or(Ok(0), |existing| class_field_storage_count(existing))?;
        let next_count = class_field_storage_count(&fields)?;
        let additional_count = next_count.saturating_sub(previous_count);
        let removed_count = previous_count.saturating_sub(next_count);
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

    pub(in crate::runtime) fn is_base_class_constructor(&self, id: FunctionId) -> bool {
        self.function(id).is_ok_and(|function| {
            function.class_constructor.is_class()
                && function
                    .super_binding
                    .as_ref()
                    .is_none_or(|binding| binding.constructor.is_none())
        })
    }

    pub(in crate::runtime) fn is_class_constructor(&self, id: FunctionId) -> Result<bool> {
        Ok(self.function(id)?.class_constructor.is_class())
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
        let private_environment = self.function(id)?.private_environment.clone();
        if let Some(private_slots) = private_slots {
            for slot in private_slots.iter() {
                self.add_private_slot_to_value(instance, slot.id.clone(), slot.value.clone())?;
            }
        }
        let Some(fields) = fields else {
            return Ok(());
        };
        let constructor_super_binding = self
            .function(id)?
            .super_binding
            .clone()
            .ok_or_else(|| Error::runtime("class field super binding disappeared"))?;
        let super_binding = Rc::new(FunctionSuperBinding {
            constructor: None,
            home_object: constructor_super_binding.home_object.clone(),
            own_constructor: None,
            this_value: std::cell::RefCell::new(None),
            allow_direct_eval_super_call: std::cell::Cell::new(false),
        });
        for field in fields.iter() {
            self.push_class_evaluation(
                instance.clone(),
                super_binding.clone(),
                private_environment.clone(),
                true,
            )?;
            let super_binding = self.current_super_frame();
            let previous_super_call_permission = super_binding
                .as_ref()
                .map(|binding| binding.allow_direct_eval_super_call.replace(false));
            let initializer = match field {
                ResolvedClassField::Public { initializer, .. }
                | ResolvedClassField::Private { initializer, .. }
                | ResolvedClassField::AutoAccessor { initializer, .. } => initializer,
            };
            let value = initializer
                .as_ref()
                .map_or(Ok(Completion::Normal(Value::Undefined)), |initializer| {
                    self.eval_bytecode_block(initializer)
                });
            if let (Some(binding), Some(previous)) =
                (&super_binding, previous_super_call_permission)
            {
                binding.allow_direct_eval_super_call.set(previous);
            }
            self.pop_temporary_this()?;
            let value = value?.into_result()?;
            let mut value = value;
            for initializer in field.decorator_initializers() {
                value = self
                    .semantic_call(initializer, &[value], instance.clone())?
                    .into_result()?;
            }
            match field {
                ResolvedClassField::Public {
                    key,
                    name,
                    infer_name,
                    ..
                } => {
                    if *infer_name {
                        self.set_function_name(&value, name, None)?;
                    }
                    let update = crate::runtime::object::PropertyUpdate::Data(
                        crate::runtime::object::DataPropertyUpdate::new(
                            Some(value.clone()),
                            Some(crate::runtime::object::PropertyWritable::Yes),
                            Some(crate::runtime::object::PropertyEnumerable::Yes),
                            Some(crate::runtime::object::PropertyConfigurable::Yes),
                        ),
                    );
                    let descriptor = crate::runtime::object::DataPropertyDescriptor::new(
                        value,
                        crate::runtime::object::PropertyWritable::Yes,
                        crate::runtime::object::PropertyEnumerable::Yes,
                        crate::runtime::object::PropertyConfigurable::Yes,
                    );
                    let descriptor_value = self.create_property_descriptor_object(
                        &crate::runtime::object::OwnPropertyDescriptor::Data(descriptor),
                    )?;
                    let mut property =
                        crate::runtime::property::DynamicPropertyKey::new(name.clone(), Some(*key));
                    if !self.semantic_define_own_property_update_with_descriptor(
                        instance,
                        &mut property,
                        update,
                        &descriptor_value,
                    )? {
                        return Err(Error::type_error("class field definition was rejected"));
                    }
                }
                ResolvedClassField::Private { name, .. } => {
                    self.add_private_slot_to_value(
                        instance,
                        name.clone(),
                        PrivateSlotValue::Field(value),
                    )?;
                }
                ResolvedClassField::AutoAccessor { backing_name, .. } => {
                    self.add_private_slot_to_value(
                        instance,
                        backing_name.clone(),
                        PrivateSlotValue::Field(value),
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn class_field_storage_count(fields: &[ResolvedClassField]) -> Result<usize> {
    fields.iter().try_fold(fields.len(), |count, field| {
        count
            .checked_add(field.decorator_initializers().len())
            .ok_or_else(|| Error::limit("class field storage count overflowed"))
    })
}
