use std::rc::Rc;

use crate::runtime::private::{PrivateSlot, PrivateSlotValue};
use crate::{
    bytecode::{
        BytecodeAddress, BytecodeClass, BytecodeClassMember, BytecodeClassMemberKey,
        BytecodeClassMemberKind, BytecodeNewTargetMode,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::control::Completion,
    runtime::function::{BytecodeFunctionInit, FunctionSuperBinding, ResolvedClassField},
    runtime::object::{
        AccessorPropertyUpdate, DataPropertyUpdate, PropertyConfigurable, PropertyEnumerable,
        PropertyKey, PropertyUpdate, PropertyWritable,
    },
    value::{FunctionId, ObjectId, Value},
};

use super::state::BytecodeState;

impl Context {
    pub(super) fn eval_bytecode_create_class(
        &mut self,
        state: &mut BytecodeState,
        class: &BytecodeClass,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let member_computed = class
            .members
            .iter()
            .filter(|member| matches!(member.key, BytecodeClassMemberKey::Computed))
            .count();
        let field_computed = class
            .fields
            .iter()
            .filter(|field| matches!(field.key, BytecodeClassMemberKey::Computed))
            .count();
        let mut computed_keys = state
            .stack
            .pop_many(member_computed.saturating_add(field_computed))?;
        let field_computed_keys = computed_keys.split_off(member_computed);
        let heritage = if class.heritage {
            Some(state.stack.pop()?)
        } else {
            None
        };
        let heritage = heritage
            .map(|value| self.resolve_class_heritage(value))
            .transpose()?;

        let constructor = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: class.constructor_id,
            name: class.name.as_ref(),
            bytecode: &class.constructor,
            constructable: true,
            kind: crate::syntax::FunctionKind::Ordinary,
            class_constructor: true,
            prototype_parent: heritage.as_ref().and_then(ClassHeritage::prototype_parent),
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(constructor_id) = &constructor else {
            return Err(Error::runtime("class constructor creation failed"));
        };
        let Some(prototype_id) = self.function_constructor_prototype(*constructor_id)? else {
            return Err(Error::runtime("class prototype object is not available"));
        };
        if let Some(heritage) = &heritage {
            self.set_function_static_parent(*constructor_id, heritage.constructor.clone())?;
            self.set_function_super_binding(
                *constructor_id,
                Rc::new(FunctionSuperBinding {
                    constructor: Some(heritage.constructor.clone()),
                    home_prototype: heritage.prototype.clone(),
                    own_constructor: Some(*constructor_id),
                }),
            )?;
        }
        let instance_home = match &heritage {
            Some(heritage) => heritage.prototype.clone(),
            // Base-class methods resolve super.property through the ordinary
            // object prototype root above the class prototype.
            None => self.objects.prototype_value(prototype_id)?,
        };
        let static_home = heritage
            .as_ref()
            .map_or(Value::Undefined, |heritage| heritage.constructor.clone());

        let targets = ClassInstallationTargets {
            constructor: constructor.clone(),
            constructor_id: *constructor_id,
            prototype_id,
            instance_home,
            static_home,
        };
        self.install_class_members(class, computed_keys, &targets)?;

        self.install_class_fields(class, &constructor, *constructor_id, &field_computed_keys)?;
        self.evaluate_class_static_blocks(class, &constructor)?;

        state.stack.push(constructor);
        state.pc = next;
        Ok(None)
    }

    fn install_class_members(
        &mut self,
        class: &BytecodeClass,
        computed_keys: Vec<Value>,
        targets: &ClassInstallationTargets,
    ) -> Result<()> {
        let mut computed_keys = computed_keys.into_iter();
        let mut instance_private_slots: Vec<PrivateSlot> = Vec::new();
        for member in class.members.iter() {
            let computed_key = match member.key {
                BytecodeClassMemberKey::Computed => Some(
                    computed_keys
                        .next()
                        .ok_or_else(|| Error::runtime("class computed member key disappeared"))?,
                ),
                BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Private { .. } => None,
            };
            let (function_id, private_slot) = self.install_class_member(
                member,
                computed_key.as_ref(),
                targets.constructor_id,
                targets.prototype_id,
                &class.private_names,
            )?;
            if let Some(private_slot) = private_slot {
                if member.is_static {
                    self.add_or_merge_private_slot_to_value(
                        &targets.constructor,
                        private_slot.id,
                        private_slot.value,
                    )?;
                } else if let Some(existing) = instance_private_slots
                    .iter_mut()
                    .find(|slot| slot.id == private_slot.id)
                {
                    existing.value.merge_accessor(private_slot.value)?;
                } else {
                    instance_private_slots.push(private_slot);
                }
            }
            let home = if member.is_static {
                targets.static_home.clone()
            } else {
                targets.instance_home.clone()
            };
            if !matches!(home, Value::Undefined) {
                self.set_function_super_binding(
                    function_id,
                    Rc::new(FunctionSuperBinding {
                        constructor: None,
                        home_prototype: home,
                        own_constructor: None,
                    }),
                )?;
            }
        }
        if !instance_private_slots.is_empty() {
            self.set_function_class_private_slots(
                targets.constructor_id,
                instance_private_slots.into(),
            )?;
        }
        Ok(())
    }

    fn evaluate_class_static_blocks(
        &mut self,
        class: &BytecodeClass,
        constructor: &Value,
    ) -> Result<()> {
        for block in class.static_blocks.iter() {
            self.push_temporary_this(constructor.clone())?;
            let completion = self.eval_bytecode_block(block);
            self.pop_temporary_this()?;
            completion?.into_result()?;
        }
        Ok(())
    }

    /// Resolves field keys once at class definition time, stores instance
    /// fields on the constructor for construction-time initialization, and
    /// evaluates static fields immediately with `this` bound to the
    /// constructor.
    fn install_class_fields(
        &mut self,
        class: &BytecodeClass,
        constructor: &Value,
        constructor_id: FunctionId,
        field_computed_keys: &[Value],
    ) -> Result<()> {
        let mut computed = field_computed_keys.iter();
        let mut instance_fields = Vec::new();
        let mut static_fields = Vec::new();
        for field in class.fields.iter() {
            let computed_key = match field.key {
                BytecodeClassMemberKey::Computed => Some(
                    computed
                        .next()
                        .ok_or_else(|| Error::runtime("class field key disappeared"))?,
                ),
                BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Private { .. } => None,
            };
            let resolved = match &field.key {
                BytecodeClassMemberKey::Private { index } => ResolvedClassField::Private {
                    name: self.resolve_own_private_name(*index)?,
                    initializer: field.initializer.clone(),
                },
                BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Computed => {
                    let (key, name, _) =
                        self.class_member_property_key(&field.key, computed_key)?;
                    ResolvedClassField::Public {
                        key,
                        name,
                        initializer: field.initializer.clone(),
                    }
                }
            };
            if field.is_static {
                static_fields.push(resolved);
            } else {
                instance_fields.push(resolved);
            }
        }
        if !instance_fields.is_empty() {
            self.set_function_class_fields(constructor_id, instance_fields.into())?;
        }
        for field in static_fields {
            self.push_temporary_this(constructor.clone())?;
            let initializer = match &field {
                ResolvedClassField::Public { initializer, .. }
                | ResolvedClassField::Private { initializer, .. } => initializer,
            };
            let value = initializer.as_ref().map_or(
                Ok(crate::runtime::control::Completion::Normal(
                    Value::Undefined,
                )),
                |initializer| self.eval_bytecode_block(initializer),
            );
            self.pop_temporary_this()?;
            let value = value?.into_result()?;
            match field {
                ResolvedClassField::Public { key, name, .. } => {
                    let update = DataPropertyUpdate::new(
                        Some(value),
                        Some(PropertyWritable::Yes),
                        Some(PropertyEnumerable::Yes),
                        Some(PropertyConfigurable::Yes),
                    );
                    self.define_function_property_key(
                        constructor_id,
                        &name,
                        key,
                        PropertyUpdate::Data(update),
                    )?;
                }
                ResolvedClassField::Private { name, .. } => {
                    self.add_private_slot_to_value(
                        constructor,
                        name,
                        PrivateSlotValue::Field(value),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn install_class_member(
        &mut self,
        member: &BytecodeClassMember,
        computed_key: Option<&Value>,
        constructor_id: FunctionId,
        prototype_id: ObjectId,
        private_names: &[crate::syntax::StaticName],
    ) -> Result<(FunctionId, Option<PrivateSlot>)> {
        let function = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: member.id,
            name: None,
            bytecode: &member.bytecode,
            constructable: false,
            kind: member.function_kind,
            class_constructor: false,
            prototype_parent: None,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(function_id) = function.clone() else {
            return Err(Error::runtime("class member creation failed"));
        };

        let prefix = match member.kind {
            BytecodeClassMemberKind::Method => None,
            BytecodeClassMemberKind::Getter => Some(crate::syntax::AccessorKind::Getter),
            BytecodeClassMemberKind::Setter => Some(crate::syntax::AccessorKind::Setter),
        };
        if let BytecodeClassMemberKey::Private { index } = member.key {
            let index_usize = usize::try_from(index)
                .map_err(|_| Error::limit("private name index exceeded supported range"))?;
            let name = private_names
                .get(index_usize)
                .ok_or_else(|| Error::runtime("private class member name disappeared"))?;
            self.set_function_name(&function, name.as_str(), prefix)?;
            let value = match member.kind {
                BytecodeClassMemberKind::Method => PrivateSlotValue::Method(function),
                BytecodeClassMemberKind::Getter => PrivateSlotValue::Accessor {
                    getter: Some(function),
                    setter: None,
                },
                BytecodeClassMemberKind::Setter => PrivateSlotValue::Accessor {
                    getter: None,
                    setter: Some(function),
                },
            };
            return Ok((
                function_id,
                Some(PrivateSlot {
                    id: self.resolve_own_private_name(index)?,
                    value,
                }),
            ));
        }

        let (key, name, function_name) =
            self.class_member_property_key(&member.key, computed_key)?;
        self.set_function_name(&function, &function_name, prefix)?;

        let update = match member.kind {
            BytecodeClassMemberKind::Method => PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(function),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            BytecodeClassMemberKind::Getter => {
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    Some(function),
                    None,
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ))
            }
            BytecodeClassMemberKind::Setter => {
                PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                    None,
                    Some(function),
                    Some(PropertyEnumerable::No),
                    Some(PropertyConfigurable::Yes),
                ))
            }
        };
        if member.is_static {
            self.define_function_property_key(constructor_id, &name, key, update)?;
            return Ok((function_id, None));
        }
        self.objects.define_property(
            prototype_id,
            key,
            &name,
            update,
            self.limits.max_object_properties,
        )?;
        Ok((function_id, None))
    }

    fn class_member_property_key(
        &mut self,
        key: &BytecodeClassMemberKey,
        computed_key: Option<&Value>,
    ) -> Result<(PropertyKey, String, String)> {
        match (key, computed_key) {
            (BytecodeClassMemberKey::Static(name), _) => {
                let name = name.as_str().to_owned();
                Ok((self.intern_property_key(&name)?, name.clone(), name))
            }
            (BytecodeClassMemberKey::Computed, Some(value)) => {
                let mut property = self.dynamic_property_key(value)?;
                let function_name = self.function_name_from_property(&property)?;
                let key = self.intern_dynamic_property_key(&mut property)?;
                Ok((key, property.name().to_owned(), function_name))
            }
            (BytecodeClassMemberKey::Computed, None) => {
                Err(Error::runtime("class computed member key disappeared"))
            }
            (BytecodeClassMemberKey::Private { .. }, _) => {
                Err(Error::runtime("private class element has no property key"))
            }
        }
    }

    fn resolve_class_heritage(&mut self, value: Value) -> Result<ClassHeritage> {
        match &value {
            Value::Null => Ok(ClassHeritage {
                constructor: Value::Undefined,
                prototype: Value::Null,
                prototype_id: None,
            }),
            Value::Function(id) => {
                let prototype_id = self.function_constructor_prototype(*id)?;
                let prototype = prototype_id.map_or(Value::Undefined, Value::Object);
                Ok(ClassHeritage {
                    constructor: value,
                    prototype,
                    prototype_id,
                })
            }
            Value::NativeFunction(_) => {
                let prototype = self.get_named(&value, CLASS_PROTOTYPE_PROPERTY)?;
                let prototype_id = match &prototype {
                    Value::Object(id) => Some(*id),
                    _ => None,
                };
                Ok(ClassHeritage {
                    constructor: value,
                    prototype,
                    prototype_id,
                })
            }
            _ => Err(Error::type_error(format!(
                "class heritage '{value}' is not a constructor"
            ))),
        }
    }
}

const CLASS_PROTOTYPE_PROPERTY: &str = "prototype";

struct ClassInstallationTargets {
    constructor: Value,
    constructor_id: FunctionId,
    prototype_id: ObjectId,
    instance_home: Value,
    static_home: Value,
}

/// Resolved `extends` heritage: the parent constructor value plus its
/// prototype object used as the parent of the class prototype.
struct ClassHeritage {
    constructor: Value,
    prototype: Value,
    prototype_id: Option<crate::value::ObjectId>,
}

impl ClassHeritage {
    const fn prototype_parent(&self) -> Option<crate::value::ObjectId> {
        self.prototype_id
    }
}
