use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::SetFailureBehavior,
        call::RuntimeCallArgs,
        native::NativeFunctionKind,
        object::{
            AccessorPropertyUpdate, DataPropertyDescriptor, DataPropertyUpdate,
            OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable, PropertyUpdate,
            PropertyWritable,
        },
        property::DynamicPropertyKey,
        roots::VmRootKind,
    },
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

const ERROR_IS_ERROR_PROPERTY: &str = "isError";
const ERROR_STACK_PROPERTY: &str = "stack";
const ERROR_TO_STRING_PROPERTY: &str = "toString";

impl Context {
    pub(in crate::runtime) fn eval_error_native_function_kind(
        &mut self,
        kind: NativeFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            NativeFunctionKind::ErrorConstructor(name) => {
                Some(self.eval_error_constructor(name, args))
            }
            NativeFunctionKind::ErrorIsError => Some(self.eval_error_is_error(args)),
            NativeFunctionKind::ErrorPrototypeToString => {
                Some(self.eval_error_prototype_to_string(args, this_value))
            }
            NativeFunctionKind::ErrorStackGetter => Some(self.eval_error_stack_getter(this_value)),
            NativeFunctionKind::ErrorStackSetter => {
                Some(self.eval_error_stack_setter(args, this_value))
            }
            _ => None,
        }
    }

    pub(in crate::runtime) fn install_error_surface(
        &mut self,
        constructor: NativeFunctionId,
        prototype: ObjectId,
    ) -> Result<()> {
        let is_error = self
            .create_ephemeral_native_function(NativeFunctionKind::ErrorIsError, Value::Undefined)?;
        let key = self.intern_property_key(ERROR_IS_ERROR_PROPERTY)?;
        self.native_function_mut(constructor)?
            .properties_mut()
            .define_builtin(key, is_error, PropertyEnumerable::No)?;

        let to_string = self.create_ephemeral_native_function(
            NativeFunctionKind::ErrorPrototypeToString,
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype, ERROR_TO_STRING_PROPERTY, to_string)?;

        let getter = self.create_ephemeral_native_function(
            NativeFunctionKind::ErrorStackGetter,
            Value::Undefined,
        )?;
        let setter = self.create_ephemeral_native_function(
            NativeFunctionKind::ErrorStackSetter,
            Value::Undefined,
        )?;
        let key = self.intern_property_key(ERROR_STACK_PROPERTY)?;
        self.objects.define_property(
            prototype,
            key,
            ERROR_STACK_PROPERTY,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                Some(setter),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime) fn eval_error_is_error(
        &self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let Some(Value::Object(id)) = args.as_slice().first() else {
            return Ok(Value::Bool(false));
        };
        Ok(Value::Bool(self.objects.error_metadata(*id)?.is_some()))
    }

    pub(in crate::runtime) fn eval_error_stack_getter(
        &mut self,
        this_value: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(
                "Error.prototype.stack getter receiver must be an object",
            ));
        }
        let Value::Object(id) = this_value else {
            return Ok(Value::Undefined);
        };
        let Some(metadata) = self.objects.error_metadata(*id)? else {
            return Ok(Value::Undefined);
        };
        let stack = metadata.to_string();
        self.heap_string_value(&stack)
    }

    pub(in crate::runtime) fn eval_error_stack_setter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if self.semantic_object_ref(this_value)?.is_none() {
            return Err(Error::type_error(
                "Error.prototype.stack setter receiver must be an object",
            ));
        }
        let value = args.as_slice().first().cloned().unwrap_or(Value::Undefined);
        if !matches!(value, Value::String(_)) {
            return Err(Error::type_error(
                "Error.prototype.stack setter value must be a string",
            ));
        }
        let home = self.error_constructor_prototype(ErrorName::Base)?;
        if matches!(this_value, Value::Object(id) if *id == home) {
            return Err(Error::type_error(
                "Error.prototype.stack cannot be set on Error.prototype",
            ));
        }

        let _scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [this_value, &value])?;
        let mut property = DynamicPropertyKey::new(ERROR_STACK_PROPERTY.to_owned(), None);
        let _key = self.intern_dynamic_property_key(&mut property)?;
        if let Some(descriptor) = self.semantic_own_property_descriptor(this_value, &property)? {
            if !matches!(this_value, Value::Object(id) if self.objects.is_proxy(*id)) {
                self.set_existing_error_stack_property(
                    this_value,
                    &mut property,
                    value,
                    descriptor,
                )?;
                return Ok(Value::Undefined);
            }
            if !self.set(
                this_value,
                property.lookup(),
                value,
                this_value,
                SetFailureBehavior::Throw,
            )? {
                return Err(Error::type_error("Error stack property update failed"));
            }
            return Ok(Value::Undefined);
        }

        self.create_error_stack_data_property(this_value, &mut property, value)?;
        Ok(Value::Undefined)
    }

    fn set_existing_error_stack_property(
        &mut self,
        target: &Value,
        property: &mut DynamicPropertyKey,
        value: Value,
        descriptor: OwnPropertyDescriptor,
    ) -> Result<()> {
        match descriptor {
            OwnPropertyDescriptor::Data(descriptor) => {
                if !descriptor.writable().is_yes() {
                    return Err(Error::type_error("Error stack property is not writable"));
                }
                let update = PropertyUpdate::Data(DataPropertyUpdate::new(
                    Some(value),
                    Some(descriptor.writable()),
                    Some(descriptor.enumerable()),
                    Some(descriptor.configurable()),
                ));
                if !self.semantic_define_own_property_update(target, property, update)? {
                    return Err(Error::type_error("Error stack property update failed"));
                }
            }
            OwnPropertyDescriptor::Accessor(descriptor) => {
                if !descriptor.has_setter() {
                    return Err(Error::type_error("Error stack property has no setter"));
                }
                self.call_accessor_function(&descriptor.set(), target.clone(), &[value])?;
            }
        }
        Ok(())
    }

    fn create_error_stack_data_property(
        &mut self,
        target: &Value,
        property: &mut DynamicPropertyKey,
        value: Value,
    ) -> Result<()> {
        let descriptor = DataPropertyDescriptor::new(
            value.clone(),
            PropertyWritable::Yes,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        );
        let descriptor_value =
            self.create_property_descriptor_object(&OwnPropertyDescriptor::Data(descriptor))?;
        let _scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, [&descriptor_value])?;
        let update = PropertyUpdate::Data(DataPropertyUpdate::new(
            Some(value),
            Some(PropertyWritable::Yes),
            Some(PropertyEnumerable::Yes),
            Some(PropertyConfigurable::Yes),
        ));
        if !self.semantic_define_own_property_update_with_descriptor(
            target,
            property,
            update,
            &descriptor_value,
        )? {
            return Err(Error::type_error("Error stack property creation failed"));
        }
        Ok(())
    }
}
