use crate::{
    error::{Error, Result},
    runtime::abstract_operations::{to_bigint_primitive, to_number_primitive},
    runtime::numeric::number_to_uint32,
    value::{ErrorName, JsBigInt, ObjectId, Value},
};

use super::{
    ByteBuffer, Object, ObjectHeap, TypedArrayContentType,
    typed_array::{to_float32, to_int8, to_int16, to_int32, to_uint8, to_uint16},
};

const DATA_VIEW_RANGE_ERROR: &str = "DataView byte offset is outside the view";
const DATA_VIEW_OFFSET_LIMIT_ERROR: &str = "DataView byte offset exceeded supported range";
const DATA_VIEW_FLOAT16_CONVERSION_ERROR: &str = "DataView Float16 conversion overflowed";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(in crate::runtime) enum DataViewElementKind {
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float16,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
}

impl DataViewElementKind {
    pub(in crate::runtime) const ALL: [Self; 11] = [
        Self::Int8,
        Self::Uint8,
        Self::Int16,
        Self::Uint16,
        Self::Int32,
        Self::Uint32,
        Self::Float16,
        Self::Float32,
        Self::Float64,
        Self::BigInt64,
        Self::BigUint64,
    ];

    pub(in crate::runtime) const fn index(self) -> usize {
        match self {
            Self::Int8 => 0,
            Self::Uint8 => 1,
            Self::Int16 => 2,
            Self::Uint16 => 3,
            Self::Int32 => 4,
            Self::Uint32 => 5,
            Self::Float16 => 6,
            Self::Float32 => 7,
            Self::Float64 => 8,
            Self::BigInt64 => 9,
            Self::BigUint64 => 10,
        }
    }

    pub(in crate::runtime) const fn get_name(self) -> &'static str {
        match self {
            Self::Int8 => "getInt8",
            Self::Uint8 => "getUint8",
            Self::Int16 => "getInt16",
            Self::Uint16 => "getUint16",
            Self::Int32 => "getInt32",
            Self::Uint32 => "getUint32",
            Self::Float16 => "getFloat16",
            Self::Float32 => "getFloat32",
            Self::Float64 => "getFloat64",
            Self::BigInt64 => "getBigInt64",
            Self::BigUint64 => "getBigUint64",
        }
    }

    pub(in crate::runtime) const fn set_name(self) -> &'static str {
        match self {
            Self::Int8 => "setInt8",
            Self::Uint8 => "setUint8",
            Self::Int16 => "setInt16",
            Self::Uint16 => "setUint16",
            Self::Int32 => "setInt32",
            Self::Uint32 => "setUint32",
            Self::Float16 => "setFloat16",
            Self::Float32 => "setFloat32",
            Self::Float64 => "setFloat64",
            Self::BigInt64 => "setBigInt64",
            Self::BigUint64 => "setBigUint64",
        }
    }

    const fn byte_width(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 => 1,
            Self::Int16 | Self::Uint16 | Self::Float16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 | Self::BigInt64 | Self::BigUint64 => 8,
        }
    }

    pub(in crate::runtime) const fn content_type(self) -> TypedArrayContentType {
        match self {
            Self::BigInt64 | Self::BigUint64 => TypedArrayContentType::BigInt,
            Self::Int8
            | Self::Uint8
            | Self::Int16
            | Self::Uint16
            | Self::Int32
            | Self::Uint32
            | Self::Float16
            | Self::Float32
            | Self::Float64 => TypedArrayContentType::Number,
        }
    }

    fn read(self, buffer: &ByteBuffer, offset: usize, little_endian: bool) -> Result<Value> {
        let value = match self {
            Self::Int8 => Value::Number(f64::from(i8::from_ne_bytes(buffer.read::<1>(offset)?))),
            Self::Uint8 => Value::Number(f64::from(u8::from_ne_bytes(buffer.read::<1>(offset)?))),
            Self::Int16 => Value::Number(f64::from(if little_endian {
                i16::from_le_bytes(buffer.read::<2>(offset)?)
            } else {
                i16::from_be_bytes(buffer.read::<2>(offset)?)
            })),
            Self::Uint16 => Value::Number(f64::from(if little_endian {
                u16::from_le_bytes(buffer.read::<2>(offset)?)
            } else {
                u16::from_be_bytes(buffer.read::<2>(offset)?)
            })),
            Self::Int32 => Value::Number(f64::from(if little_endian {
                i32::from_le_bytes(buffer.read::<4>(offset)?)
            } else {
                i32::from_be_bytes(buffer.read::<4>(offset)?)
            })),
            Self::Uint32 => Value::Number(f64::from(if little_endian {
                u32::from_le_bytes(buffer.read::<4>(offset)?)
            } else {
                u32::from_be_bytes(buffer.read::<4>(offset)?)
            })),
            Self::Float16 => {
                let bits = if little_endian {
                    u16::from_le_bytes(buffer.read::<2>(offset)?)
                } else {
                    u16::from_be_bytes(buffer.read::<2>(offset)?)
                };
                Value::Number(binary16_to_f64(bits))
            }
            Self::Float32 => Value::Number(f64::from(if little_endian {
                f32::from_le_bytes(buffer.read::<4>(offset)?)
            } else {
                f32::from_be_bytes(buffer.read::<4>(offset)?)
            })),
            Self::Float64 => Value::Number(if little_endian {
                f64::from_le_bytes(buffer.read::<8>(offset)?)
            } else {
                f64::from_be_bytes(buffer.read::<8>(offset)?)
            }),
            Self::BigInt64 => {
                let integer = if little_endian {
                    i64::from_le_bytes(buffer.read::<8>(offset)?)
                } else {
                    i64::from_be_bytes(buffer.read::<8>(offset)?)
                };
                Value::BigInt(JsBigInt::from_i64(integer))
            }
            Self::BigUint64 => {
                let integer = if little_endian {
                    u64::from_le_bytes(buffer.read::<8>(offset)?)
                } else {
                    u64::from_be_bytes(buffer.read::<8>(offset)?)
                };
                Value::BigInt(JsBigInt::from_u64(integer))
            }
        };
        Ok(value)
    }

    fn write(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        value: &Value,
        little_endian: bool,
    ) -> Result<()> {
        if self.content_type() == TypedArrayContentType::BigInt {
            return self.write_bigint(buffer, offset, value, little_endian);
        }
        self.write_number(buffer, offset, value, little_endian)
    }

    fn write_bigint(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        value: &Value,
        little_endian: bool,
    ) -> Result<()> {
        let bigint = to_bigint_primitive(value)?;
        match self {
            Self::BigInt64 => {
                let Some(integer) = bigint.as_int_n(64).to_i64() else {
                    return Err(Error::runtime("DataView BigInt64 conversion overflowed"));
                };
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    integer.to_le_bytes(),
                    integer.to_be_bytes(),
                )
            }
            Self::BigUint64 => {
                let Some(integer) = bigint.as_uint_n(64).to_u64() else {
                    return Err(Error::runtime("DataView BigUint64 conversion overflowed"));
                };
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    integer.to_le_bytes(),
                    integer.to_be_bytes(),
                )
            }
            Self::Int8
            | Self::Uint8
            | Self::Int16
            | Self::Uint16
            | Self::Int32
            | Self::Uint32
            | Self::Float16
            | Self::Float32
            | Self::Float64 => Err(Error::runtime(
                "BigInt DataView content type did not match its element kind",
            )),
        }
    }

    fn write_number(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        value: &Value,
        little_endian: bool,
    ) -> Result<()> {
        let number = to_number_primitive(value)?;
        match self {
            Self::Int8 => buffer.write(offset, &to_int8(number)?.to_ne_bytes()),
            Self::Uint8 => buffer.write(offset, &to_uint8(number)?.to_ne_bytes()),
            Self::Int16 => {
                let value = to_int16(number)?;
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    value.to_le_bytes(),
                    value.to_be_bytes(),
                )
            }
            Self::Uint16 => {
                let value = to_uint16(number)?;
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    value.to_le_bytes(),
                    value.to_be_bytes(),
                )
            }
            Self::Int32 => {
                let value = to_int32(number)?;
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    value.to_le_bytes(),
                    value.to_be_bytes(),
                )
            }
            Self::Uint32 => {
                let value = number_to_uint32(number, "DataView Uint32 conversion")?;
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    value.to_le_bytes(),
                    value.to_be_bytes(),
                )
            }
            Self::Float16 => {
                let value = f64_to_binary16(number)?;
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    value.to_le_bytes(),
                    value.to_be_bytes(),
                )
            }
            Self::Float32 => {
                let value = to_float32(number);
                write_endian(
                    buffer,
                    offset,
                    little_endian,
                    value.to_le_bytes(),
                    value.to_be_bytes(),
                )
            }
            Self::Float64 => write_endian(
                buffer,
                offset,
                little_endian,
                number.to_le_bytes(),
                number.to_be_bytes(),
            ),
            Self::BigInt64 | Self::BigUint64 => Err(Error::runtime(
                "Number DataView content type did not match its element kind",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct DataViewView {
    buffer: ByteBuffer,
    buffer_object: ObjectId,
    byte_offset: usize,
    byte_length: Option<usize>,
}

impl DataViewView {
    pub(in crate::runtime) const fn new(
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: usize,
        byte_length: Option<usize>,
    ) -> Self {
        Self {
            buffer,
            buffer_object,
            byte_offset,
            byte_length,
        }
    }

    pub(in crate::runtime) const fn buffer_object(&self) -> ObjectId {
        self.buffer_object
    }

    pub(in crate::runtime) fn byte_offset(&self) -> Result<usize> {
        self.ensure_in_bounds()?;
        Ok(self.byte_offset)
    }

    pub(in crate::runtime) fn byte_length(&self) -> Result<usize> {
        self.ensure_in_bounds()?;
        if let Some(byte_length) = self.byte_length {
            return Ok(byte_length);
        }
        Ok(self.buffer.byte_length().saturating_sub(self.byte_offset))
    }

    pub(in crate::runtime) fn read(
        &self,
        kind: DataViewElementKind,
        offset: usize,
        little_endian: bool,
    ) -> Result<Value> {
        let absolute = self.element_offset(offset, kind.byte_width())?;
        kind.read(&self.buffer, absolute, little_endian)
    }

    pub(in crate::runtime) fn write(
        &self,
        kind: DataViewElementKind,
        offset: usize,
        value: &Value,
        little_endian: bool,
    ) -> Result<()> {
        let absolute = self.element_offset(offset, kind.byte_width())?;
        kind.write(&self.buffer, absolute, value, little_endian)
    }

    pub(in crate::runtime) fn ensure_mutable(&self) -> Result<()> {
        self.buffer.ensure_mutable()
    }

    fn element_offset(&self, offset: usize, width: usize) -> Result<usize> {
        self.ensure_in_bounds()?;
        let relative_end = offset
            .checked_add(width)
            .ok_or_else(|| Error::limit(DATA_VIEW_OFFSET_LIMIT_ERROR))?;
        if relative_end > self.byte_length()? {
            return Err(Error::exception(
                ErrorName::RangeError,
                DATA_VIEW_RANGE_ERROR,
            ));
        }
        self.byte_offset
            .checked_add(offset)
            .ok_or_else(|| Error::limit(DATA_VIEW_OFFSET_LIMIT_ERROR))
    }

    fn ensure_in_bounds(&self) -> Result<()> {
        if self.buffer.is_detached() {
            return Err(Error::type_error("DataView buffer is detached"));
        }
        if let Some(byte_length) = self.byte_length {
            let Some(end) = self.byte_offset.checked_add(byte_length) else {
                return Err(Error::limit(DATA_VIEW_OFFSET_LIMIT_ERROR));
            };
            if end > self.buffer.byte_length() {
                return Err(Error::type_error("DataView is out of bounds"));
            }
        } else if self.byte_offset > self.buffer.byte_length() {
            return Err(Error::type_error("DataView is out of bounds"));
        }
        Ok(())
    }
}

impl ObjectHeap {
    pub(in crate::runtime) fn create_data_view(
        &mut self,
        view: DataViewView,
        prototype: ObjectId,
        max_objects: usize,
    ) -> Result<ObjectId> {
        let mut object = Object::ordinary();
        object.prototype = Some(prototype);
        object.data_view = Some(view);
        self.push_object(object, max_objects)
    }

    pub(in crate::runtime) fn data_view(&self, id: ObjectId) -> Result<Option<DataViewView>> {
        Ok(self.object(id)?.data_view.clone())
    }
}

fn write_endian<const N: usize>(
    buffer: &ByteBuffer,
    offset: usize,
    little_endian: bool,
    little: [u8; N],
    big: [u8; N],
) -> Result<()> {
    buffer.write(offset, if little_endian { &little } else { &big })
}

fn binary16_to_f64(bits: u16) -> f64 {
    let sign = if bits & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = u32::from((bits >> 10) & 0x1f);
    let fraction = u32::from(bits & 0x03ff);
    match exponent {
        0 if fraction == 0 => sign * 0.0,
        0 => sign * f64::from(fraction) * 2.0_f64.powi(-24),
        31 if fraction == 0 => sign * f64::INFINITY,
        31 => f64::NAN,
        _ => {
            let significand = 1.0 + f64::from(fraction) / 1024.0;
            let Ok(exponent) = i32::try_from(exponent) else {
                return f64::NAN;
            };
            let exponent = exponent - 15;
            sign * significand * 2.0_f64.powi(exponent)
        }
    }
}

fn f64_to_binary16(value: f64) -> Result<u16> {
    let bits = value.to_bits();
    let sign = if bits >> 63 == 0 { 0_u16 } else { 0x8000_u16 };
    let exponent_bits = (bits >> 52) & 0x07ff;
    let fraction = bits & 0x000f_ffff_ffff_ffff;
    if exponent_bits == 0x07ff {
        return Ok(if fraction == 0 {
            sign | 0x7c00
        } else {
            sign | 0x7e00
        });
    }
    if exponent_bits == 0 {
        return Ok(sign);
    }
    let exponent = i32::try_from(exponent_bits)
        .map_err(|_| Error::runtime(DATA_VIEW_FLOAT16_CONVERSION_ERROR))?
        - 1023;
    let significand = (1_u64 << 52) | fraction;
    let encoded = if exponent >= -14 {
        let rounded = round_right_ties_even(significand, 42);
        let mut half_exponent = exponent + 15;
        let mut half_significand = rounded;
        if half_significand == 2048 {
            half_exponent = half_exponent.saturating_add(1);
            half_significand = 1024;
        }
        if half_exponent >= 31 {
            0x7c00_u64
        } else {
            let exponent_field = u64::try_from(half_exponent)
                .map_err(|_| Error::runtime(DATA_VIEW_FLOAT16_CONVERSION_ERROR))?;
            (exponent_field << 10) | half_significand.saturating_sub(1024)
        }
    } else {
        let shift = u32::try_from(28_i32.saturating_sub(exponent))
            .map_err(|_| Error::runtime(DATA_VIEW_FLOAT16_CONVERSION_ERROR))?;
        round_right_ties_even(significand, shift)
    };
    let encoded =
        u16::try_from(encoded).map_err(|_| Error::runtime(DATA_VIEW_FLOAT16_CONVERSION_ERROR))?;
    Ok(sign | encoded)
}

const fn round_right_ties_even(value: u64, shift: u32) -> u64 {
    if shift == 0 {
        return value;
    }
    if shift >= u64::BITS {
        return 0;
    }
    let retained = value >> shift;
    let mask = (1_u64 << shift).saturating_sub(1);
    let discarded = value & mask;
    let halfway = 1_u64 << shift.saturating_sub(1);
    if discarded > halfway || (discarded == halfway && retained % 2 == 1) {
        retained.saturating_add(1)
    } else {
        retained
    }
}
