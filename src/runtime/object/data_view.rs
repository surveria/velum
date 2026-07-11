use crate::{
    error::{Error, Result},
    runtime::numeric::number_to_uint32,
    value::{ErrorName, ObjectId},
};

use super::{
    ByteBuffer, Object, ObjectHeap,
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
}

impl DataViewElementKind {
    pub(in crate::runtime) const ALL: [Self; 9] = [
        Self::Int8,
        Self::Uint8,
        Self::Int16,
        Self::Uint16,
        Self::Int32,
        Self::Uint32,
        Self::Float16,
        Self::Float32,
        Self::Float64,
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
        }
    }

    const fn byte_width(self) -> usize {
        match self {
            Self::Int8 | Self::Uint8 => 1,
            Self::Int16 | Self::Uint16 | Self::Float16 => 2,
            Self::Int32 | Self::Uint32 | Self::Float32 => 4,
            Self::Float64 => 8,
        }
    }

    fn read(self, buffer: &ByteBuffer, offset: usize, little_endian: bool) -> Result<f64> {
        let number = match self {
            Self::Int8 => f64::from(i8::from_ne_bytes(buffer.read::<1>(offset)?)),
            Self::Uint8 => f64::from(u8::from_ne_bytes(buffer.read::<1>(offset)?)),
            Self::Int16 => f64::from(if little_endian {
                i16::from_le_bytes(buffer.read::<2>(offset)?)
            } else {
                i16::from_be_bytes(buffer.read::<2>(offset)?)
            }),
            Self::Uint16 => f64::from(if little_endian {
                u16::from_le_bytes(buffer.read::<2>(offset)?)
            } else {
                u16::from_be_bytes(buffer.read::<2>(offset)?)
            }),
            Self::Int32 => f64::from(if little_endian {
                i32::from_le_bytes(buffer.read::<4>(offset)?)
            } else {
                i32::from_be_bytes(buffer.read::<4>(offset)?)
            }),
            Self::Uint32 => f64::from(if little_endian {
                u32::from_le_bytes(buffer.read::<4>(offset)?)
            } else {
                u32::from_be_bytes(buffer.read::<4>(offset)?)
            }),
            Self::Float16 => {
                let bits = if little_endian {
                    u16::from_le_bytes(buffer.read::<2>(offset)?)
                } else {
                    u16::from_be_bytes(buffer.read::<2>(offset)?)
                };
                binary16_to_f64(bits)
            }
            Self::Float32 => f64::from(if little_endian {
                f32::from_le_bytes(buffer.read::<4>(offset)?)
            } else {
                f32::from_be_bytes(buffer.read::<4>(offset)?)
            }),
            Self::Float64 => {
                if little_endian {
                    f64::from_le_bytes(buffer.read::<8>(offset)?)
                } else {
                    f64::from_be_bytes(buffer.read::<8>(offset)?)
                }
            }
        };
        Ok(number)
    }

    fn write(
        self,
        buffer: &ByteBuffer,
        offset: usize,
        number: f64,
        little_endian: bool,
    ) -> Result<()> {
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
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::runtime) struct DataViewView {
    buffer: ByteBuffer,
    buffer_object: ObjectId,
    byte_offset: usize,
    byte_length: usize,
}

impl DataViewView {
    pub(in crate::runtime) const fn new(
        buffer: ByteBuffer,
        buffer_object: ObjectId,
        byte_offset: usize,
        byte_length: usize,
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

    pub(in crate::runtime) const fn byte_offset(&self) -> usize {
        self.byte_offset
    }

    pub(in crate::runtime) const fn byte_length(&self) -> usize {
        self.byte_length
    }

    pub(in crate::runtime) fn read(
        &self,
        kind: DataViewElementKind,
        offset: usize,
        little_endian: bool,
    ) -> Result<f64> {
        let absolute = self.element_offset(offset, kind.byte_width())?;
        kind.read(&self.buffer, absolute, little_endian)
    }

    pub(in crate::runtime) fn write(
        &self,
        kind: DataViewElementKind,
        offset: usize,
        number: f64,
        little_endian: bool,
    ) -> Result<()> {
        let absolute = self.element_offset(offset, kind.byte_width())?;
        kind.write(&self.buffer, absolute, number, little_endian)
    }

    fn element_offset(&self, offset: usize, width: usize) -> Result<usize> {
        let relative_end = offset
            .checked_add(width)
            .ok_or_else(|| Error::limit(DATA_VIEW_OFFSET_LIMIT_ERROR))?;
        if relative_end > self.byte_length {
            return Err(Error::exception(
                ErrorName::RangeError,
                DATA_VIEW_RANGE_ERROR,
            ));
        }
        self.byte_offset
            .checked_add(offset)
            .ok_or_else(|| Error::limit(DATA_VIEW_OFFSET_LIMIT_ERROR))
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
