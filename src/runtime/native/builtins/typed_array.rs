use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        call::RuntimeCallArgs,
        native::{ARRAY_BUFFER_NAME, NativeFunctionKind, UINT8_ARRAY_NAME},
        object::{ByteBuffer, ByteBufferOrigin, Uint8ArrayView},
    },
    value::{ObjectId, Value},
};

const TYPED_ARRAY_LENGTH_LIMIT_ERROR: &str = "typed array length exceeded supported range";

impl Context {
    pub(in crate::runtime::native) fn array_buffer_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::ArrayBuffer) {
            return Ok(Value::NativeFunction(id));
        }
        let constructor =
            self.create_native_function(NativeFunctionKind::ArrayBuffer, Value::Undefined)?;
        self.insert_global_builtin(ARRAY_BUFFER_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime::native) fn uint8_array_constructor_value(&mut self) -> Result<Value> {
        if let Some(id) = self.native_function_id(NativeFunctionKind::Uint8Array) {
            return Ok(Value::NativeFunction(id));
        }
        let constructor =
            self.create_native_function(NativeFunctionKind::Uint8Array, Value::Undefined)?;
        self.insert_global_builtin(UINT8_ARRAY_NAME, constructor.clone())?;
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

    pub(in crate::runtime) fn construct_uint8_array(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let source = args.as_slice().first();
        if let Some(Value::Object(buffer_object)) = source
            && let Some(buffer) = self.objects.array_buffer(*buffer_object)?
        {
            return self.create_uint8_array_value(buffer, *buffer_object);
        }
        let index = self.to_index(source)?;
        let length = Self::length_to_usize(index, TYPED_ARRAY_LENGTH_LIMIT_ERROR)?;
        self.check_byte_buffer_length(length)?;
        let buffer = ByteBuffer::new(length, ByteBufferOrigin::EngineOwned);
        self.create_uint8_array_with_buffer(buffer)
    }

    /// Creates a VM-global `Uint8Array` from bytes supplied by the Rust host.
    ///
    /// # Errors
    ///
    /// Returns an error if the VM cannot allocate the backing objects or global
    /// binding.
    pub fn create_host_uint8_array_global(&mut self, name: &str, bytes: Vec<u8>) -> Result<Value> {
        let buffer = ByteBuffer::from_bytes(bytes, ByteBufferOrigin::HostProvided);
        let value = self.create_uint8_array_with_buffer(buffer)?;
        self.insert_global_builtin(name, value.clone())?;
        self.sync_global_object_binding_property(name, value.clone())?;
        Ok(value)
    }

    /// Returns the debug storage origin for a minimal `Uint8Array` object.
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

    fn create_uint8_array_with_buffer(&mut self, buffer: ByteBuffer) -> Result<Value> {
        let buffer_object = self.create_array_buffer_object(buffer.clone())?;
        self.create_uint8_array_value(buffer, buffer_object)
    }

    fn create_array_buffer_value(&mut self, buffer: ByteBuffer) -> Result<Value> {
        self.create_array_buffer_object(buffer).map(Value::Object)
    }

    fn create_array_buffer_object(&mut self, buffer: ByteBuffer) -> Result<ObjectId> {
        let prototype = self.default_object_prototype()?;
        self.objects
            .create_array_buffer(buffer, prototype, self.limits.max_objects)
    }

    fn create_uint8_array_value(
        &mut self,
        buffer: ByteBuffer,
        buffer_object: ObjectId,
    ) -> Result<Value> {
        let length = buffer.byte_length();
        let view = Uint8ArrayView::new(buffer, buffer_object, 0, length);
        let prototype = self.default_object_prototype()?;
        self.objects
            .create_uint8_array(view, prototype, self.limits.max_objects)
            .map(Value::Object)
    }

    fn default_object_prototype(&mut self) -> Result<ObjectId> {
        let constructor_key = self.object_constructor_property_key()?;
        self.objects.object_prototype_id(
            constructor_key,
            self.limits.max_objects,
            self.limits.max_object_properties,
        )
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
}
