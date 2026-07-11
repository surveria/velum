use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{ARRAY_BUFFER_NAME, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY},
        object::{
            ByteBuffer, ByteBufferOrigin, ObjectPropertyInit, PropertyEnumerable,
            TypedArrayElementKind, TypedArrayView,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

const BYTES_PER_ELEMENT_PROPERTY: &str = "BYTES_PER_ELEMENT";
const TYPED_ARRAY_LENGTH_LIMIT_ERROR: &str = "typed array length exceeded supported range";
const TYPED_ARRAY_BYTE_LENGTH_LIMIT_ERROR: &str =
    "typed array byte length exceeded supported range";
const TYPED_ARRAY_BUFFER_RANGE_ERROR: &str = "typed array view exceeds its ArrayBuffer";
const TYPED_ARRAY_OFFSET_ALIGNMENT_ERROR: &str =
    "typed array byteOffset must align to the element size";

impl Context {
    pub(in crate::runtime::native) fn array_buffer_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::ArrayBuffer) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.typed_storage_prototype_with_constructor(constructor.clone())?;
        let name = self.native_function_name_value(NativeFunctionKind::ArrayBuffer)?;
        self.push_native_function_with_id(
            id,
            NativeFunctionKind::ArrayBuffer,
            Value::Object(prototype),
            name,
        )?;
        self.insert_global_builtin(ARRAY_BUFFER_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn typed_array_constructor_value(
        &mut self,
        element_kind: TypedArrayElementKind,
    ) -> Result<Value> {
        let function_kind = NativeFunctionKind::TypedArray(element_kind);
        if let Some(id) = self.native_function_id(function_kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.typed_storage_prototype_with_constructor(constructor.clone())?;
        let name = self.native_function_name_value(function_kind)?;
        self.push_native_function_with_id(id, function_kind, Value::Object(prototype), name)?;
        self.define_non_enumerable_object_property(
            prototype,
            BYTES_PER_ELEMENT_PROPERTY,
            Value::Number(Self::element_size_number(element_kind)?),
        )?;
        self.insert_global_builtin(element_kind.name(), constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn construct_array_buffer(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let index = self.to_index(args.as_slice().first())?;
        let length = Self::length_to_usize(index, TYPED_ARRAY_LENGTH_LIMIT_ERROR)?;
        self.check_byte_buffer_length(length)?;
        let buffer = ByteBuffer::new(length, ByteBufferOrigin::EngineOwned);
        self.create_array_buffer_value(buffer)
    }

    pub(in crate::runtime) fn construct_typed_array(
        &mut self,
        element_kind: TypedArrayElementKind,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let source = values.first().cloned().unwrap_or(Value::Undefined);
        if let Value::Object(buffer_object) = source
            && let Some(buffer) = self.objects.array_buffer(buffer_object)?
        {
            return self.create_typed_array_from_buffer(
                element_kind,
                buffer,
                buffer_object,
                values.get(1),
                values.get(2),
            );
        }
        if self.semantic_object_ref(&source)?.is_some() {
            return self.create_typed_array_from_array_like(element_kind, &source);
        }
        let index = self.to_index(Some(&source))?;
        let length = Self::length_to_usize(index, TYPED_ARRAY_LENGTH_LIMIT_ERROR)?;
        self.create_typed_array_with_length(element_kind, length)
    }

    /// Creates a VM-global `Uint8Array` from bytes supplied by the Rust host.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM cannot allocate the backing objects or global
    /// binding.
    pub fn create_host_uint8_array_global(&mut self, name: &str, bytes: Vec<u8>) -> Result<Value> {
        let buffer = ByteBuffer::from_bytes(bytes, ByteBufferOrigin::HostProvided);
        let value = self.create_typed_array_with_buffer(TypedArrayElementKind::Uint8, buffer)?;
        self.insert_global_builtin(name, value.clone())?;
        self.sync_global_object_binding_property(name, value.clone())?;
        Ok(value)
    }

    /// Returns the debug storage origin for a numeric typed array object.
    ///
    /// # Errors
    ///
    /// Returns an error if the value references an object-like storage slot
    /// that is not live in this VM.
    pub fn typed_array_debug_origin(&self, value: &Value) -> Result<Option<&'static str>> {
        let Some(object) = self.semantic_object_ref(value)? else {
            return Ok(None);
        };
        let Some(id) = object.object_id() else {
            return Ok(None);
        };
        let Some(origin) = self.objects.typed_array_debug_origin(id)? else {
            return Ok(None);
        };
        Ok(Some(match origin {
            ByteBufferOrigin::EngineOwned => "engine-owned",
            ByteBufferOrigin::HostProvided => "host-provided",
        }))
    }

    fn create_typed_array_from_array_like(
        &mut self,
        element_kind: TypedArrayElementKind,
        source: &Value,
    ) -> Result<Value> {
        let length_value = self.get_named(source, "length")?;
        let length = Self::length_to_usize(
            self.to_length(&length_value)?,
            TYPED_ARRAY_LENGTH_LIMIT_ERROR,
        )?;
        self.check_typed_array_length(element_kind, length)?;
        let mut numbers = Vec::with_capacity(length);
        for index in 0..length {
            self.step()?;
            let value = self.get_named(source, &index.to_string())?;
            numbers.push(self.to_number(&value)?);
        }
        let value = self.create_typed_array_with_length(element_kind, length)?;
        let Value::Object(id) = value else {
            return Err(Error::runtime(
                "typed array allocation did not return an object",
            ));
        };
        for (index, number) in numbers.into_iter().enumerate() {
            if !self.objects.set_typed_array_number(id, index, number)? {
                return Err(Error::runtime(
                    "typed array initialization index is out of bounds",
                ));
            }
        }
        Ok(Value::Object(id))
    }

    fn create_typed_array_from_buffer(
        &mut self,
        element_kind: TypedArrayElementKind,
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: Option<&Value>,
        requested_length: Option<&Value>,
    ) -> Result<Value> {
        let byte_offset = Self::length_to_usize(
            self.to_index(byte_offset)?,
            TYPED_ARRAY_BYTE_LENGTH_LIMIT_ERROR,
        )?;
        let element_size = element_kind.bytes_per_element();
        if !byte_offset.is_multiple_of(element_size) {
            return Err(Error::exception(
                ErrorName::RangeError,
                TYPED_ARRAY_OFFSET_ALIGNMENT_ERROR,
            ));
        }
        let buffer_length = buffer.byte_length();
        let Some(available) = buffer_length.checked_sub(byte_offset) else {
            return Err(Error::exception(
                ErrorName::RangeError,
                TYPED_ARRAY_BUFFER_RANGE_ERROR,
            ));
        };
        let length = if requested_length.is_some_and(|value| !matches!(value, Value::Undefined)) {
            let requested = Self::length_to_usize(
                self.to_index(requested_length)?,
                TYPED_ARRAY_LENGTH_LIMIT_ERROR,
            )?;
            let required = self.typed_array_byte_length(element_kind, requested)?;
            if required > available {
                return Err(Error::exception(
                    ErrorName::RangeError,
                    TYPED_ARRAY_BUFFER_RANGE_ERROR,
                ));
            }
            requested
        } else {
            if !available.is_multiple_of(element_size) {
                return Err(Error::exception(
                    ErrorName::RangeError,
                    TYPED_ARRAY_BUFFER_RANGE_ERROR,
                ));
            }
            available / element_size
        };
        self.create_typed_array_value(element_kind, buffer, buffer_object, byte_offset, length)
    }

    fn create_typed_array_with_length(
        &mut self,
        element_kind: TypedArrayElementKind,
        length: usize,
    ) -> Result<Value> {
        let byte_length = self.typed_array_byte_length(element_kind, length)?;
        let buffer = ByteBuffer::new(byte_length, ByteBufferOrigin::EngineOwned);
        self.create_typed_array_with_buffer(element_kind, buffer)
    }

    fn create_typed_array_with_buffer(
        &mut self,
        element_kind: TypedArrayElementKind,
        buffer: ByteBuffer,
    ) -> Result<Value> {
        let buffer_object = self.create_array_buffer_object(buffer.clone())?;
        let byte_length = buffer.byte_length();
        let element_size = element_kind.bytes_per_element();
        if !byte_length.is_multiple_of(element_size) {
            return Err(Error::exception(
                ErrorName::RangeError,
                TYPED_ARRAY_BUFFER_RANGE_ERROR,
            ));
        }
        self.create_typed_array_value(
            element_kind,
            buffer,
            buffer_object,
            0,
            byte_length / element_size,
        )
    }

    fn create_array_buffer_value(&mut self, buffer: ByteBuffer) -> Result<Value> {
        self.create_array_buffer_object(buffer).map(Value::Object)
    }

    fn create_array_buffer_object(&mut self, buffer: ByteBuffer) -> Result<ObjectId> {
        let prototype = self.array_buffer_constructor_prototype()?;
        self.objects
            .create_array_buffer(buffer, prototype, self.limits.max_objects)
    }

    fn create_typed_array_value(
        &mut self,
        element_kind: TypedArrayElementKind,
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: usize,
        length: usize,
    ) -> Result<Value> {
        let view = TypedArrayView::new(buffer, buffer_object, byte_offset, length, element_kind);
        let prototype = self.typed_array_constructor_prototype(element_kind)?;
        self.objects
            .create_typed_array(view, prototype, self.limits.max_objects)
            .map(Value::Object)
    }

    fn typed_storage_prototype_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.create_with_prototype_property(
            None,
            ObjectPropertyInit::new(
                constructor_key,
                OBJECT_CONSTRUCTOR_PROPERTY,
                constructor,
                PropertyEnumerable::No,
            ),
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
    }

    fn array_buffer_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.array_buffer_constructor_value()? else {
            return Err(Error::runtime(
                "ArrayBuffer constructor value is not native",
            ));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("ArrayBuffer prototype is not an object")),
        }
    }

    fn typed_array_constructor_prototype(
        &mut self,
        element_kind: TypedArrayElementKind,
    ) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.typed_array_constructor_value(element_kind)? else {
            return Err(Error::runtime(
                "typed array constructor value is not native",
            ));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("typed array prototype is not an object")),
        }
    }

    fn typed_array_byte_length(
        &self,
        element_kind: TypedArrayElementKind,
        length: usize,
    ) -> Result<usize> {
        let byte_length = length
            .checked_mul(element_kind.bytes_per_element())
            .ok_or_else(|| Error::limit(TYPED_ARRAY_BYTE_LENGTH_LIMIT_ERROR))?;
        self.check_byte_buffer_length(byte_length)?;
        Ok(byte_length)
    }

    fn check_typed_array_length(
        &self,
        element_kind: TypedArrayElementKind,
        length: usize,
    ) -> Result<()> {
        self.typed_array_byte_length(element_kind, length).map(drop)
    }

    fn check_byte_buffer_length(&self, length: usize) -> Result<()> {
        if length > self.limits.max_object_properties {
            return Err(Error::limit(format!(
                "typed array byte length exceeded {}",
                self.limits.max_object_properties
            )));
        }
        Ok(())
    }

    fn element_size_number(element_kind: TypedArrayElementKind) -> Result<f64> {
        let size = u32::try_from(element_kind.bytes_per_element())
            .map_err(|_| Error::limit(TYPED_ARRAY_BYTE_LENGTH_LIMIT_ERROR))?;
        Ok(f64::from(size))
    }
}
