use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeClass, BytecodeClassMember, BytecodeClassMemberKey,
        BytecodeClassMemberKind, BytecodeNewTargetMode,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::control::Completion,
    runtime::function::{BytecodeFunctionInit, FunctionSuperBinding},
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
        let computed_count = class
            .members
            .iter()
            .filter(|member| matches!(member.key, BytecodeClassMemberKey::Computed))
            .count();
        let computed_keys = state.stack.pop_many(computed_count)?;
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
            is_async: false,
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

        let mut computed_keys = computed_keys.into_iter();
        for member in class.members.iter() {
            let computed_key = match member.key {
                BytecodeClassMemberKey::Computed => Some(
                    computed_keys
                        .next()
                        .ok_or_else(|| Error::runtime("class computed member key disappeared"))?,
                ),
                BytecodeClassMemberKey::Static(_) => None,
            };
            let function_id = self.install_class_member(
                member,
                computed_key.as_ref(),
                *constructor_id,
                prototype_id,
            )?;
            let home = if member.is_static {
                static_home.clone()
            } else {
                instance_home.clone()
            };
            if !matches!(home, Value::Undefined) {
                self.set_function_super_binding(
                    function_id,
                    Rc::new(FunctionSuperBinding {
                        constructor: None,
                        home_prototype: home,
                    }),
                )?;
            }
        }

        state.stack.push(constructor);
        state.pc = next;
        Ok(None)
    }

    fn install_class_member(
        &mut self,
        member: &BytecodeClassMember,
        computed_key: Option<&Value>,
        constructor_id: FunctionId,
        prototype_id: ObjectId,
    ) -> Result<FunctionId> {
        let function = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: member.id,
            name: member.name.as_ref(),
            bytecode: &member.bytecode,
            constructable: false,
            is_async: false,
            class_constructor: false,
            prototype_parent: None,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(function_id) = function.clone() else {
            return Err(Error::runtime("class member creation failed"));
        };

        let (key, name) = self.class_member_property_key(&member.key, computed_key)?;
        if computed_key.is_some() {
            self.set_computed_method_name(&function, &name)?;
        }

        if member.is_static {
            if member.kind != BytecodeClassMemberKind::Method {
                return Err(Error::runtime(
                    "class static accessors are not supported yet",
                ));
            }
            let update = DataPropertyUpdate::new(
                Some(function),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            );
            self.define_function_property_key(constructor_id, &name, key, update)?;
            return Ok(function_id);
        }

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
        self.objects.define_property(
            prototype_id,
            key,
            &name,
            update,
            self.limits.max_object_properties,
        )?;
        Ok(function_id)
    }

    fn class_member_property_key(
        &mut self,
        key: &BytecodeClassMemberKey,
        computed_key: Option<&Value>,
    ) -> Result<(PropertyKey, String)> {
        match (key, computed_key) {
            (BytecodeClassMemberKey::Static(name), _) => Ok((
                self.intern_property_key(name.as_str())?,
                name.as_str().to_owned(),
            )),
            (BytecodeClassMemberKey::Computed, Some(value)) => {
                let mut property = self.dynamic_property_key(value)?;
                let key = self.intern_dynamic_property_key(&mut property)?;
                Ok((key, property.name().to_owned()))
            }
            (BytecodeClassMemberKey::Computed, None) => {
                Err(Error::runtime("class computed member key disappeared"))
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
                let prototype = self.get_property_value(&value, CLASS_PROTOTYPE_PROPERTY)?;
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
