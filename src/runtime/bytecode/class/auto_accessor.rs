use alloc::rc::Rc;

use crate::{
    bytecode::{
        BytecodeClass, BytecodeClassAutoAccessor, BytecodeClassField, BytecodeClassMemberKey,
        BytecodeFunction, BytecodeNewTargetMode,
    },
    error::{Error, Result},
    runtime::{
        Context,
        function::{BytecodeFunctionInit, FunctionSuperBinding, ResolvedClassField},
        object::{
            AccessorPropertyUpdate, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate,
        },
        private::PrivateNameId,
    },
    syntax::AccessorKind,
    value::{FunctionId, Value},
};

use super::ClassInstallationTargets;

pub(super) struct AutoAccessorProperty {
    pub(super) key: PropertyKey,
    pub(super) name: String,
    pub(super) function_name: String,
}

impl Context {
    pub(super) fn resolve_class_field(
        &mut self,
        class: &BytecodeClass,
        field: &BytecodeClassField,
        decorators: Vec<Value>,
        computed_key: Option<&Value>,
        targets: &ClassInstallationTargets,
    ) -> Result<ResolvedClassField> {
        let public_property = match &field.key {
            BytecodeClassMemberKey::Private { .. } => None,
            BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Computed => {
                let (key, name, function_name) =
                    self.class_member_property_key(&field.key, computed_key)?;
                Some(AutoAccessorProperty {
                    key,
                    name,
                    function_name,
                })
            }
        };
        let (name, is_private) = if let Some(property) = &public_property {
            (property.name.clone(), false)
        } else {
            let BytecodeClassMemberKey::Private { index } = field.key else {
                return Err(Error::runtime("class field property resolution failed"));
            };
            let index = usize::try_from(index)
                .map_err(|_| Error::limit("private name index exceeded supported range"))?;
            let name = class
                .private_names
                .get(index)
                .ok_or_else(|| Error::runtime("private class field name disappeared"))?;
            (name.as_str().to_owned(), true)
        };
        let decorator_initializers = self.apply_field_decorators(
            decorators,
            if field.auto_accessor.is_some() {
                "accessor"
            } else {
                "field"
            },
            &name,
            field.is_static,
            is_private,
        )?;
        if let Some(accessor) = &field.auto_accessor {
            let property = public_property
                .as_ref()
                .ok_or_else(|| Error::runtime("public auto-accessor property key disappeared"))?;
            let backing_name =
                self.install_class_auto_accessor(accessor, field.is_static, property, targets)?;
            return Ok(ResolvedClassField::AutoAccessor {
                backing_name,
                initializer: field.initializer.clone(),
                decorator_initializers: decorator_initializers.into(),
            });
        }
        match &field.key {
            BytecodeClassMemberKey::Private { index } => Ok(ResolvedClassField::Private {
                name: self.resolve_own_private_name(*index)?,
                initializer: field.initializer.clone(),
                decorator_initializers: decorator_initializers.into(),
            }),
            BytecodeClassMemberKey::Static(_) | BytecodeClassMemberKey::Computed => {
                let property = public_property
                    .ok_or_else(|| Error::runtime("public class field property key disappeared"))?;
                Ok(ResolvedClassField::Public {
                    key: property.key,
                    name: property.name,
                    infer_name: field.infer_name_from_computed_key,
                    initializer: field.initializer.clone(),
                    decorator_initializers: decorator_initializers.into(),
                })
            }
        }
    }

    pub(super) fn install_class_auto_accessor(
        &mut self,
        accessor: &BytecodeClassAutoAccessor,
        is_static: bool,
        property: &AutoAccessorProperty,
        targets: &ClassInstallationTargets,
    ) -> Result<PrivateNameId> {
        let home_object = if is_static {
            targets.constructor.clone()
        } else {
            Value::Object(targets.prototype_id)
        };
        let getter = self.create_auto_accessor_function(
            accessor.getter_id,
            &accessor.getter,
            &property.function_name,
            AccessorKind::Getter,
            &home_object,
        )?;
        let setter = self.create_auto_accessor_function(
            accessor.setter_id,
            &accessor.setter,
            &property.function_name,
            AccessorKind::Setter,
            &home_object,
        )?;
        let update = PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
            Some(getter),
            Some(setter),
            Some(PropertyEnumerable::No),
            Some(PropertyConfigurable::Yes),
        ));
        if is_static {
            self.define_function_property_key(
                targets.constructor_id,
                &property.name,
                property.key,
                update,
            )?;
        } else {
            self.objects.define_property(
                targets.prototype_id,
                property.key,
                &property.name,
                update,
                self.limits.max_object_properties,
            )?;
        }
        self.resolve_own_private_name(accessor.backing_name_index)
    }

    fn create_auto_accessor_function(
        &mut self,
        id: crate::syntax::StaticFunctionId,
        bytecode: &BytecodeFunction,
        name: &str,
        prefix: AccessorKind,
        home_object: &Value,
    ) -> Result<Value> {
        let function = self.create_bytecode_function(&BytecodeFunctionInit {
            static_function_id: id,
            name: None,
            bytecode,
            constructable: false,
            kind: crate::syntax::FunctionKind::Ordinary,
            class_constructor: false,
            prototype_parent: None,
            new_target_mode: BytecodeNewTargetMode::Own,
        })?;
        let Value::Function(function_id) = function.clone() else {
            return Err(Error::runtime("auto-accessor function creation failed"));
        };
        self.set_function_name(&function, name, Some(prefix))?;
        self.set_auto_accessor_super_binding(function_id, home_object)?;
        Ok(function)
    }

    fn set_auto_accessor_super_binding(
        &mut self,
        function_id: FunctionId,
        home_object: &Value,
    ) -> Result<()> {
        self.set_function_super_binding(
            function_id,
            Rc::new(FunctionSuperBinding {
                constructor: None,
                home_object: home_object.clone(),
                own_constructor: None,
                this_value: core::cell::RefCell::new(None),
                allow_direct_eval_super_call: core::cell::Cell::new(false),
            }),
        )
    }
}
