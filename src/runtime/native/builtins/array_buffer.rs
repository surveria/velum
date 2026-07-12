use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{ARRAY_BUFFER_NAME, ArrayBufferFunctionKind, NativeFunctionKind},
        object::{
            AccessorPropertyUpdate, ByteBuffer, DataPropertyUpdate, PropertyConfigurable,
            PropertyEnumerable, PropertyKey, PropertyLookup, PropertyUpdate, PropertyWritable,
        },
    },
    value::{ErrorName, NativeFunctionId, ObjectId, Value},
};

const RECEIVER_TYPE_ERROR: &str = "ArrayBuffer method receiver is not an ArrayBuffer";
const DETACHED_BUFFER_ERROR: &str = "ArrayBuffer is detached";
const LENGTH_LIMIT_ERROR: &str = "ArrayBuffer length exceeded supported range";
const SPECIES_ERROR: &str = "ArrayBuffer species is not a constructor";
const SPECIES_RESULT_ERROR: &str = "ArrayBuffer species constructor returned an invalid buffer";
const SPECIES_PROPERTY: &str = "species";
const SPECIES_DISPLAY: &str = "[Symbol.species]";
const TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";

impl Context {
    pub(super) fn install_array_buffer_builtins(
        &mut self,
        constructor: NativeFunctionId,
        prototype: ObjectId,
    ) -> Result<()> {
        self.define_array_buffer_static(constructor, "isView", ArrayBufferFunctionKind::IsView)?;
        for (name, kind) in [
            ("byteLength", ArrayBufferFunctionKind::ByteLengthGetter),
            (
                "maxByteLength",
                ArrayBufferFunctionKind::MaxByteLengthGetter,
            ),
            ("resizable", ArrayBufferFunctionKind::ResizableGetter),
            ("detached", ArrayBufferFunctionKind::DetachedGetter),
        ] {
            self.define_array_buffer_accessor(prototype, name, kind)?;
        }
        for (name, kind) in [
            ("resize", ArrayBufferFunctionKind::Resize),
            ("slice", ArrayBufferFunctionKind::Slice),
            ("transfer", ArrayBufferFunctionKind::Transfer),
            (
                "transferToFixedLength",
                ArrayBufferFunctionKind::TransferToFixedLength,
            ),
        ] {
            self.define_array_buffer_method(prototype, name, kind)?;
        }
        self.define_array_buffer_to_string_tag(prototype)
    }

    pub(in crate::runtime::native) fn eval_array_buffer_native_function_kind(
        &mut self,
        kind: ArrayBufferFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            ArrayBufferFunctionKind::IsView => self.eval_array_buffer_is_view(args),
            ArrayBufferFunctionKind::ByteLengthGetter => {
                let (_, buffer) = self.array_buffer_receiver(this_value)?;
                Self::array_buffer_usize_value(buffer.byte_length())
            }
            ArrayBufferFunctionKind::MaxByteLengthGetter => {
                let (_, buffer) = self.array_buffer_receiver(this_value)?;
                Self::array_buffer_usize_value(buffer.max_byte_length())
            }
            ArrayBufferFunctionKind::ResizableGetter => {
                let (_, buffer) = self.array_buffer_receiver(this_value)?;
                Ok(Value::Bool(buffer.is_resizable()))
            }
            ArrayBufferFunctionKind::DetachedGetter => {
                let (_, buffer) = self.array_buffer_receiver(this_value)?;
                Ok(Value::Bool(buffer.is_detached()))
            }
            ArrayBufferFunctionKind::Resize => self.eval_array_buffer_resize(args, this_value),
            ArrayBufferFunctionKind::Slice => self.eval_array_buffer_slice(args, this_value),
            ArrayBufferFunctionKind::Transfer => {
                self.eval_array_buffer_transfer(args, this_value, true)
            }
            ArrayBufferFunctionKind::TransferToFixedLength => {
                self.eval_array_buffer_transfer(args, this_value, false)
            }
        }
    }

    fn eval_array_buffer_is_view(&self, args: RuntimeCallArgs<'_>) -> Result<Value> {
        let is_view = if let Some(Value::Object(id)) = args.as_slice().first() {
            self.objects.is_array_buffer_view(*id)?
        } else {
            false
        };
        Ok(Value::Bool(is_view))
    }

    fn eval_array_buffer_resize(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let (id, _) = self.array_buffer_receiver(this_value)?;
        let new_length =
            Self::length_to_usize(self.to_index(args.as_slice().first())?, LENGTH_LIMIT_ERROR)?;
        self.objects.resize_array_buffer(id, new_length)?;
        Ok(Value::Undefined)
    }

    fn eval_array_buffer_slice(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let (_, source) = self.array_buffer_receiver(this_value)?;
        if source.is_detached() {
            return Err(Error::type_error(DETACHED_BUFFER_ERROR));
        }
        let length = source.byte_length();
        let values = args.as_slice();
        let start = self.array_buffer_relative_index(values.first(), length, 0)?;
        let end = if values
            .get(1)
            .is_some_and(|value| !matches!(value, Value::Undefined))
        {
            self.array_buffer_relative_index(values.get(1), length, length)?
        } else {
            length
        };
        let new_length = end.saturating_sub(start);
        let constructor = self.array_buffer_species_constructor(this_value)?;
        let length_value = Self::array_buffer_usize_value(new_length)?;
        let result = self.semantic_construct(
            &constructor,
            std::slice::from_ref(&length_value),
            constructor.clone(),
        )?;
        let (_, target) = self.array_buffer_receiver(&result)?;
        if result == *this_value || target.is_detached() || target.byte_length() < new_length {
            return Err(Error::type_error(SPECIES_RESULT_ERROR));
        }
        if source.is_detached() {
            return Err(Error::type_error(DETACHED_BUFFER_ERROR));
        }
        let copy_end = start
            .checked_add(new_length)
            .ok_or_else(|| Error::limit(LENGTH_LIMIT_ERROR))?;
        let bytes = source.copy_bytes(start, copy_end)?;
        target.write(0, &bytes)?;
        Ok(result)
    }

    fn eval_array_buffer_transfer(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
        preserve_resizability: bool,
    ) -> Result<Value> {
        let (id, source) = self.array_buffer_receiver(this_value)?;
        if source.is_detached() {
            return Err(Error::type_error(DETACHED_BUFFER_ERROR));
        }
        let current_length = source.byte_length();
        let new_length = if args
            .as_slice()
            .first()
            .is_some_and(|value| !matches!(value, Value::Undefined))
        {
            Self::length_to_usize(self.to_index(args.as_slice().first())?, LENGTH_LIMIT_ERROR)?
        } else {
            current_length
        };
        if source.is_detached() {
            return Err(Error::type_error(DETACHED_BUFFER_ERROR));
        }
        let max_byte_length = source.max_byte_length();
        if preserve_resizability && source.is_resizable() && new_length > max_byte_length {
            return Err(Error::exception(
                ErrorName::RangeError,
                "transfer length exceeds maxByteLength",
            ));
        }
        self.check_byte_buffer_length(new_length)?;
        let mut bytes = source.copy_bytes(0, current_length)?;
        bytes.resize(new_length, 0);
        let result_buffer = if preserve_resizability && source.is_resizable() {
            ByteBuffer::from_resizable_bytes(bytes, max_byte_length)
        } else {
            ByteBuffer::from_bytes(bytes, crate::runtime::object::ByteBufferOrigin::EngineOwned)
        };
        let result = self.create_array_buffer_value(result_buffer)?;
        self.objects.detach_array_buffer(id)?;
        Ok(result)
    }

    fn array_buffer_receiver(&self, value: &Value) -> Result<(ObjectId, ByteBuffer)> {
        let Value::Object(id) = value else {
            return Err(Error::type_error(RECEIVER_TYPE_ERROR));
        };
        self.objects
            .array_buffer(*id)?
            .filter(|buffer| !buffer.is_shared())
            .map(|buffer| (*id, buffer))
            .ok_or_else(|| Error::type_error(RECEIVER_TYPE_ERROR))
    }

    fn array_buffer_relative_index(
        &mut self,
        value: Option<&Value>,
        length: usize,
        default: usize,
    ) -> Result<usize> {
        let Some(value) = value else {
            return Ok(default);
        };
        let relative = self.to_integer_or_infinity(value)?;
        if relative == f64::NEG_INFINITY {
            return Ok(0);
        }
        if relative == f64::INFINITY {
            return Ok(length);
        }
        let length_number = Self::array_buffer_usize_number(length)?;
        let absolute = if relative < 0.0 {
            (length_number + relative).max(0.0)
        } else {
            relative.min(length_number)
        };
        Self::finite_nonnegative_integer_to_usize(absolute, LENGTH_LIMIT_ERROR)
    }

    fn array_buffer_species_constructor(&mut self, source: &Value) -> Result<Value> {
        let default = self.array_buffer_constructor_value()?;
        let constructor = self.get_named(source, "constructor")?;
        if matches!(constructor, Value::Undefined) {
            return Ok(default);
        }
        if self.semantic_object_ref(&constructor)?.is_none() {
            return Err(Error::type_error(SPECIES_ERROR));
        }
        let symbol_constructor = self.symbol_constructor_value()?;
        let species_symbol = self.get_named(&symbol_constructor, SPECIES_PROPERTY)?;
        let Value::Symbol(species_symbol) = species_symbol else {
            return Err(Error::runtime("Symbol.species is not initialized"));
        };
        let lookup =
            PropertyLookup::from_key(SPECIES_DISPLAY, PropertyKey::symbol(species_symbol.id()));
        let species = self.get(&constructor, lookup)?;
        if matches!(species, Value::Undefined | Value::Null) {
            return Ok(default);
        }
        if !self.semantic_is_constructor(&species)? {
            return Err(Error::type_error(SPECIES_ERROR));
        }
        Ok(species)
    }

    fn define_array_buffer_static(
        &mut self,
        constructor: NativeFunctionId,
        name: &str,
        kind: ArrayBufferFunctionKind,
    ) -> Result<()> {
        let method = self.create_native_function(
            NativeFunctionKind::ArrayBufferPrototype(kind),
            Value::Undefined,
        )?;
        let key = self.intern_property_key(name)?;
        self.define_native_function_property_key(
            constructor,
            name,
            key,
            DataPropertyUpdate::new(
                Some(method),
                Some(PropertyWritable::Yes),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            ),
        )
    }

    fn define_array_buffer_accessor(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: ArrayBufferFunctionKind,
    ) -> Result<()> {
        let getter = self.create_native_function(
            NativeFunctionKind::ArrayBufferPrototype(kind),
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

    fn define_array_buffer_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: ArrayBufferFunctionKind,
    ) -> Result<()> {
        let method = self.create_native_function(
            NativeFunctionKind::ArrayBufferPrototype(kind),
            Value::Undefined,
        )?;
        self.define_non_enumerable_object_property(prototype, name, method)
    }

    fn define_array_buffer_to_string_tag(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag = self.get_named(&symbol_constructor, TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag) = tag else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(ARRAY_BUFFER_NAME)?;
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

    fn array_buffer_usize_value(value: usize) -> Result<Value> {
        Self::array_buffer_usize_number(value).map(Value::Number)
    }

    fn array_buffer_usize_number(value: usize) -> Result<f64> {
        let value = u32::try_from(value).map_err(|_| Error::limit(LENGTH_LIMIT_ERROR))?;
        Ok(f64::from(value))
    }
}
