use crate::{
    error::{Error, Result},
    runtime::abstract_operations::{to_bigint_primitive, to_number_primitive},
    value::{JsBigInt, Value},
};

use super::{
    ByteBuffer, DETACHED_BUFFER_ERROR, TYPED_ARRAY_INDEX_ERROR, TYPED_ARRAY_RANGE_ERROR,
    TypedArrayContentType, TypedArrayElementKind, TypedArrayView, number_to_uint32, to_float32,
    to_int8, to_int16, to_int32, to_uint8, to_uint8_clamp, to_uint16,
};

const MAX_ELEMENT_BYTES: usize = 8;

#[derive(Debug, Clone, Copy)]
pub(in crate::runtime) struct TypedArrayElementBytes {
    bytes: [u8; MAX_ELEMENT_BYTES],
    length: usize,
}

impl TypedArrayElementBytes {
    fn from_slice(source: &[u8]) -> Result<Self> {
        let mut bytes = [0_u8; MAX_ELEMENT_BYTES];
        let Some(target) = bytes.get_mut(..source.len()) else {
            return Err(Error::runtime(
                "typed array element encoding exceeded eight bytes",
            ));
        };
        target.copy_from_slice(source);
        Ok(Self {
            bytes,
            length: source.len(),
        })
    }

    fn as_slice(&self) -> Result<&[u8]> {
        self.bytes
            .get(..self.length)
            .ok_or_else(|| Error::runtime("typed array element encoding length is invalid"))
    }
}

impl ByteBuffer {
    pub(in crate::runtime) fn shares_storage(&self, other: &Self) -> bool {
        match (&self.storage, &other.storage) {
            (super::ByteBufferStorage::Local(left), super::ByteBufferStorage::Local(right)) => {
                std::rc::Rc::ptr_eq(left, right)
            }
            (super::ByteBufferStorage::Shared(left), super::ByteBufferStorage::Shared(right)) => {
                std::sync::Arc::ptr_eq(left, right)
            }
            (super::ByteBufferStorage::Local(_), super::ByteBufferStorage::Shared(_))
            | (super::ByteBufferStorage::Shared(_), super::ByteBufferStorage::Local(_)) => false,
        }
    }

    fn with_bytes<T>(&self, operation: impl FnOnce(&[u8]) -> Result<T>) -> Result<T> {
        self.with_state(|state| {
            let Some(bytes) = state.bytes.as_ref() else {
                return Err(Error::type_error(DETACHED_BUFFER_ERROR));
            };
            operation(bytes)
        })
    }

    pub(in crate::runtime) fn copy_bytes(&self, start: usize, end: usize) -> Result<Vec<u8>> {
        self.with_bytes(|bytes| {
            bytes
                .get(start..end)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| Error::runtime(TYPED_ARRAY_INDEX_ERROR))
        })
    }

    pub(in crate::runtime) fn read<const N: usize>(&self, offset: usize) -> Result<[u8; N]> {
        self.with_bytes(|bytes| read_array(bytes, offset))
    }

    pub(in crate::runtime) fn write(&self, offset: usize, value: &[u8]) -> Result<()> {
        self.with_exclusive_bytes_mut(|bytes| write_slice(bytes, offset, value))
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
}

impl TypedArrayElementKind {
    pub(in crate::runtime) fn read(self, buffer: &ByteBuffer, offset: usize) -> Result<Value> {
        buffer.with_bytes(|bytes| self.read_from_bytes(bytes, offset))
    }

    pub(in crate::runtime) fn read_from_bytes(self, bytes: &[u8], offset: usize) -> Result<Value> {
        let value = match self {
            Self::Int8 => Value::Number(f64::from(i8::from_ne_bytes(read_array(bytes, offset)?))),
            Self::Uint8 | Self::Uint8Clamped => {
                Value::Number(f64::from(u8::from_ne_bytes(read_array(bytes, offset)?)))
            }
            Self::Int16 => Value::Number(f64::from(i16::from_ne_bytes(read_array(bytes, offset)?))),
            Self::Uint16 => {
                Value::Number(f64::from(u16::from_ne_bytes(read_array(bytes, offset)?)))
            }
            Self::Int32 => Value::Number(f64::from(i32::from_ne_bytes(read_array(bytes, offset)?))),
            Self::Uint32 => {
                Value::Number(f64::from(u32::from_ne_bytes(read_array(bytes, offset)?)))
            }
            Self::Float32 => {
                Value::Number(f64::from(f32::from_ne_bytes(read_array(bytes, offset)?)))
            }
            Self::Float64 => Value::Number(f64::from_ne_bytes(read_array(bytes, offset)?)),
            Self::BigInt64 => Value::BigInt(JsBigInt::from_i64(i64::from_ne_bytes(read_array(
                bytes, offset,
            )?))),
            Self::BigUint64 => Value::BigInt(JsBigInt::from_u64(u64::from_ne_bytes(read_array(
                bytes, offset,
            )?))),
        };
        Ok(value)
    }

    pub(in crate::runtime) fn encode(self, value: &Value) -> Result<TypedArrayElementBytes> {
        if self.content_type() == TypedArrayContentType::BigInt {
            let bigint = to_bigint_primitive(value)?;
            return match self {
                Self::BigInt64 => {
                    let Some(value) = bigint.as_int_n(64).to_i64() else {
                        return Err(Error::runtime("BigInt64 conversion overflowed"));
                    };
                    TypedArrayElementBytes::from_slice(&value.to_ne_bytes())
                }
                Self::BigUint64 => {
                    let Some(value) = bigint.as_uint_n(64).to_u64() else {
                        return Err(Error::runtime("BigUint64 conversion overflowed"));
                    };
                    TypedArrayElementBytes::from_slice(&value.to_ne_bytes())
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
            Self::Int8 => TypedArrayElementBytes::from_slice(&to_int8(number)?.to_ne_bytes()),
            Self::Uint8 => TypedArrayElementBytes::from_slice(&to_uint8(number)?.to_ne_bytes()),
            Self::Uint8Clamped => {
                TypedArrayElementBytes::from_slice(&to_uint8_clamp(number).to_ne_bytes())
            }
            Self::Int16 => TypedArrayElementBytes::from_slice(&to_int16(number)?.to_ne_bytes()),
            Self::Uint16 => TypedArrayElementBytes::from_slice(&to_uint16(number)?.to_ne_bytes()),
            Self::Int32 => TypedArrayElementBytes::from_slice(&to_int32(number)?.to_ne_bytes()),
            Self::Uint32 => TypedArrayElementBytes::from_slice(
                &number_to_uint32(number, self.name())?.to_ne_bytes(),
            ),
            Self::Float32 => TypedArrayElementBytes::from_slice(&to_float32(number).to_ne_bytes()),
            Self::Float64 => TypedArrayElementBytes::from_slice(&number.to_ne_bytes()),
            Self::BigInt64 | Self::BigUint64 => Err(Error::runtime(
                "Number typed array content type did not match its element kind",
            )),
        }
    }

    pub(in crate::runtime) fn write(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        value: &Value,
    ) -> Result<()> {
        let encoded = self.encode(value)?;
        buffer.with_exclusive_bytes_mut(|bytes| self.write_encoded(bytes, offset, &encoded))
    }

    pub(in crate::runtime) fn write_encoded(
        self,
        bytes: &mut [u8],
        offset: usize,
        encoded: &TypedArrayElementBytes,
    ) -> Result<()> {
        let source = encoded.as_slice()?;
        if source.len() != self.bytes_per_element() {
            return Err(Error::runtime(
                "typed array encoded element width did not match its kind",
            ));
        }
        write_slice(bytes, offset, source)
    }
}

impl TypedArrayView {
    pub(in crate::runtime) fn with_bytes<T>(
        &self,
        operation: impl FnOnce(&[u8]) -> Result<T>,
    ) -> Result<T> {
        let (start, end) = self.byte_range()?;
        self.buffer.with_bytes(|bytes| {
            let Some(view_bytes) = bytes.get(start..end) else {
                return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
            };
            operation(view_bytes)
        })
    }

    pub(in crate::runtime) fn with_bytes_mut<T>(
        &self,
        operation: impl FnOnce(&mut [u8]) -> Result<T>,
    ) -> Result<T> {
        let (start, end) = self.byte_range()?;
        self.buffer.with_exclusive_bytes_mut(|bytes| {
            let Some(view_bytes) = bytes.get_mut(start..end) else {
                return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
            };
            operation(view_bytes)
        })
    }

    fn byte_range(&self) -> Result<(usize, usize)> {
        if self.is_out_of_bounds() {
            return Err(Error::type_error(DETACHED_BUFFER_ERROR));
        }
        let byte_length = self.byte_length()?;
        let end = self
            .byte_offset
            .checked_add(byte_length)
            .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
        Ok((self.byte_offset, end))
    }
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
    let Some(source) = bytes.get(offset..end) else {
        return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
    };
    let mut result = [0_u8; N];
    result.copy_from_slice(source);
    Ok(result)
}

fn write_slice(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or_else(|| Error::limit(TYPED_ARRAY_RANGE_ERROR))?;
    let Some(target) = bytes.get_mut(offset..end) else {
        return Err(Error::runtime(TYPED_ARRAY_INDEX_ERROR));
    };
    target.copy_from_slice(value);
    Ok(())
}
