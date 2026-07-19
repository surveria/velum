#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        control::Completion,
        object::{
            AccessorPropertyDescriptor, AccessorPropertyUpdate, DataPropertyUpdate,
            ObjectPrimitiveValue, ObjectPropertyInit, OwnPropertyDescriptor, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyUpdate, PropertyWritable,
        },
        property::DynamicPropertyKey,
        roots::VmRootKind,
    },
    value::{ObjectId, Value},
};

use super::{
    OBJECT_PROTOTYPE_DEFINE_GETTER_NAME, OBJECT_PROTOTYPE_DEFINE_SETTER_NAME,
    OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME, OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME,
    OBJECT_PROTOTYPE_LOOKUP_GETTER_NAME, OBJECT_PROTOTYPE_LOOKUP_SETTER_NAME,
    OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME, OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME,
    OBJECT_PROTOTYPE_TO_STRING_NAME, OBJECT_PROTOTYPE_VALUE_OF_NAME,
};

const OBJECT_UNDEFINED_TAG: &str = "[object Undefined]";
const OBJECT_NULL_TAG: &str = "[object Null]";
const TAG_ARRAY: &str = "Array";
const TAG_ARGUMENTS: &str = "Arguments";
const TAG_FUNCTION: &str = "Function";
const TAG_ERROR: &str = "Error";
const TAG_BOOLEAN: &str = "Boolean";
const TAG_NUMBER: &str = "Number";
const TAG_STRING: &str = "String";
const TAG_DATE: &str = "Date";
const TAG_REGEXP: &str = "RegExp";
const TAG_OBJECT: &str = "Object";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const TO_STRING_TAG_DISPLAY: &str = "Symbol(Symbol.toStringTag)";
const TO_LOCALE_STRING_METHOD: &str = "toString";
const ENTRY_KEY_PROPERTY: &str = "0";
const ENTRY_VALUE_PROPERTY: &str = "1";
const ENTRY_NOT_OBJECT_ERROR: &str = "Object.fromEntries iterator value must be an object";
const OBJECT_RECEIVER_ERROR: &str = "Object.prototype method called on null or undefined";
const LEGACY_PROTO_PROPERTY: &str = "__proto__";

#[derive(Clone, Copy)]
enum LegacyAccessorKind {
    Getter,
    Setter,
}

impl Context {
    pub(in crate::runtime) fn ensure_object_prototype_intrinsic_for_ordinary_lookup(
        &mut self,
        object: ObjectId,
        property: &str,
    ) -> Result<()> {
        if !Self::object_prototype_intrinsic_property(property)
            || self
                .native_function_id(crate::runtime::native::NativeFunctionKind::Object)
                .is_some()
        {
            return Ok(());
        }
        let Some(root) = self.objects.object_prototype else {
            return Ok(());
        };
        if object != root && !self.objects.prototype_chain_has_object(object, root)? {
            return Ok(());
        }
        self.object_constructor_value().map(|_| ())
    }

    fn object_prototype_intrinsic_property(property: &str) -> bool {
        matches!(
            property,
            LEGACY_PROTO_PROPERTY
                | OBJECT_PROTOTYPE_DEFINE_GETTER_NAME
                | OBJECT_PROTOTYPE_DEFINE_SETTER_NAME
                | OBJECT_PROTOTYPE_HAS_OWN_PROPERTY_NAME
                | OBJECT_PROTOTYPE_IS_PROTOTYPE_OF_NAME
                | OBJECT_PROTOTYPE_LOOKUP_GETTER_NAME
                | OBJECT_PROTOTYPE_LOOKUP_SETTER_NAME
                | OBJECT_PROTOTYPE_PROPERTY_IS_ENUMERABLE_NAME
                | OBJECT_PROTOTYPE_TO_LOCALE_STRING_NAME
                | OBJECT_PROTOTYPE_TO_STRING_NAME
                | OBJECT_PROTOTYPE_VALUE_OF_NAME
        )
    }

    pub(in crate::runtime::native) fn eval_object_prototype_to_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let builtin = match this_value {
            Value::Undefined => return self.heap_string_value(OBJECT_UNDEFINED_TAG),
            Value::Null => return self.heap_string_value(OBJECT_NULL_TAG),
            Value::Object(id) => self.object_builtin_class(*id)?,
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => TAG_FUNCTION,
            Value::Bool(_) => TAG_BOOLEAN,
            Value::Number(_) => TAG_NUMBER,
            Value::BigInt(_) | Value::Symbol(_) => TAG_OBJECT,
            Value::String(_) => TAG_STRING,
        };
        let object = self.object_to_object(this_value)?;
        let builtin = if self.semantic_is_array(&object)? {
            TAG_ARRAY
        } else if self.semantic_is_callable(&object)? {
            TAG_FUNCTION
        } else {
            builtin
        };
        let tag = self.object_builtin_tag(&object, builtin)?;
        self.heap_string_value(&format!("[object {tag}]"))
    }

    pub(in crate::runtime::native) fn eval_object_prototype_value_of(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.object_to_object(this_value)
    }

    pub(in crate::runtime::native) fn eval_object_prototype_define_getter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_object_prototype_define_accessor(args, this_value, LegacyAccessorKind::Getter)
    }

    pub(in crate::runtime::native) fn eval_object_prototype_define_setter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_object_prototype_define_accessor(args, this_value, LegacyAccessorKind::Setter)
    }

    fn eval_object_prototype_define_accessor(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        kind: LegacyAccessorKind,
    ) -> Result<Value> {
        let object = self.object_to_object(this_value)?;
        let accessor = Self::argument_or_undefined(args.as_slice().get(1));
        if !self.semantic_is_callable(&accessor)? {
            return Err(Error::type_error(
                "legacy property accessor must be callable",
            ));
        }
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values([&object, &accessor])?;
        let property_value = Self::argument_or_undefined(args.as_slice().first());
        let mut property = self.dynamic_property_key(&property_value)?;
        let (get, set, update) = match kind {
            LegacyAccessorKind::Getter => (
                accessor.clone(),
                Value::Undefined,
                AccessorPropertyUpdate::new(
                    Some(accessor),
                    None,
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                ),
            ),
            LegacyAccessorKind::Setter => (
                Value::Undefined,
                accessor.clone(),
                AccessorPropertyUpdate::new(
                    None,
                    Some(accessor),
                    Some(PropertyEnumerable::Yes),
                    Some(PropertyConfigurable::Yes),
                ),
            ),
        };
        let descriptor = OwnPropertyDescriptor::Accessor(AccessorPropertyDescriptor::new(
            get,
            set,
            PropertyEnumerable::Yes,
            PropertyConfigurable::Yes,
        ));
        let descriptor_value = self.create_legacy_accessor_descriptor_object(&descriptor, kind)?;
        roots.add_values(core::iter::once(&descriptor_value))?;
        if !self.semantic_define_own_property_update_with_descriptor(
            &object,
            &mut property,
            PropertyUpdate::Accessor(update),
            &descriptor_value,
        )? {
            return Err(Error::type_error(
                "legacy accessor property definition failed",
            ));
        }
        Ok(Value::Undefined)
    }

    fn create_legacy_accessor_descriptor_object(
        &mut self,
        descriptor: &OwnPropertyDescriptor,
        kind: LegacyAccessorKind,
    ) -> Result<Value> {
        let OwnPropertyDescriptor::Accessor(descriptor) = descriptor else {
            return Err(Error::runtime(
                "legacy accessor descriptor must be an accessor",
            ));
        };
        let (accessor_name, accessor_value) = match kind {
            LegacyAccessorKind::Getter => ("get", descriptor.get()),
            LegacyAccessorKind::Setter => ("set", descriptor.set()),
        };
        let properties = vec![
            ObjectPropertyInit::new(
                self.intern_property_key(accessor_name)?,
                accessor_name,
                accessor_value,
                PropertyEnumerable::Yes,
            ),
            ObjectPropertyInit::new(
                self.intern_property_key("enumerable")?,
                "enumerable",
                Value::Bool(true),
                PropertyEnumerable::Yes,
            ),
            ObjectPropertyInit::new(
                self.intern_property_key("configurable")?,
                "configurable",
                Value::Bool(true),
                PropertyEnumerable::Yes,
            ),
        ];
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_data_object(
            properties,
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    pub(in crate::runtime::native) fn eval_object_prototype_lookup_getter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_object_prototype_lookup_accessor(args, this_value, LegacyAccessorKind::Getter)
    }

    pub(in crate::runtime::native) fn eval_object_prototype_lookup_setter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_object_prototype_lookup_accessor(args, this_value, LegacyAccessorKind::Setter)
    }

    fn eval_object_prototype_lookup_accessor(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        kind: LegacyAccessorKind,
    ) -> Result<Value> {
        let mut current = self.object_to_object(this_value)?;
        let roots = self.active_transient_root_scope(VmRootKind::TransientTemporary)?;
        roots.add_values(core::iter::once(&current))?;
        let property_value = Self::argument_or_undefined(args.as_slice().first());
        let property = self.dynamic_property_key(&property_value)?;
        loop {
            self.step()?;
            if let Some(descriptor) = self.semantic_own_property_descriptor(&current, &property)? {
                return Ok(match (kind, descriptor) {
                    (LegacyAccessorKind::Getter, OwnPropertyDescriptor::Accessor(descriptor)) => {
                        descriptor.get()
                    }
                    (LegacyAccessorKind::Setter, OwnPropertyDescriptor::Accessor(descriptor)) => {
                        descriptor.set()
                    }
                    (_, OwnPropertyDescriptor::Data(_)) => Value::Undefined,
                });
            }
            let Some(prototype) = self.semantic_get_prototype(&current)? else {
                return Ok(Value::Undefined);
            };
            if matches!(prototype, Value::Null) {
                return Ok(Value::Undefined);
            }
            current = prototype;
            roots.add_values(core::iter::once(&current))?;
        }
    }

    pub(in crate::runtime::native) fn eval_object_prototype_proto_getter(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let object = self.object_to_object(this_value)?;
        self.semantic_get_prototype(&object)?
            .ok_or_else(|| Error::runtime("Object prototype getter requires an object"))
    }

    pub(in crate::runtime::native) fn eval_object_prototype_proto_setter(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "Object prototype setter called on null or undefined",
            ));
        }
        let prototype = Self::argument_or_undefined(args.as_slice().first());
        if !matches!(prototype, Value::Null) && self.semantic_object_ref(&prototype)?.is_none() {
            return Ok(Value::Undefined);
        }
        let Some(updated) = self.semantic_try_set_prototype(this_value, prototype)? else {
            return Ok(Value::Undefined);
        };
        if !updated {
            return Err(Error::type_error("Object prototype mutation was rejected"));
        }
        Ok(Value::Undefined)
    }

    pub(in crate::runtime::native) fn eval_object_prototype_to_locale_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let method = self
            .get_named_method(this_value, TO_LOCALE_STRING_METHOD)?
            .ok_or_else(|| Error::type_error("Object toString method is missing"))?;
        match self.call(&method, &[], this_value.clone())? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    pub(in crate::runtime::native) fn eval_object_prototype_is_prototype_of(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let Some(mut current) = args.as_slice().first().cloned() else {
            return Ok(Value::Bool(false));
        };
        if self.semantic_object_ref(&current)?.is_none() {
            return Ok(Value::Bool(false));
        }
        let receiver = self.object_to_object(this_value)?;
        loop {
            self.step()?;
            let Some(prototype) = self.semantic_get_prototype(&current)? else {
                return Ok(Value::Bool(false));
            };
            if prototype == receiver {
                return Ok(Value::Bool(true));
            }
            if matches!(prototype, Value::Null) {
                return Ok(Value::Bool(false));
            }
            current = prototype;
        }
    }

    pub(in crate::runtime::native) fn eval_object_from_entries(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let iterable = Self::argument_or_undefined(args.as_slice().first());
        if matches!(iterable, Value::Undefined | Value::Null) {
            return Err(Error::type_error(
                "Object.fromEntries requires an iterable argument",
            ));
        }
        let result = self.create_object_from_constructor()?;
        let mut source = self.get_iterator(&iterable)?;
        loop {
            self.step()?;
            match self.iterator_step(&mut source)? {
                crate::runtime::abstract_operations::IteratorStep::Value(entry) => {
                    if let Err(error) = self.add_object_from_entry(&result, &entry) {
                        return Err(self.iterator_close_on_error(&mut source, error));
                    }
                }
                crate::runtime::abstract_operations::IteratorStep::Done => break,
                crate::runtime::abstract_operations::IteratorStep::Abrupt(completion) => {
                    return completion.into_result();
                }
            }
        }
        Ok(result)
    }

    fn add_object_from_entry(&mut self, result: &Value, entry: &Value) -> Result<()> {
        if self.semantic_object_ref(entry)?.is_none() {
            return Err(Error::type_error(ENTRY_NOT_OBJECT_ERROR));
        }
        let key = self.get_named(entry, ENTRY_KEY_PROPERTY)?;
        let value = self.get_named(entry, ENTRY_VALUE_PROPERTY)?;
        let mut dynamic = self.object_property_key(Some(&key))?;
        let updated = self.semantic_define_own_property_update(
            result,
            &mut dynamic,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::Yes),
                Some(PropertyConfigurable::Yes),
            )),
        )?;
        if !updated {
            return Err(Error::type_error(
                "Object.fromEntries could not define result property",
            ));
        }
        Ok(())
    }

    /// Coerce a receiver to an object (`ToObject`), boxing primitives.
    pub(in crate::runtime) fn object_to_object(&mut self, this_value: &Value) -> Result<Value> {
        match this_value {
            Value::Object(_)
            | Value::Function(_)
            | Value::NativeFunction(_)
            | Value::HostFunction(_) => Ok(this_value.clone()),
            Value::Bool(value) => self.create_boolean_object_from_value(*value),
            Value::Number(value) => self.create_number_object_from_value(*value),
            Value::BigInt(value) => self.create_bigint_object_from_value(value.clone()),
            Value::String(_) => self.create_string_object_from_value(this_value),
            Value::Symbol(value) => self.create_symbol_object_from_value(value.clone()),
            Value::Undefined | Value::Null => Err(Error::type_error(OBJECT_RECEIVER_ERROR)),
        }
    }

    /// Determine the `Object.prototype.toString` tag for an object, honoring a
    /// string-valued `Symbol.toStringTag` over the builtin class tag.
    fn object_builtin_tag(&mut self, object: &Value, builtin: &str) -> Result<String> {
        if let Some(tag) = self.object_to_string_tag(object)? {
            return Ok(tag);
        }
        Ok(builtin.to_owned())
    }

    fn object_builtin_class(&self, id: ObjectId) -> Result<&'static str> {
        if self.objects.error_metadata(id)?.is_some() {
            return Ok(TAG_ERROR);
        }
        if self.objects.is_arguments_object(id)? {
            return Ok(TAG_ARGUMENTS);
        }
        if self.objects.array_len_if_array(id)?.is_some() {
            return Ok(TAG_ARRAY);
        }
        if self.objects.regexp_value(id)?.is_some() {
            return Ok(TAG_REGEXP);
        }
        if self.objects.date_value(id)?.is_some() {
            return Ok(TAG_DATE);
        }
        if self.objects.string_object_value(id)?.is_some() {
            return Ok(TAG_STRING);
        }
        match self.objects.primitive_value(id)? {
            Some(ObjectPrimitiveValue::Bool(_)) => Ok(TAG_BOOLEAN),
            Some(ObjectPrimitiveValue::Number(_)) => Ok(TAG_NUMBER),
            Some(ObjectPrimitiveValue::BigInt(_) | ObjectPrimitiveValue::Symbol(_)) | None => {
                Ok(TAG_OBJECT)
            }
        }
    }

    fn object_to_string_tag(&mut self, object: &Value) -> Result<Option<String>> {
        let Some(symbol) = self.resolve_to_string_tag_symbol()? else {
            return Ok(None);
        };
        let key = DynamicPropertyKey::new(
            TO_STRING_TAG_DISPLAY.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        let value = self.get(object, key.lookup())?;
        Ok(match value {
            Value::String(text) => Some(text.into_string()),
            _ => None,
        })
    }

    fn resolve_to_string_tag_symbol(&mut self) -> Result<Option<crate::storage::symbol::SymbolId>> {
        let constructor = self.symbol_constructor_value()?;
        let value = self.get_named(&constructor, TO_STRING_TAG_PROPERTY)?;
        Ok(match value {
            Value::Symbol(symbol) => Some(symbol.id()),
            _ => None,
        })
    }

    pub(in crate::runtime) fn define_builtin_to_string_tag(
        &mut self,
        object: ObjectId,
        tag: &str,
    ) -> Result<()> {
        let symbol = self
            .resolve_to_string_tag_symbol()?
            .ok_or_else(|| Error::runtime("Symbol.toStringTag is not initialized"))?;
        let value = self.heap_string_value(tag)?;
        self.objects.define_property(
            object,
            PropertyKey::symbol(symbol),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(crate::runtime::object::DataPropertyUpdate::new(
                Some(value),
                Some(crate::runtime::object::PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }
}
