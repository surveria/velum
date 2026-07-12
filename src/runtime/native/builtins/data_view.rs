use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::to_boolean,
        call::RuntimeCallArgs,
        native::{
            DATA_VIEW_NAME, DataViewFunctionKind, NativeFunctionKind, OBJECT_CONSTRUCTOR_PROPERTY,
        },
        object::{
            AccessorPropertyUpdate, DataPropertyUpdate, DataViewElementKind, DataViewView,
            ObjectPropertyInit, PropertyConfigurable, PropertyEnumerable, PropertyKey,
            PropertyUpdate, PropertyWritable, TypedArrayContentType,
        },
    },
    value::{ErrorName, ObjectId, Value},
};

const DATA_VIEW_BUFFER_PROPERTY: &str = "buffer";
const DATA_VIEW_BYTE_LENGTH_PROPERTY: &str = "byteLength";
const DATA_VIEW_BYTE_OFFSET_PROPERTY: &str = "byteOffset";
const DATA_VIEW_BUFFER_TYPE_ERROR: &str = "DataView buffer must be an ArrayBuffer";
const DATA_VIEW_RECEIVER_TYPE_ERROR: &str = "DataView method receiver is not a DataView";
const DATA_VIEW_RANGE_ERROR: &str = "DataView range exceeds its ArrayBuffer";
const DATA_VIEW_LENGTH_LIMIT_ERROR: &str = "DataView length exceeded supported range";
const SYMBOL_TO_STRING_TAG_PROPERTY: &str = "toStringTag";
const SYMBOL_TO_STRING_TAG_DISPLAY: &str = "[Symbol.toStringTag]";

impl Context {
    pub(in crate::runtime::native) fn data_view_constructor_value(&mut self) -> Result<Value> {
        let constructor_kind = NativeFunctionKind::DataView(DataViewFunctionKind::Constructor);
        if let Some(id) = self.native_function_id(constructor_kind) {
            return Ok(Value::NativeFunction(id));
        }
        self.object_constructor_value()?;
        let id = self.next_native_function_id();
        let constructor = Value::NativeFunction(id);
        let prototype = self.data_view_prototype_with_constructor(constructor.clone())?;
        let name = self.native_function_name_value(constructor_kind)?;
        self.push_native_function_with_id(id, constructor_kind, Value::Object(prototype), name)?;
        self.install_data_view_prototype(prototype)?;
        self.insert_global_builtin(DATA_VIEW_NAME, constructor.clone())?;
        Ok(constructor)
    }

    pub(in crate::runtime) fn construct_data_view(
        &mut self,
        args: RuntimeCallArgs<'_>,
    ) -> Result<Value> {
        let values = args.as_slice();
        let Some(Value::Object(buffer_object)) = values.first() else {
            return Err(Error::type_error(DATA_VIEW_BUFFER_TYPE_ERROR));
        };
        let Some(buffer) = self.objects.array_buffer(*buffer_object)? else {
            return Err(Error::type_error(DATA_VIEW_BUFFER_TYPE_ERROR));
        };
        let byte_offset =
            Self::length_to_usize(self.to_index(values.get(1))?, DATA_VIEW_LENGTH_LIMIT_ERROR)?;
        let Some(available) = buffer.byte_length().checked_sub(byte_offset) else {
            return Err(Error::exception(
                ErrorName::RangeError,
                DATA_VIEW_RANGE_ERROR,
            ));
        };
        let byte_length = if values
            .get(2)
            .is_some_and(|value| !matches!(value, Value::Undefined))
        {
            let requested =
                Self::length_to_usize(self.to_index(values.get(2))?, DATA_VIEW_LENGTH_LIMIT_ERROR)?;
            if requested > available {
                return Err(Error::exception(
                    ErrorName::RangeError,
                    DATA_VIEW_RANGE_ERROR,
                ));
            }
            requested
        } else {
            available
        };
        let prototype = self.data_view_constructor_prototype()?;
        let view = DataViewView::new(buffer, *buffer_object, byte_offset, byte_length);
        self.objects
            .create_data_view(view, prototype, self.limits.max_objects)
            .map(Value::Object)
    }

    pub(in crate::runtime::native) fn eval_data_view_native_function_kind(
        &mut self,
        kind: DataViewFunctionKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        match kind {
            DataViewFunctionKind::Constructor => {
                Err(Error::type_error("DataView constructor requires 'new'"))
            }
            DataViewFunctionKind::BufferGetter => self
                .data_view_receiver(this_value)
                .map(|view| Value::Object(view.buffer_object())),
            DataViewFunctionKind::ByteLengthGetter => {
                let view = self.data_view_receiver(this_value)?;
                Self::data_view_usize_value(view.byte_length()?)
            }
            DataViewFunctionKind::ByteOffsetGetter => {
                let view = self.data_view_receiver(this_value)?;
                Self::data_view_usize_value(view.byte_offset()?)
            }
            DataViewFunctionKind::Get(element_kind) => {
                self.eval_data_view_get(element_kind, args, this_value)
            }
            DataViewFunctionKind::Set(element_kind) => {
                self.eval_data_view_set(element_kind, args, this_value)
            }
        }
    }

    fn eval_data_view_get(
        &mut self,
        element_kind: DataViewElementKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let view = self.data_view_receiver(this_value)?;
        let values = args.as_slice();
        let byte_offset =
            Self::length_to_usize(self.to_index(values.first())?, DATA_VIEW_LENGTH_LIMIT_ERROR)?;
        let little_endian = values.get(1).is_some_and(to_boolean);
        view.read(element_kind, byte_offset, little_endian)
    }

    fn eval_data_view_set(
        &mut self,
        element_kind: DataViewElementKind,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        let view = self.data_view_receiver(this_value)?;
        let values = args.as_slice();
        let byte_offset =
            Self::length_to_usize(self.to_index(values.first())?, DATA_VIEW_LENGTH_LIMIT_ERROR)?;
        let value = values.get(1).unwrap_or(&Value::Undefined);
        let element = match element_kind.content_type() {
            TypedArrayContentType::Number => Value::Number(self.to_number(value)?),
            TypedArrayContentType::BigInt => Value::BigInt(self.to_bigint(value)?),
        };
        let little_endian = values.get(2).is_some_and(to_boolean);
        view.write(element_kind, byte_offset, &element, little_endian)?;
        Ok(Value::Undefined)
    }

    fn data_view_receiver(&self, this_value: &Value) -> Result<DataViewView> {
        let Value::Object(id) = this_value else {
            return Err(Error::type_error(DATA_VIEW_RECEIVER_TYPE_ERROR));
        };
        self.objects
            .data_view(*id)?
            .ok_or_else(|| Error::type_error(DATA_VIEW_RECEIVER_TYPE_ERROR))
    }

    fn data_view_prototype_with_constructor(&mut self, constructor: Value) -> Result<ObjectId> {
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

    fn data_view_constructor_prototype(&mut self) -> Result<ObjectId> {
        let Value::NativeFunction(id) = self.data_view_constructor_value()? else {
            return Err(Error::runtime("DataView constructor value is not native"));
        };
        match self.native_function(id)?.properties().prototype() {
            Value::Object(id) => Ok(id),
            _ => Err(Error::runtime("DataView prototype is not an object")),
        }
    }

    fn install_data_view_prototype(&mut self, prototype: ObjectId) -> Result<()> {
        for (name, kind) in [
            (
                DATA_VIEW_BUFFER_PROPERTY,
                DataViewFunctionKind::BufferGetter,
            ),
            (
                DATA_VIEW_BYTE_LENGTH_PROPERTY,
                DataViewFunctionKind::ByteLengthGetter,
            ),
            (
                DATA_VIEW_BYTE_OFFSET_PROPERTY,
                DataViewFunctionKind::ByteOffsetGetter,
            ),
        ] {
            self.define_data_view_accessor(prototype, name, kind)?;
        }
        for element_kind in DataViewElementKind::ALL {
            self.define_data_view_method(
                prototype,
                element_kind.get_name(),
                DataViewFunctionKind::Get(element_kind),
            )?;
            self.define_data_view_method(
                prototype,
                element_kind.set_name(),
                DataViewFunctionKind::Set(element_kind),
            )?;
        }
        self.define_data_view_to_string_tag(prototype)
    }

    fn define_data_view_accessor(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: DataViewFunctionKind,
    ) -> Result<()> {
        let getter =
            self.create_native_function(NativeFunctionKind::DataView(kind), Value::Undefined)?;
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

    fn define_data_view_method(
        &mut self,
        prototype: ObjectId,
        name: &str,
        kind: DataViewFunctionKind,
    ) -> Result<()> {
        let method =
            self.create_native_function(NativeFunctionKind::DataView(kind), Value::Undefined)?;
        self.define_non_enumerable_object_property(prototype, name, method)
    }

    fn define_data_view_to_string_tag(&mut self, prototype: ObjectId) -> Result<()> {
        let symbol_constructor = self.symbol_constructor_value()?;
        let tag = self.get_named(&symbol_constructor, SYMBOL_TO_STRING_TAG_PROPERTY)?;
        let Value::Symbol(tag) = tag else {
            return Err(Error::runtime("Symbol.toStringTag is not initialized"));
        };
        let value = self.heap_string_value(DATA_VIEW_NAME)?;
        self.objects.define_property(
            prototype,
            PropertyKey::symbol(tag.id()),
            SYMBOL_TO_STRING_TAG_DISPLAY,
            PropertyUpdate::Data(DataPropertyUpdate::new(
                Some(value),
                Some(PropertyWritable::No),
                Some(PropertyEnumerable::No),
                Some(PropertyConfigurable::Yes),
            )),
            self.limits.max_object_properties,
        )
    }

    fn data_view_usize_value(value: usize) -> Result<Value> {
        let value = u32::try_from(value).map_err(|_| Error::limit(DATA_VIEW_LENGTH_LIMIT_ERROR))?;
        Ok(Value::Number(f64::from(value)))
    }
}
