use std::{cell::RefCell, rc::Rc};

use crate::{
    error::{Error, Result},
    runtime::{abstract_operations::to_number_primitive, numeric::number_to_uint32},
    value::{ObjectId, Value},
};

use super::{Object, ObjectHeap};

const TYPED_ARRAY_INDEX_ERROR: &str = "typed array byte index is out of bounds";
const TYPED_ARRAY_RANGE_ERROR: &str = "typed array byte range exceeded supported range";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ByteBufferOrigin {
    EngineOwned,
    HostProvided,
}

#[derive(Debug, Clone)]
pub struct ByteBuffer {
    bytes: Rc<RefCell<Vec<u8>>>,
    origin: ByteBufferOrigin,
}

impl ByteBuffer {
    pub fn new(length: usize, origin: ByteBufferOrigin) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(vec![0; length])),
            origin,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>, origin: ByteBufferOrigin) -> Self {
        Self {
            bytes: Rc::new(RefCell::new(bytes)),
            origin,
        }
    }

    pub fn byte_length(&self) -> usize {
        self.bytes.borrow().len()
    }

    pub(in crate::runtime) fn read<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        let end = offset
            .checked_add(N)
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        let bytes = self.bytes.borrow();
        let Some(source) = bytes.get(offset..end) else {
            return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
        };
        let mut result = [0_u8; N];
        result.copy_from_slice(source);
        Ok(result)
    }

    pub(in crate::runtime) fn write(&self, offset: usize, value: &[u8]) -> Result<()> {
        let end = offset
            .checked_add(value.len())
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        let mut bytes = self.bytes.borrow_mut();
        let Some(target) = bytes.get_mut(offset..end) else {
            return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
        };
        target.copy_from_slice(value);
        Ok(())
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
}

impl TypedArrayElementKind {
    pub(in crate::runtime) const ALL: [Self; 9] = [
        Self::Int8,
        Self::Uint8,
        Self::Uint8Clamped,
        Self::Int16,
        Self::Uint16,
        Self::Int32,
        Self::Uint32,
        Self::Float32,
        Self::Float64,
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
        }
    }

    pub(in crate::runtime) const fn bytes_per_element(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 | Self::Uint8Clamped => 1,
            Self::Int16 | Self::Uint16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 => 8,
        }
    }

    pub(in crate::runtime) fn read(self, buffer: &ByteBuffer, offset: usize) -> Result<f64> {
        let value = match self {
            Self::Int8 => f64::from(i8::from_ne_bytes(buffer.read::<1>(offset)?)),
            Self::Uint8 | Self::Uint8Clamped => {
                f64::from(u8::from_ne_bytes(buffer.read::<1>(offset)?))
            }
            Self::Int16 => f64::from(i16::from_ne_bytes(buffer.read::<2>(offset)?)),
            Self::Uint16 => f64::from(u16::from_ne_bytes(buffer.read::<2>(offset)?)),
            Self::Int32 => f64::from(i32::from_ne_bytes(buffer.read::<4>(offset)?)),
            Self::Uint32 => f64::from(u32::from_ne_bytes(buffer.read::<4>(offset)?)),
            Self::Float32 => f64::from(f32::from_ne_bytes(buffer.read::<4>(offset)?)),
            Self::Float64 => f64::from_ne_bytes(buffer.read::<8>(offset)?),
        };
        Ok(value)
    }

    pub(in crate::runtime) fn write(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        number: f64,
    ) -> Result<()> {
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
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypedArrayView {
    buffer: ByteBuffer,
    buffer_object: ObjectId,
    byte_offset: usize,
    length: usize,
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
            element_kind,
        }
    }

    pub const fn length(&self) -> usize {
        self.length
    }

    pub fn byte_length(&self) -> Result<usize> {
        self.length
            .checked_mul(self.element_kind.bytes_per_element())
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))
    }

    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub const fn buffer_object(&self) -> ObjectId {
        self.buffer_object
    }

    pub(in crate::runtime) const fn element_kind(&self) -> TypedArrayElementKind {
        self.element_kind
    }

    pub fn read(&self, index: usize) -> Result<Option<f64>> {
        let Some(absolute) = self.element_offset(index)? else {
            return Ok(None);
        };
        self.element_kind.read(&self.buffer, absolute).map(Some)
    }

    pub fn write(&self, index: usize, number: f64) -> Result<bool> {
        let Some(absolute) = self.element_offset(index)? else {
            return Ok(false);
        };
        self.element_kind.write(&self.buffer, absolute, number)?;
        Ok(true)
    }

    fn element_offset(&self, index: usize) -> Result<Option<usize>> {
        if index >= self.length {
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

    pub(crate) fn typed_array(&self, id: ObjectId) -> Result<Option<TypedArrayView>> {
        Ok(self.object(id)?.typed_array.clone())
    }

    pub(crate) fn typed_array_number(&self, id: ObjectId, index: usize) -> Result<Option<f64>> {
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(None);
        };
        view.read(index)
    }

    pub(crate) fn set_typed_array_number(
        &self,
        id: ObjectId,
        index: usize,
        number: f64,
    ) -> Result<bool> {
        let Some(view) = self.object(id)?.typed_array.as_ref() else {
            return Ok(false);
        };
        view.write(index, number)
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

pub fn typed_array_number(value: &Value) -> Result<f64> {
    to_number_primitive(value)
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
