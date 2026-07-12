use std::{cell::RefCell, rc::Rc, sync::Arc};

use parking_lot::RwLock;

use crate::{
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{to_bigint_primitive, to_number_primitive},
        numeric::number_to_uint32,
    },
    value::{JsBigInt, ObjectId, Value, format_ecmascript_number},
};

use super::{Object, ObjectHeap};

const TYPED_ARRAY_INDEX_ERROR: &str = "typed array byte index is out of bounds";
const TYPED_ARRAY_RANGE_ERROR: &str = "typed array byte range exceeded supported range";
const DETACHED_BUFFER_ERROR: &str = "ArrayBuffer is detached";
const FIXED_BUFFER_ERROR: &str = "ArrayBuffer is not resizable";
const RESIZE_LIMIT_ERROR: &str = "ArrayBuffer resize exceeds maxByteLength";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ByteBufferOrigin {
    EngineOwned,
    HostProvided,
}

#[derive(Debug, Clone)]
pub struct ByteBuffer {
    storage: ByteBufferStorage,
    origin: ByteBufferOrigin,
}

#[derive(Debug, Clone)]
enum ByteBufferStorage {
    Local(Rc<RefCell<ByteBufferState>>),
    Shared(Arc<RwLock<ByteBufferState>>),
}

#[derive(Debug)]
struct ByteBufferState {
    bytes: Option<Vec<u8>>,
    max_byte_length: Option<usize>,
}

impl ByteBuffer {
    fn with_state<T>(&self, operation: impl FnOnce(&ByteBufferState) -> T) -> T {
        match &self.storage {
            ByteBufferStorage::Local(state) => operation(&state.borrow()),
            ByteBufferStorage::Shared(state) => operation(&state.read()),
        }
    }

    fn with_state_mut<T>(&self, operation: impl FnOnce(&mut ByteBufferState) -> T) -> T {
        match &self.storage {
            ByteBufferStorage::Local(state) => operation(&mut state.borrow_mut()),
            ByteBufferStorage::Shared(state) => operation(&mut state.write()),
        }
    }

    pub fn new(length: usize, origin: ByteBufferOrigin) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(vec![0; length]),
                max_byte_length: None,
            }))),
            origin,
        }
    }

    pub(in crate::runtime) fn new_resizable(length: usize, max_byte_length: usize) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(vec![0; length]),
                max_byte_length: Some(max_byte_length),
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub(in crate::runtime) fn from_resizable_bytes(bytes: Vec<u8>, max_byte_length: usize) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(bytes),
                max_byte_length: Some(max_byte_length),
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>, origin: ByteBufferOrigin) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(bytes),
                max_byte_length: None,
            }))),
            origin,
        }
    }

    pub(in crate::runtime) fn new_shared(length: usize, max_byte_length: Option<usize>) -> Self {
        Self {
            storage: ByteBufferStorage::Shared(Arc::new(RwLock::new(ByteBufferState {
                bytes: Some(vec![0; length]),
                max_byte_length,
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub fn byte_length(&self) -> usize {
        self.with_state(|state| state.bytes.as_ref().map_or(0, std::vec::Vec::len))
    }

    pub(in crate::runtime) fn max_byte_length(&self) -> usize {
        self.with_state(|state| {
            let Some(bytes) = state.bytes.as_ref() else {
                return 0;
            };
            state.max_byte_length.unwrap_or(bytes.len())
        })
    }

    pub(in crate::runtime) fn is_resizable(&self) -> bool {
        self.with_state(|state| state.max_byte_length.is_some())
    }

    pub(in crate::runtime) fn is_detached(&self) -> bool {
        self.with_state(|state| state.bytes.is_none())
    }

    pub(in crate::runtime) const fn is_shared(&self) -> bool {
        matches!(&self.storage, ByteBufferStorage::Shared(_))
    }

    pub(in crate::runtime) fn copy_bytes(&self, start: usize, end: usize) -> Result<Vec<u8>> {
        self.with_state(|state| {
            let Some(bytes) = state.bytes.as_ref() else {
                return Err(Error::type_error(DETACHED_BUFFER_ERROR));
            };
            bytes
                .get(start..end)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| Error::runtime(TYPED_ARRAY_INDEX_ERROR))
        })
    }

    pub(in crate::runtime) fn resize(&self, new_length: usize) -> Result<()> {
        self.with_state_mut(|state| {
            let Some(max_byte_length) = state.max_byte_length else {
                return Err(Error::type_error(FIXED_BUFFER_ERROR));
            };
            if new_length > max_byte_length {
                return Err(Error::exception(
                    crate::value::ErrorName::RangeError,
                    RESIZE_LIMIT_ERROR,
                ));
            }
            let Some(bytes) = state.bytes.as_mut() else {
                return Err(Error::type_error(DETACHED_BUFFER_ERROR));
            };
            bytes.resize(new_length, 0);
            Ok(())
        })
    }

    pub(in crate::runtime) fn detach(&self) -> Result<Vec<u8>> {
        if self.is_shared() {
            return Err(Error::type_error("SharedArrayBuffer cannot be detached"));
        }
        self.with_state_mut(|state| {
            state
                .bytes
                .take()
                .ok_or_else(|| Error::type_error(DETACHED_BUFFER_ERROR))
        })
    }

    pub(in crate::runtime) fn read<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        let end = offset
            .checked_add(N)
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        self.with_state(|state| {
            let Some(bytes) = state.bytes.as_ref() else {
                return Err(Error::type_error(DETACHED_BUFFER_ERROR));
            };
            let Some(source) = bytes.get(offset..end) else {
                return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
            };
            let mut result = [0_u8; N];
            result.copy_from_slice(source);
            Ok(result)
        })
    }

    pub(in crate::runtime) fn write(&self, offset: usize, value: &[u8]) -> Result<()> {
        let end = offset
            .checked_add(value.len())
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        self.with_state_mut(|state| {
            let Some(bytes) = state.bytes.as_mut() else {
                return Err(Error::type_error(DETACHED_BUFFER_ERROR));
            };
            let Some(target) = bytes.get_mut(offset..end) else {
                return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
            };
            target.copy_from_slice(value);
            Ok(())
        })
    }

    pub(in crate::runtime) fn with_exclusive_bytes_mut<T>(
        &self,
        operation: impl FnOnce(&mut [u8]) -> Result<T>,
    ) -> Result<T> {
        self.with_state_mut(|state| {
            let Some(bytes) = state.bytes.as_mut() else {
                return Err(Error::type_error(DETACHED_BUFFER_ERROR));
            };
            operation(bytes)
        })
    }

    pub const fn origin(&self) -> &ByteBufferOrigin {
        &self.origin
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum TypedArrayElementKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum TypedArrayContentType {
    Number,
    BigInt,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum TypedArrayPropertyIndex {
    Valid(usize),
    Invalid,
}

impl TypedArrayElementKind {
    pub(in crate::runtime) const ALL: [Self; 11] = [
        Self::Int8,
        Self::Uint8,
        Self::Uint8Clamped,
        Self::Int16,
        Self::Uint16,
        Self::Int32,
        Self::Uint32,
        Self::Float32,
        Self::Float64,
        Self::BigInt64,
        Self::BigUint64,
    ];

    pub(in crate::runtime) const fn name(self) -> &'static str {
        match self {
            Self::Int8 => "Int8Array",
            Self::Uint8 => "Uint8Array",
            Self::Uint8Clamped => "Uint8ClampedArray",
            Self::Int16 => "Int16Array",
            Self::Uint16 => "Uint16Array",
            Self::Int32 => "Int32Array",
            Self::Uint32 => "Uint32Array",
            Self::Float32 => "Float32Array",
            Self::Float64 => "Float64Array",
            Self::BigInt64 => "BigInt64Array",
            Self::BigUint64 => "BigUint64Array",
        }
    }

    pub(in crate::runtime) const fn bytes_per_element(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 | Self::Uint8Clamped => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 | Self::BigInt64 | Self::BigUint64 => 8,
        }
    }

    pub(in crate::runtime) const fn content_type(self) -> TypedArrayContentType {
        match self {
            Self::BigInt64 | Self::BigUint64 => TypedArrayContentType::BigInt,
            Self::Int8
            | Self::Uint8
            | Self::Uint8Clamped
            | Self::Int16
            | Self::Uint16
            | Self::Int32
            | Self::Uint32
            | Self::Float32
            | Self::Float64 => TypedArrayContentType::Number,
        }
    }

    pub(in crate::runtime) fn read(self, buffer: &ByteBuffer, offset: usize) -> Result<Value> {
        let value = match self {
            Self::Int8 => Value::Number(f64::from(i8::from_ne_bytes(buffer.read::<1>(offset)?))),
            Self::Uint8 | Self::Uint8Clamped => {
                Value::Number(f64::from(u8::from_ne_bytes(buffer.read::<1>(offset)?)))
            }
            Self::Int16 => Value::Number(f64::from(i16::from_ne_bytes(buffer.read::<2>(offset)?))),
            Self::Uint16 => Value::Number(f64::from(u16::from_ne_bytes(buffer.read::<2>(offset)?))),
            Self::Int32 => Value::Number(f64::from(i32::from_ne_bytes(buffer.read::<4>(offset)?))),
            Self::Uint32 => Value::Number(f64::from(u32::from_ne_bytes(buffer.read::<4>(offset)?))),
            Self::Float32 => {
                Value::Number(f64::from(f32::from_ne_bytes(buffer.read::<4>(offset)?)))
            }
            Self::Float64 => Value::Number(f64::from_ne_bytes(buffer.read::<8>(offset)?)),
            Self::BigInt64 => Value::BigInt(JsBigInt::from_i64(i64::from_ne_bytes(
                buffer.read::<8>(offset)?,
            ))),
            Self::BigUint64 => Value::BigInt(JsBigInt::from_u64(u64::from_ne_bytes(
                buffer.read::<8>(offset)?,
            ))),
        };
        Ok(value)
    }

    pub(in crate::runtime) fn write(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        value: &Value,
    ) -> Result<()> {
        if self.content_type() == TypedArrayContentType::BigInt {
            let bigint = to_bigint_primitive(value)?;
            return match self {
                Self::BigInt64 => {
                    let Some(value) = bigint.as_int_n(64).to_i64() else {
                        return Err(Error::runtime("BigInt64 conversion overflowed"));
                    };
                    buffer.write(offset, &value.to_ne_bytes())
                }
                Self::BigUint64 => {
                    let Some(value) = bigint.as_uint_n(64).to_u64() else {
                        return Err(Error::runtime("BigUint64 conversion overflowed"));
                    };
                    buffer.write(offset, &value.to_ne_bytes())
                }
                Self::Int8
                | Self::Uint8
                | Self::Uint8Clamped
                | Self::Int16
                | Self::Uint16
                | Self::Int32
                | Self::Uint32
                | Self::Float32
                | Self::Float64 => Err(Error::runtime(
                    "BigInt typed array content type did not match its element kind",
                )),
            };
        }
        let number = to_number_primitive(value)?;
        match self {
            Self::Int8 => buffer.write(offset, &to_int8(number)?.to_ne_bytes()),
            Self::Uint8 => buffer.write(offset, &to_uint8(number)?.to_ne_bytes()),
            Self::Uint8Clamped => buffer.write(offset, &to_uint8_clamp(number).to_ne_bytes()),
            Self::Int16 => buffer.write(offset, &to_int16(number)?.to_ne_bytes()),
            Self::Uint16 => buffer.write(offset, &to_uint16(number)?.to_ne_bytes()),
            Self::Int32 => buffer.write(offset, &to_int32(number)?.to_ne_bytes()),
            Self::Uint32 => buffer.write(
                offset,
                &number_to_uint32(number, self.name())?.to_ne_bytes(),
            ),
            Self::Float32 => buffer.write(offset, &to_float32(number).to_ne_bytes()),
            Self::Float64 => buffer.write(offset, &number.to_ne_bytes()),
            Self::BigInt64 | Self::BigUint64 => Err(Error::runtime(
                "Number typed array content type did not match its element kind",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypedArrayView {
    buffer: ByteBuffer,
    buffer_object: ObjectId,
    byte_offset: usize,
    length: usize,
    length_tracking: bool,
    element_kind: TypedArrayElementKind,
}

impl TypedArrayView {
    pub(in crate::runtime) const fn new(
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: usize,
        length: usize,
        element_kind: TypedArrayElementKind,
    ) -> Self {
        Self {
            buffer,
            buffer_object,
            byte_offset,
            length,
            length_tracking: false,
            element_kind,
        }
    }

    pub(in crate::runtime) const fn new_length_tracking(
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: usize,
        element_kind: TypedArrayElementKind,
    ) -> Self {
        Self {
            buffer,
            buffer_object,
            byte_offset,
            length: 0,
            length_tracking: true,
            element_kind,
        }
    }

    pub fn length(&self) -> usize {
        if self.is_out_of_bounds() {
            return 0;
        }
        if self.length_tracking {
            return self.buffer.byte_length().saturating_sub(self.byte_offset)
                / self.element_kind.bytes_per_element();
        }
        self.length
    }

    pub fn byte_length(&self) -> Result<usize> {
        self.length()
            .checked_mul(self.element_kind.bytes_per_element())
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))
    }

    pub fn byte_offset(&self) -> usize {
        if self.is_out_of_bounds() {
            return 0;
        }
        self.byte_offset
    }

    pub(in crate::runtime) const fn raw_view_slots(&self) -> (usize, bool) {
        (self.byte_offset, self.length_tracking)
    }
    pub const fn buffer_object(&self) -> ObjectId {
        self.buffer_object
    }

    pub(in crate::runtime) const fn buffer(&self) -> &ByteBuffer {
        &self.buffer
    }

    pub(in crate::runtime) const fn element_kind(&self) -> TypedArrayElementKind {
        self.element_kind
    }

    pub fn read(&self, index: usize) -> Result<Option<Value>> {
        let Some(absolute) = self.element_offset(index)? else {
            return Ok(None);
        };
        self.element_kind.read(&self.buffer, absolute).map(Some)
    }

    pub fn write(&self, index: usize, value: &Value) -> Result<bool> {
        let Some(absolute) = self.element_offset(index)? else {
            return Ok(false);
        };
        self.element_kind.write(&self.buffer, absolute, value)?;
        Ok(true)
    }

    pub(in crate::runtime) fn element_offset(&self, index: usize) -> Result<Option<usize>> {
        if index >= self.length() {
            return Ok(None);
        }
        let relative = index
            .checked_mul(self.element_kind.bytes_per_element())
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        self.byte_offset
            .checked_add(relative)
            .map(Some)
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))
    }

    pub(in crate::runtime) fn is_out_of_bounds(&self) -> bool {
        if self.buffer.is_detached() {
            return true;
        }
        if self.length_tracking {
            return self.byte_offset > self.buffer.byte_length();
        }
        let Some(byte_length) = self
            .length
            .checked_mul(self.element_kind.bytes_per_element())
        else {
            return true;
        };
        self.byte_offset
            .checked_add(byte_length)
            .is_none_or(|end| end > self.buffer.byte_length())
    }
}

impl ObjectHeap {
    pub(crate) fn create_array_buffer(
        &mut self,
        buffer: ByteBuffer,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.byte_buffer = Some(buffer);
        self.push_object(object, max_objects)
    }

    pub(crate) fn create_typed_array(
        &mut self,
        view: TypedArrayView,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.typed_array = Some(view);
        self.push_object(object, max_objects)
    }

    pub(crate) fn array_buffer(&self, id: ObjectId) -> Result<Option<ByteBuffer>> {
        Ok(self.object(id)?.byte_buffer.clone())
    }

    pub(in crate::runtime) fn is_array_buffer_view(&self, id: ObjectId) -> Result<bool> {
        let object = self.object(id)?;
        Ok(object.typed_array.is_some() || object.data_view.is_some())
    }

    pub(in crate::runtime) fn resize_array_buffer(
        &mut self,
        id: ObjectId,
        new_length: usize,
    ) -> Result<()> {
        let buffer = self.array_buffer(id)?.ok_or_else(|| {
            Error::type_error("ArrayBuffer method receiver is not an ArrayBuffer")
        })?;
        if buffer.is_detached() {
            return Err(Error::type_error(DETACHED_BUFFER_ERROR));
        }
        if !buffer.is_resizable() {
            return Err(Error::type_error(FIXED_BUFFER_ERROR));
        }
        if new_length > buffer.max_byte_length() {
            return Err(Error::exception(
                crate::value::ErrorName::RangeError,
                RESIZE_LIMIT_ERROR,
            ));
        }
        let old_length = buffer.byte_length();
        let projected = if new_length >= old_length {
            self.byte_buffer_payload_bytes
                .checked_add(new_length - old_length)
        } else {
            self.byte_buffer_payload_bytes
                .checked_sub(old_length - new_length)
        }
        .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        if projected
            > self
                .storage_limits
                .max_payload_bytes(crate::runtime::VmStorageKind::ByteBuffer)
        {
            return Err(Error::limit(RESIZE_LIMIT_ERROR));
        }
        buffer.resize(new_length)?;
        self.byte_buffer_payload_bytes = projected;
        Ok(())
    }

    pub(in crate::runtime) fn grow_shared_array_buffer(
        &mut self,
        id: ObjectId,
        new_length: usize,
    ) -> Result<()> {
        let buffer = self.array_buffer(id)?.ok_or_else(|| {
            Error::type_error("SharedArrayBuffer method receiver is not a SharedArrayBuffer")
        })?;
        if !buffer.is_shared() {
            return Err(Error::type_error(
                "SharedArrayBuffer method receiver is not a SharedArrayBuffer",
            ));
        }
        if !buffer.is_resizable() {
            return Err(Error::type_error("SharedArrayBuffer is not growable"));
        }
        let old_length = buffer.byte_length();
        if new_length < old_length || new_length > buffer.max_byte_length() {
            return Err(Error::exception(
                crate::value::ErrorName::RangeError,
                "SharedArrayBuffer grow length is out of range",
            ));
        }
        if new_length == old_length {
            return Ok(());
        }
        let projected = self
            .byte_buffer_payload_bytes
            .checked_add(new_length - old_length)
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        if projected
            > self
                .storage_limits
                .max_payload_bytes(crate::runtime::VmStorageKind::ByteBuffer)
        {
            return Err(Error::limit(RESIZE_LIMIT_ERROR));
        }
        buffer.resize(new_length)?;
        self.byte_buffer_payload_bytes = projected;
        Ok(())
    }

    pub(in crate::runtime) fn detach_array_buffer(&mut self, id: ObjectId) -> Result<Vec<u8>> {
        let buffer = self.array_buffer(id)?.ok_or_else(|| {
            Error::type_error("ArrayBuffer method receiver is not an ArrayBuffer")
        })?;
        let old_length = buffer.byte_length();
        let projected = self
            .byte_buffer_payload_bytes
            .checked_sub(old_length)
            .ok_or_else(|| Error::runtime("byte buffer payload accounting underflowed"))?;
        let bytes = buffer.detach()?;
        self.byte_buffer_payload_bytes = projected;
        Ok(bytes)
    }

    pub(crate) fn typed_array(&self, id: ObjectId) -> Result<Option<TypedArrayView>> {
        Ok(self.object(id)?.typed_array.clone())
    }

    pub(in crate::runtime) fn typed_array_property_index(
        &self,
        id: ObjectId,
        property: &str,
    ) -> Result<Option<TypedArrayPropertyIndex>> {
        if typed_array_property_index(property, usize::MAX).is_none() {
            return Ok(None);
        }
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(None);
        };
        Ok(typed_array_property_index(property, view.length()))
    }

    pub(crate) fn typed_array_rejects_numeric_property(
        &self,
        id: ObjectId,
        property: &str,
    ) -> Result<bool> {
        if typed_array_property_index(property, usize::MAX).is_none() {
            return Ok(false);
        }
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(false);
        };
        Ok(matches!(
            typed_array_property_index(property, view.length()),
            Some(TypedArrayPropertyIndex::Invalid)
        ))
    }

    pub(crate) fn typed_array_value(&self, id: ObjectId, index: usize) -> Result<Option<Value>> {
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(None);
        };
        view.read(index)
    }

    pub(crate) fn set_typed_array_value(
        &self,
        id: ObjectId,
        index: usize,
        value: &Value,
    ) -> Result<bool> {
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(false);
        };
        view.write(index, value)
    }

    pub(crate) fn typed_array_debug_origin(
        &self,
        id: ObjectId,
    ) -> Result<Option<&ByteBufferOrigin>> {
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(None);
        };
        Ok(Some(view.buffer.origin()))
    }
}

impl Context {
    pub(crate) fn detach_host_array_buffer(&mut self, id: ObjectId) -> Result<()> {
        self.objects.detach_array_buffer(id).map(drop)
    }
}

pub(in crate::runtime::object) fn typed_array_property_index(
    property: &str,
    length: usize,
) -> Option<TypedArrayPropertyIndex> {
    if property == "-0" {
        return Some(TypedArrayPropertyIndex::Invalid);
    }
    let number = match property {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => property.parse::<f64>().ok()?,
    };
    if format_ecmascript_number(number) != property {
        return None;
    }
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 {
        return Some(TypedArrayPropertyIndex::Invalid);
    }
    let Ok(index) = property.parse::<usize>() else {
        return Some(TypedArrayPropertyIndex::Invalid);
    };
    Some(if index < length {
        TypedArrayPropertyIndex::Valid(index)
    } else {
        TypedArrayPropertyIndex::Invalid
    })
}

pub(super) fn to_int8(value: f64) -> Result<i8> {
    let unsigned = to_uint8(value)?;
    Ok(i8::from_ne_bytes(unsigned.to_ne_bytes()))
}

pub(super) fn to_uint8(value: f64) -> Result<u8> {
    let unsigned = number_to_uint32(value, "typed array Uint8 conversion")? % 256;
    u8::try_from(unsigned).map_err(|_| Error::runtime("typed array Uint8 conversion overflowed"))
}

pub(super) fn to_int16(value: f64) -> Result<i16> {
    let unsigned = to_uint16(value)?;
    Ok(i16::from_ne_bytes(unsigned.to_ne_bytes()))
}

pub(super) fn to_uint16(value: f64) -> Result<u16> {
    let unsigned = number_to_uint32(value, "typed array Uint16 conversion")? % 65_536;
    u16::try_from(unsigned).map_err(|_| Error::runtime("typed array Uint16 conversion overflowed"))
}

pub(super) fn to_int32(value: f64) -> Result<i32> {
    let unsigned = number_to_uint32(value, "typed array Int32 conversion")?;
    Ok(i32::from_ne_bytes(unsigned.to_ne_bytes()))
}

// Exact half-way detection is required by the ECMAScript ties-to-even algorithm.
#[allow(clippy::float_cmp)]
fn to_uint8_clamp(value: f64) -> u8 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    if value >= 255.0 {
        return 255;
    }
    let floor = value.floor();
    let fraction = value - floor;
    let rounded = if fraction > 0.5 || (fraction == 0.5 && floor % 2.0 != 0.0) {
        floor + 1.0
    } else {
        floor
    };
    rounded.to_string().parse::<u8>().unwrap_or(0)
}

// Rust's IEEE-754 narrowing cast implements the Float32Array conversion directly.
#[allow(clippy::cast_possible_truncation)]
pub(super) const fn to_float32(value: f64) -> f32 {
    value as f32
}
