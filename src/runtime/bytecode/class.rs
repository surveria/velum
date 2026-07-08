use crate::{
    bytecode::{
        BytecodeAddress, BytecodeClass, BytecodeClassMember, BytecodeClassMemberKey,
        BytecodeClassMemberKind, BytecodeNewTargetMode,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::completion::Completion,
    runtime::function::BytecodeFunctionInit,
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

        let constructor = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: class.constructor_id,
            name: class.name.as_ref(),
            bytecode: &class.constructor,
            constructable: true,
            is_async: false,
            class_constructor: true,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(constructor_id) = &constructor else {
            return Err(Error::runtime("class constructor creation failed"));
        };
        let Some(prototype_id) = self.function_constructor_prototype(*constructor_id)? else {
            return Err(Error::runtime("class prototype object is not available"));
        };

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
            self.install_class_member(
                member,
                computed_key.as_ref(),
                *constructor_id,
                prototype_id,
            )?;
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
    ) -> Result<()> {
        let function = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: member.id,
            name: member.name.as_ref(),
            bytecode: &member.bytecode,
            constructable: false,
            is_async: false,
            class_constructor: false,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;

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
            return self.define_function_property_key(constructor_id, &name, key, update);
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
        )
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
}
