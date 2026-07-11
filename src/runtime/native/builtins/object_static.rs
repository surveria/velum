use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::same_value,
        call::RuntimeCallArgs,
        object::{
            DataPropertyUpdate, OwnPropertyDescriptor, PropertyConfigurable, PropertyEnumerable,
            PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        roots::VmRootKind,
        semantic_object::SemanticIntegrityLevel,
        transient_roots::TransientRootScope,
    },
    value::{ObjectId, Value},
};

struct PendingPropertyUpdate {
    property: DynamicPropertyKey,
    update: PropertyUpdate,
    descriptor: Value,
}

impl Context {
    pub(in crate::runtime::native) fn eval_object_assign(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_assign(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_assign(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = self.object_assign_target(Self::argument_or_undefined(args.first()))?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(std::iter::once(&target))?;
        for source in args.iter().skip(1) {
            self.copy_enumerable_properties(&target, source)?;
        }
        Ok(target)
    }

    pub(in crate::runtime::native) fn eval_object_create(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_create(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_create(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let prototype = Self::object_create_prototype(&Self::argument_or_undefined(args.first()))?;
        let object = self
            .objects
            .create_with_exact_prototype(prototype, self.limits.max_objects)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(std::iter::once(&object).chain(args.get(1)))?;
        if let Some(properties) = args.get(1)
            && !matches!(properties, Value::Undefined)
        {
            self.define_properties_on_target_with_roots(&object, properties, &roots)?;
        }
        Ok(object)
    }

    pub(in crate::runtime::native) fn eval_object_define_properties(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let target = Self::argument_or_undefined(values.first());
        let properties = Self::argument_or_undefined(values.get(1));
        self.define_properties_on_target(&target, &properties)?;
        Ok(target)
    }

    pub(in crate::runtime::native) fn eval_object_entries(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.as_slice().first());
        let keys = self.own_enumerable_keys(&target)?;
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let mut entries = Vec::with_capacity(keys.len());
        for key in keys {
            let key_value = self.heap_string_value(&key)?;
            let value = self.get_named(&target, &key)?;
            let entry = self.create_array_with_prototype(vec![key_value, value], prototype)?;
            entries.push(entry);
        }
        self.create_array_with_prototype(entries, prototype)
    }

    pub(in crate::runtime::native) fn eval_object_freeze(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_freeze(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_freeze(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        if self
            .semantic_set_integrity_level(&target, SemanticIntegrityLevel::Frozen)?
            .is_some_and(|updated| !updated)
        {
            return Err(Error::type_error("Object.freeze could not freeze target"));
        }
        Ok(target)
    }

    pub(in crate::runtime::native) fn eval_object_get_own_property_descriptors(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.as_slice().first());
        let names = self.own_property_names(&target)?;
        let result = self.create_object_from_constructor()?;
        let Value::Object(result_id) = result else {
            return Err(Error::runtime(
                "Object result allocation did not return an object",
            ));
        };
        for name in names {
            let mut property = self.named_dynamic_property(name);
            let Some(descriptor) = self.own_property_descriptor_value(&target, &property)? else {
                continue;
            };
            let descriptor_value = self.create_property_descriptor_object(&descriptor)?;
            self.define_data_property(
                result_id,
                &mut property,
                descriptor_value,
                PropertyEnumerable::Yes,
                PropertyWritable::Yes,
                PropertyConfigurable::Yes,
            )?;
        }
        Ok(Value::Object(result_id))
    }

    pub(in crate::runtime::native) fn eval_direct_object_is(args: &[Value]) -> Value {
        let left = Self::argument_or_undefined(args.first());
        let right = Self::argument_or_undefined(args.get(1));
        Value::Bool(same_value(&left, &right))
    }

    pub(in crate::runtime::native) fn eval_object_is_extensible(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let slice = args.as_slice();
        if let Some(Value::Object(id)) = slice.first()
            && self.objects.is_proxy(*id)
        {
            return Ok(Value::Bool(self.proxy_is_extensible(*id)?));
        }
        self.eval_direct_object_is_extensible(slice)
    }

    pub(in crate::runtime::native) fn eval_direct_object_is_extensible(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        let result = self.semantic_is_extensible(&target)?.unwrap_or(false);
        Ok(Value::Bool(result))
    }

    pub(in crate::runtime::native) fn eval_object_is_frozen(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_is_frozen(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_is_frozen(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        let result = self
            .semantic_test_integrity_level(&target, SemanticIntegrityLevel::Frozen)?
            .unwrap_or(true);
        Ok(Value::Bool(result))
    }

    pub(in crate::runtime::native) fn eval_object_is_sealed(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_is_sealed(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_is_sealed(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        let result = self
            .semantic_test_integrity_level(&target, SemanticIntegrityLevel::Sealed)?
            .unwrap_or(true);
        Ok(Value::Bool(result))
    }

    pub(in crate::runtime::native) fn eval_object_prevent_extensions(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_prevent_extensions(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_prevent_extensions(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        if self
            .semantic_prevent_extensions(&target)?
            .is_some_and(|prevented| !prevented)
        {
            return Err(Error::type_error(
                "Object.preventExtensions trap returned falsy",
            ));
        }
        Ok(target)
    }

    pub(in crate::runtime::native) fn eval_object_seal(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_seal(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_seal(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        if self
            .semantic_set_integrity_level(&target, SemanticIntegrityLevel::Sealed)?
            .is_some_and(|updated| !updated)
        {
            return Err(Error::type_error("Object.seal could not seal target"));
        }
        Ok(target)
    }

    pub(in crate::runtime::native) fn eval_object_set_prototype_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        self.eval_direct_object_set_prototype_of(args.as_slice())
    }

    pub(in crate::runtime::native) fn eval_direct_object_set_prototype_of(
        &mut self,
        args: &[Value],
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.first());
        let prototype = Self::argument_or_undefined(args.get(1));
        Self::validate_prototype_value(&prototype)?;
        match self.semantic_try_set_prototype(&target, prototype)? {
            Some(true) => Ok(target),
            Some(false) => Err(Error::type_error(
                "Object.setPrototypeOf could not update target prototype",
            )),
            None => match target {
                Value::Object(_) => Err(Error::runtime(
                    "Object.setPrototypeOf lost its semantic object target",
                )),
                Value::Undefined | Value::Null => Err(Error::runtime(
                    "Object.setPrototypeOf target cannot be converted to an object",
                )),
                Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => {
                    Err(Error::runtime(
                        "Object.setPrototypeOf target does not support prototype mutation",
                    ))
                }
                Value::Bool(_)
                | Value::Number(_)
                | Value::String(_)
                | Value::HeapString(_)
                | Value::Symbol(_) => Ok(target),
            },
        }
    }

    pub(in crate::runtime::native) fn eval_object_values(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let target = Self::argument_or_undefined(args.as_slice().first());
        let keys = self.own_enumerable_keys(&target)?;
        self.array_constructor_value()?;
        let prototype = self.objects.existing_array_prototype_id()?;
        let mut values = Vec::with_capacity(keys.len());
        for key in keys {
            values.push(self.get_named(&target, &key)?);
        }
        self.create_array_with_prototype(values, prototype)
    }

    fn define_properties_on_target(&mut self, target: &Value, properties: &Value) -> Result<()> {
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values([target, properties])?;
        self.define_properties_on_target_with_roots(target, properties, &roots)
    }

    fn define_properties_on_target_with_roots(
        &mut self,
        target: &Value,
        properties: &Value,
        roots: &TransientRootScope,
    ) -> Result<()> {
        Self::validate_define_properties_target(target)?;
        let updates = self.pending_property_updates(properties, roots)?;
        for PendingPropertyUpdate {
            mut property,
            update,
            descriptor,
        } in updates
        {
            if !self.semantic_define_own_property_update_with_descriptor(
                target,
                &mut property,
                update,
                &descriptor,
            )? {
                return Err(Error::type_error(
                    "proxy defineProperty trap returned falsy",
                ));
            }
        }
        Ok(())
    }

    fn validate_define_properties_target(target: &Value) -> Result<()> {
        match target {
            Value::Object(_) | Value::Function(_) | Value::NativeFunction(_) => Ok(()),
            Value::Undefined
            | Value::Null
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Err(Error::type_error(
                "Object.defineProperties target must be an object",
            )),
            Value::HostFunction(_) => Err(Error::runtime(
                "Object.defineProperties host-function targets are not supported",
            )),
        }
    }

    fn pending_property_updates(
        &mut self,
        properties: &Value,
        roots: &TransientRootScope,
    ) -> Result<Vec<PendingPropertyUpdate>> {
        let keys = self.semantic_own_property_keys(properties)?;
        roots.add_values(keys.iter())?;
        let mut updates = Vec::with_capacity(keys.len());
        for key in keys {
            let property = self.dynamic_property_key(&key)?;
            let Some(descriptor) = self.semantic_own_property_descriptor(properties, &property)?
            else {
                continue;
            };
            let enumerable = match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable(),
                OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable(),
            };
            if !enumerable.is_yes() {
                continue;
            }
            let descriptor_value = self.get(properties, property.lookup())?;
            let update = self.property_update_from_value(&descriptor_value)?;
            roots.add_values(
                std::iter::once(&descriptor_value)
                    .chain(update.trace_values().into_iter().flatten()),
            )?;
            updates.push(PendingPropertyUpdate {
                property,
                update,
                descriptor: descriptor_value,
            });
        }
        Ok(updates)
    }

    fn copy_enumerable_properties(&mut self, target: &Value, source: &Value) -> Result<()> {
        if matches!(source, Value::Undefined | Value::Null) {
            return Ok(());
        }
        let keys = self.semantic_own_property_keys(source)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(keys.iter())?;
        for key in keys {
            let mut property = self.dynamic_property_key(&key)?;
            let Some(descriptor) = self.semantic_own_property_descriptor(source, &property)? else {
                continue;
            };
            let enumerable = match descriptor {
                OwnPropertyDescriptor::Data(descriptor) => descriptor.enumerable(),
                OwnPropertyDescriptor::Accessor(descriptor) => descriptor.enumerable(),
            };
            if !enumerable.is_yes() {
                continue;
            }
            let value = self.get(source, property.lookup())?;
            let _value_scope =
                self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&value))?;
            let updated = self
                .semantic_reflect_property_write(target, &mut property, value, target)?
                .unwrap_or(false);
            if !updated {
                return Err(Error::type_error(format!(
                    "Object.assign could not set property '{}'",
                    property.name()
                )));
            }
        }
        Ok(())
    }

    fn object_assign_target(&mut self, value: Value) -> Result<Value> {
        match value {
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Ok(value),
            Value::Undefined | Value::Null => Err(Error::type_error(
                "Object.assign target cannot be converted to an object",
            )),
            Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => {
                let args = [value];
                self.eval_direct_object_constructor(&args)
            }
        }
    }

    fn object_create_prototype(value: &Value) -> Result<Option<ObjectId>> {
        match value {
            Value::Object(id) => Ok(Some(*id)),
            Value::Null => Ok(None),
            Value::Undefined
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_) => Err(Error::type_error(
                "Object.create prototype must be an object or null",
            )),
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => Err(
                Error::runtime("Object.create callable prototypes are not supported"),
            ),
        }
    }

    fn validate_prototype_value(value: &Value) -> Result<()> {
        match value {
            Value::Object(_) | Value::Null => Ok(()),
            Value::Undefined
            | Value::Bool(_)
            | Value::Number(_)
            | Value::String(_)
            | Value::HeapString(_)
            | Value::Symbol(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Err(Error::runtime(
                "Object.setPrototypeOf prototype must be an object or null",
            )),
        }
    }

    fn define_data_property(
        &mut self,
        id: ObjectId,
        property: &mut DynamicPropertyKey,
        value: Value,
        enumerable: PropertyEnumerable,
        writable: PropertyWritable,
        configurable: PropertyConfigurable,
    ) -> Result<()> {
        let key = self.intern_dynamic_property_key(property)?;
        self.objects.define_property(
            id,
            key,
            property.name(),
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(writable),
                Some(enumerable),
                Some(configurable),
            )),
            self.limits.max_object_properties,
        )
    }

    fn create_array_with_prototype(
        &mut self,
        elements: Vec<Value>,
        prototype: ObjectId,
    ) -> Result<Value> {
        self.objects.create_array(
            elements,
            prototype,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn named_dynamic_property(&self, name: String) -> DynamicPropertyKey {
        let key = self.known_property_key(&name);
        DynamicPropertyKey::new(name, key)
    }
}
