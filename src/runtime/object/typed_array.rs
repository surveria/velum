use std::{cell::RefCell, rc::Rc, sync::Arc, time::Duration};

use crate::{
    error::{Error, Result},
    runtime::{Context, numeric::number_to_uint32},
    value::{ObjectId, Value, format_ecmascript_number},
};

use super::{Object, ObjectHeap};

mod bulk;
pub(super) mod waiters;

use waiters::SharedByteBuffer;
pub(in crate::runtime) use waiters::{AtomicWaitOutcome, AtomicWaitRegistration};

const TYPED_ARRAY_INDEX_ERROR: &str = "typed array byte index is out of bounds";
const TYPED_ARRAY_RANGE_ERROR: &str = "typed array byte range exceeded supported range";
const DETACHED_BUFFER_ERROR: &str = "ArrayBuffer is detached";
const FIXED_BUFFER_ERROR: &str = "ArrayBuffer is not resizable";
const RESIZE_LIMIT_ERROR: &str = "ArrayBuffer resize exceeds maxByteLength";
const IMMUTABLE_BUFFER_ERROR: &str = "ArrayBuffer is immutable";

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
    Shared(Arc<SharedByteBuffer>),
}

#[derive(Debug)]
struct ByteBufferState {
    bytes: Option<Vec<u8>>,
    max_byte_length: Option<usize>,
    immutable: bool,
}

impl ByteBuffer {
    fn with_state<T>(&self, operation: impl FnOnce(&ByteBufferState) -> T) -> T {
        match &self.storage {
            ByteBufferStorage::Local(state) => operation(&state.borrow()),
            ByteBufferStorage::Shared(shared) => operation(&shared.state.read()),
        }
    }

    fn with_state_mut<T>(&self, operation: impl FnOnce(&mut ByteBufferState) -> T) -> T {
        match &self.storage {
            ByteBufferStorage::Local(state) => operation(&mut state.borrow_mut()),
            ByteBufferStorage::Shared(shared) => operation(&mut shared.state.write()),
        }
    }

    pub fn new(length: usize, origin: ByteBufferOrigin) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(vec![0; length]),
                max_byte_length: None,
                immutable: false,
            }))),
            origin,
        }
    }

    pub(in crate::runtime) fn new_resizable(length: usize, max_byte_length: usize) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(vec![0; length]),
                max_byte_length: Some(max_byte_length),
                immutable: false,
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub(in crate::runtime) fn from_resizable_bytes(bytes: Vec<u8>, max_byte_length: usize) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(bytes),
                max_byte_length: Some(max_byte_length),
                immutable: false,
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>, origin: ByteBufferOrigin) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(bytes),
                max_byte_length: None,
                immutable: false,
            }))),
            origin,
        }
    }

    pub(in crate::runtime) fn from_immutable_bytes(bytes: Vec<u8>) -> Self {
        Self {
            storage: ByteBufferStorage::Local(Rc::new(RefCell::new(ByteBufferState {
                bytes: Some(bytes),
                max_byte_length: None,
                immutable: true,
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub(in crate::runtime) fn new_shared(length: usize, max_byte_length: Option<usize>) -> Self {
        Self {
            storage: ByteBufferStorage::Shared(Arc::new(SharedByteBuffer::new(ByteBufferState {
                bytes: Some(vec![0; length]),
                max_byte_length,
                immutable: false,
            }))),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub(in crate::runtime) fn wait_at(
        &self,
        byte_offset: usize,
        timeout: Option<Duration>,
    ) -> Result<AtomicWaitOutcome> {
        let ByteBufferStorage::Shared(shared) = &self.storage else {
            return Err(Error::type_error("Atomics.wait requires shared storage"));
        };
        Ok(SharedByteBuffer::register_waiter(shared, byte_offset).wait(timeout))
    }

    pub(in crate::runtime) fn register_waiter_at(
        &self,
        byte_offset: usize,
    ) -> Result<AtomicWaitRegistration> {
        let ByteBufferStorage::Shared(shared) = &self.storage else {
            return Err(Error::type_error("Atomics.wait requires shared storage"));
        };
        Ok(SharedByteBuffer::register_waiter(shared, byte_offset))
    }

    pub(in crate::runtime) fn notify_at(&self, byte_offset: usize, count: usize) -> Result<usize> {
        let ByteBufferStorage::Shared(shared) = &self.storage else {
            return Ok(0);
        };
        shared.notify_at(byte_offset, count)
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

    pub(in crate::runtime) fn is_immutable(&self) -> bool {
        self.with_state(|state| state.immutable)
    }

    pub(in crate::runtime) fn ensure_mutable(&self) -> Result<()> {
        if self.is_immutable() {
            return Err(Error::type_error(IMMUTABLE_BUFFER_ERROR));
        }
        Ok(())
    }

    pub(crate) const fn is_shared(&self) -> bool {
        matches!(&self.storage, ByteBufferStorage::Shared(_))
    }

    pub(crate) fn shared_storage(&self) -> Option<Arc<SharedByteBuffer>> {
        let ByteBufferStorage::Shared(shared) = &self.storage else {
            return None;
        };
        Some(shared.clone())
    }

    pub(crate) const fn from_shared_storage(shared: Arc<SharedByteBuffer>) -> Self {
        Self {
            storage: ByteBufferStorage::Shared(shared),
            origin: ByteBufferOrigin::EngineOwned,
        }
    }

    pub(in crate::runtime) fn resize(&self, new_length: usize) -> Result<()> {
        self.with_state_mut(|state| {
            if state.immutable {
                return Err(Error::type_error(IMMUTABLE_BUFFER_ERROR));
            }
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
            if state.immutable {
                return Err(Error::type_error(IMMUTABLE_BUFFER_ERROR));
            }
            state
                .bytes
                .take()
                .ok_or_else(|| Error::type_error(DETACHED_BUFFER_ERROR))
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

    pub(in crate::runtime) fn ensure_mutable(&self) -> Result<()> {
        self.buffer.ensure_mutable()
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
        if self
            .objects
            .array_buffer(id)?
            .is_some_and(|buffer| buffer.is_detached())
        {
            return Ok(());
        }
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
    let bytes = property.as_bytes();
    if !bytes.is_empty() && bytes.iter().all(u8::is_ascii_digit) {
        if bytes.len() > 1 && bytes.first().is_some_and(|digit| *digit == b'0') {
            return None;
        }
        if let Ok(index) = property.parse::<usize>() {
            return Some(if index < length {
                TypedArrayPropertyIndex::Valid(index)
            } else {
                TypedArrayPropertyIndex::Invalid
            });
        }
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
