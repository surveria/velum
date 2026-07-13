use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{
            NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY, SHARED_ARRAY_BUFFER_NAME,
            SharedArrayBufferFunctionKind,
        },
        object::{
            AccessorPropertyUpdate, ByteBuffer, DataPropertyUpdate, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

const RECEIVER_ERROR: &str = "SharedArrayBuffer method receiver is not a SharedArrayBuffer";
const LENGTH_ERROR: &str = "SharedArrayBuffer length exceeded supported range";
const SPECIES_ERROR: &str = "SharedArrayBuffer species is not a constructor";
const SPECIES_RESULT_ERROR: &str =
    "SharedArrayBuffer species constructor returned an invalid buffer";
const SPECIES_DISPLAY: &str = "[Symbol.species]";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";

impl Context {
    pub(in crate::runtime::native) fn shared_array_buffer_constructor_value(
        &mut self,
    ) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::SharedArrayBuffer) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.typed_storage_prototype_with_constructor(constructor.clone(), None)?;
        let name = self.native_function_name_value(NativeFunctionKind::SharedArrayBuffer)?;
        self.push_native_function_with_id(
            id,
            NativeFunctionKind::SharedArrayBuffer,
            Value::Object(prototype),
            name,
        )?;
        self.install_shared_array_buffer_builtins(prototype)?;
        self.install_species_accessor(id)?;
        self.insert_global_builtin(SHARED_ARRAY_BUFFER_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn construct_shared_array_buffer(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let length = Self::length_to_usize(self.to_index(args.as_slice().first())?, LENGTH_ERROR)?;
        self.check_byte_buffer_length(length)?;
        let maximum = self.array_buffer_max_byte_length_option(args.as_slice().get(1))?;
        if maximum.is_some_and(|maximum| maximum < length) {
            return Err(Error::exception(
                ErrorName::RangeError,
                "SharedArrayBuffer maxByteLength is smaller than byteLength",
            ));
        }
        if let Some(maximum) = maximum {
            self.check_byte_buffer_length(maximum)?;
        }
        self.create_shared_array_buffer_value(ByteBuffer::new_shared(length, maximum))
    }

    pub(in crate::runtime::native) fn eval_shared_array_buffer_native_function_kind(
        &mut self,
        kind: SharedArrayBufferFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            SharedArrayBufferFunctionKind::ByteLengthGetter => {
                let (_, buffer) = self.shared_array_buffer_receiver(this_value)?;
                Self::shared_buffer_usize_value(buffer.byte_length())
            }
            SharedArrayBufferFunctionKind::MaxByteLengthGetter => {
                let (_, buffer) = self.shared_array_buffer_receiver(this_value)?;
                Self::shared_buffer_usize_value(buffer.max_byte_length())
            }
            SharedArrayBufferFunctionKind::GrowableGetter => {
                let (_, buffer) = self.shared_array_buffer_receiver(this_value)?;
                Ok(Value::Bool(buffer.is_resizable()))
            }
            SharedArrayBufferFunctionKind::Grow => {
                self.eval_shared_array_buffer_grow(args, this_value)
            }
            SharedArrayBufferFunctionKind::Slice => {
                self.eval_shared_array_buffer_slice(args, this_value)
            }
        }
    }

    fn eval_shared_array_buffer_grow(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let (id, _) = self.shared_array_buffer_receiver(this_value)?;
        let length = Self::length_to_usize(self.to_index(args.as_slice().first())?, LENGTH_ERROR)?;
        self.objects.grow_shared_array_buffer(id, length)?;
        Ok(Value::Undefined)
    }

    fn eval_shared_array_buffer_slice(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let (_, source) = self.shared_array_buffer_receiver(this_value)?;
        let length = source.byte_length();
        let values = args.as_slice();
        let start = self.shared_buffer_relative_index(values.first(), length, 0)?;
        let end = self.shared_buffer_relative_index(values.get(1), length, length)?;
        let new_length = end.saturating_sub(start);
        let constructor = self.shared_array_buffer_species_constructor(this_value)?;
        let length_value = Self::shared_buffer_usize_value(new_length)?;
        let result = self.semantic_construct(
            &constructor,
            std::slice::from_ref(&length_value),
            constructor.clone(),
        )?;
        let (_, target) = self.shared_array_buffer_receiver(&result)?;
        if result == *this_value || target.byte_length() < new_length {
            return Err(Error::type_error(SPECIES_RESULT_ERROR));
        }
        let copy_end = start
            .checked_add(new_length)
            .ok_or_else(|| Error::limit(LENGTH_ERROR))?;
        target.write(0, &source.copy_bytes(start, copy_end)?)?;
        Ok(result)
    }

    fn install_shared_array_buffer_builtins(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (
                "byteLength",
                SharedArrayBufferFunctionKind::ByteLengthGetter,
            ),
            (
                "maxByteLength",
                SharedArrayBufferFunctionKind::MaxByteLengthGetter,
            ),
            ("growable", SharedArrayBufferFunctionKind::GrowableGetter),
        ] {
            self.define_shared_buffer_accessor(prototype, name, kind)?;
        }
        for (name, kind) in [
            ("grow", SharedArrayBufferFunctionKind::Grow),
            ("slice", SharedArrayBufferFunctionKind::Slice),
        ] {
            let method = self.create_native_function(
                NativeFunctionKind::SharedArrayBufferPrototype(kind),
                Value::Undefined,
            )?;
            self.define_non_enumerable_object_property(prototype, name, method)?;
        }
        self.define_shared_buffer_to_string_tag(prototype)
    }

    fn define_shared_buffer_accessor(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: SharedArrayBufferFunctionKind,
    ) -> Result<()> {
        let getter = self.create_native_function(
            NativeFunctionKind::SharedArrayBufferPrototype(kind),
            Value::Undefined,
        )?;
        let key = self.intern_property_key(name)?;
        self.objects.define_property(
            prototype,
            key,
            name,
            PropertyUpdate::Accessor(AccessorPropertyUpdate::new(
                Some(getter),
                None,
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn define_shared_buffer_to_string_tag(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(tag) = self.get_named(&symbol, "toStringTag")? else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(SHARED_ARRAY_BUFFER_NAME)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag.id()),
            TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    pub(crate) fn create_shared_array_buffer_value(&mut self, buffer: ByteBuffer) -> Result<Value> {
        let prototype = self.shared_array_buffer_constructor_prototype()?;
        self.objects
            .create_array_buffer(buffer, prototype, self.limits.max_objects)
            .map(Value::Object)
    }

    fn shared_array_buffer_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.shared_array_buffer_constructor_value()? else {
            return Err(Error::runtime(
                "SharedArrayBuffer constructor is not native",
            ));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime(
                "SharedArrayBuffer prototype is not an object",
            )),
        }
    }

    fn shared_array_buffer_receiver(&self, value: &Value) -> Result<(ObjectId, ByteBuffer)> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(RECEIVER_ERROR));
        };
        self.objects
            .array_buffer(*id)?
            .filter(ByteBuffer::is_shared)
            .map(|buffer| (*id, buffer))
            .ok_or_else(|| Error::type_error(RECEIVER_ERROR))
    }

    fn shared_array_buffer_species_constructor(&mut self, source: &Value) -> Result<Value> {
        let default = self.shared_array_buffer_constructor_value()?;
        let constructor = self.get_named(source, OBJECT_CONSTRUCTOR_PROPERTY)?;
        if matches!(constructor, Value::Undefined) {
            return Ok(default);
        }
        if self.semantic_object_ref(&constructor)?.is_none() {
            return Err(Error::type_error(SPECIES_ERROR));
        }
        let symbol = self.symbol_constructor_value()?;
        let Value::Symbol(species) = self.get_named(&symbol, "species")? else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        let value = self.get(
            &constructor,
            PropertyLookup::from_key(SPECIES_DISPLAY, PropertyKey::symbol(species.id())),
        )?;
        if matches!(value, Value::Undefined | Value::Null) {
            return Ok(default);
        }
        if !self.semantic_is_constructor(&value)? {
            return Err(Error::type_error(SPECIES_ERROR));
        }
        Ok(value)
    }

    fn shared_buffer_relative_index(
        &mut self,
        value: Option<&Value>,
        length: usize,
        default: usize,
    ) -> Result<usize> {
        let Some(value) = value.filter(|value| !matches!(value, Value::Undefined)) else {
            return Ok(default);
        };
        let relative = self.to_integer_or_infinity(value)?;
        let length_number = Self::shared_buffer_usize_number(length)?;
        let absolute = if relative == f64::NEG_INFINITY {
            0.0
        } else if relative == f64::INFINITY {
            length_number
        } else if relative < 0.0 {
            (length_number + relative).max(0.0)
        } else {
            relative.min(length_number)
        };
        Self::finite_nonnegative_integer_to_usize(absolute, LENGTH_ERROR)
    }

    fn shared_buffer_usize_value(value: usize) -> Result<Value> {
        Self::shared_buffer_usize_number(value).map(Value::Number)
    }

    fn shared_buffer_usize_number(value: usize) -> Result<f64> {
        let value = u32::try_from(value).map_err(|_| Error::limit(LENGTH_ERROR))?;
        Ok(f64::from(value))
    }
}
