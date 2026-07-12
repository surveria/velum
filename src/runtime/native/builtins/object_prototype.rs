use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        control::Completion,
        object::{ObjectPrimitiveValue, PropertyKey},
        property::DynamicPropertyKey,
    },
    value::{ObjectId, Value},
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
const TAG_BIGINT: &str = "BigInt";

impl Context {
    pub(in crate::runtime::native) fn eval_object_prototype_to_string(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let tag = match this_value {
            Value::Undefined => return self.heap_string_value(OBJECT_UNDEFINED_TAG),
            Value::Null => return self.heap_string_value(OBJECT_NULL_TAG),
            Value::Object(id) => {
                let builtin = if self.semantic_is_callable(this_value)? {
                    TAG_FUNCTION
                } else {
                    self.object_builtin_class(*id)?
                };
                self.object_builtin_tag(*id, builtin)?
            }
            Value::Function(_) | Value::NativeFunction(_) | Value::HostFunction(_) => {
                TAG_FUNCTION.to_owned()
            }
            Value::Bool(_) => TAG_BOOLEAN.to_owned(),
            Value::Number(_) => TAG_NUMBER.to_owned(),
            Value::BigInt(_) => TAG_BIGINT.to_owned(),
            Value::String(_) | Value::HeapString(_) => TAG_STRING.to_owned(),
            Value::Symbol(_) => TAG_OBJECT.to_owned(),
        };
        self.heap_string_value(&format!("[object {tag}]"))
    }

    pub(in crate::runtime::native) fn eval_object_prototype_value_of(
        &mut self,
        _args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.object_to_object(this_value)
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
        let receiver = self.object_to_object(this_value)?;
        let Some(mut current) = args.as_slice().first().cloned() else {
            return Ok(Value::Bool(false));
        };
        if self.semantic_object_ref(&current)?.is_none() {
            return Ok(Value::Bool(false));
        }
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
        let name = dynamic.name().to_owned();
        let property_key = self.intern_dynamic_property_key(&mut dynamic)?;
        self.set_property_value_with_accessors(result, property_key, &name, value)
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
            Value::String(_) | Value::HeapString(_) => {
                self.create_string_object_from_value(this_value)
            }
            Value::Symbol(value) => self.create_symbol_object_from_value(value.clone()),
            Value::Undefined | Value::Null => Err(Error::type_error(OBJECT_RECEIVER_ERROR)),
        }
    }

    /// Determine the `Object.prototype.toString` tag for an object, honoring a
    /// string-valued `Symbol.toStringTag` over the builtin class tag.
    fn object_builtin_tag(&mut self, id: ObjectId, builtin: &str) -> Result<String> {
        if let Some(tag) = self.object_to_string_tag(id)? {
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
            Some(ObjectPrimitiveValue::BigInt(_)) => Ok(TAG_BIGINT),
            Some(ObjectPrimitiveValue::Symbol(_)) | None => Ok(TAG_OBJECT),
        }
    }

    fn object_to_string_tag(&mut self, id: ObjectId) -> Result<Option<String>> {
        let Some(symbol) = self.resolve_to_string_tag_symbol()? else {
            return Ok(None);
        };
        let key = DynamicPropertyKey::new(
            TO_STRING_TAG_DISPLAY.to_owned(),
            Some(PropertyKey::symbol(symbol)),
        );
        let receiver = Value::Object(id);
        let value = self.get(&receiver, key.lookup())?;
        Ok(match value {
            Value::String(text) => Some(text),
            Value::HeapString(text) => Some(text.as_str().to_owned()),
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
}
