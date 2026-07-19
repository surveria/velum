#[cfg(not(feature = "std"))]
use crate::prelude::*;

use crate::{
    error::{Error, Result},
    runtime::{Context, native::TypedArrayFunctionKind, object::TypedArrayView, roots::VmRootKind},
    value::{ErrorName, Value},
};

const SOURCE_OUT_OF_BOUNDS_ERROR: &str = "TypedArray slice source is out of bounds";
const RESULT_OUT_OF_BOUNDS_ERROR: &str = "TypedArray slice result became out of bounds";
const COPY_RANGE_ERROR: &str = "TypedArray slice byte range exceeded supported range";
const WITH_INDEX_ERROR: &str = "TypedArray.prototype.with index is out of range";

impl Context {
    pub(super) fn eval_typed_array_copy_mutation_kind(
        &mut self,
        kind: TypedArrayFunctionKind,
        args: &[Value],
        this_value: &Value,
    ) -> Option<Result<Value>> {
        match kind {
            TypedArrayFunctionKind::CopyWithin => {
                Some(self.eval_typed_array_copy_within_record(args, this_value))
            }
            TypedArrayFunctionKind::Reverse => {
                Some(self.eval_typed_array_reverse_record(args, this_value))
            }
            _ => None,
        }
    }

    pub(super) fn eval_typed_array_to_reversed_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if !args.is_empty() {
            return Err(Error::runtime(
                "TypedArray.prototype.toReversed received routed arguments",
            ));
        }
        let record = self.typed_array_view_record(this_value)?;
        let mut values = Vec::with_capacity(record.length);
        for index in (0..record.length).rev() {
            self.step()?;
            values.push(record.value(index)?);
        }
        self.create_typed_array_from_values(record.view.element_kind(), values)
    }

    pub(super) fn eval_typed_array_subarray_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_branded_view_record(this_value)?;
        let (raw_byte_offset, length_tracking) = record.view.raw_view_slots();
        let begin = self.typed_array_relative_index(args.first(), record.length, 0)?;
        let explicit_end = args
            .get(1)
            .filter(|value| !matches!(value, Value::Undefined));
        let end = if let Some(end) = explicit_end {
            Some(self.typed_array_relative_index(Some(end), record.length, record.length)?)
        } else if length_tracking {
            None
        } else {
            Some(record.length)
        };
        let relative_bytes = begin
            .checked_mul(record.view.element_kind().bytes_per_element())
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let byte_offset = raw_byte_offset
            .checked_add(relative_bytes)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let mut constructor_args = Vec::with_capacity(3);
        constructor_args.push(Value::Object(record.view.buffer_object()));
        constructor_args.push(Self::typed_array_usize_value(byte_offset)?);
        if let Some(end) = end {
            constructor_args.push(Self::typed_array_usize_value(end.saturating_sub(begin))?);
        }
        self.typed_array_species_construct(this_value, &constructor_args)
    }

    pub(super) fn eval_typed_array_with_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        let relative = self.to_integer_or_infinity(args.first().unwrap_or(&Value::Undefined))?;
        let length_number = Self::typed_array_usize_number(record.length)?;
        let actual = if relative >= 0.0 {
            relative
        } else {
            length_number + relative
        };
        let value = args.get(1).unwrap_or(&Value::Undefined);
        let numeric_value =
            self.convert_typed_array_element_value(record.view.element_kind(), value)?;
        let Some(index) = Self::valid_current_typed_array_index(&record.view, actual)? else {
            return Err(Error::exception(ErrorName::RangeError, WITH_INDEX_ERROR));
        };
        let result =
            self.create_typed_array_with_length(record.view.element_kind(), record.length)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&result))?;
        let (result_id, _) = self.typed_array_receiver(&result)?;
        for current in 0..record.length {
            self.step()?;
            let element = if current == index {
                numeric_value.clone()
            } else {
                let value = record.value(current)?;
                self.convert_typed_array_element_value(record.view.element_kind(), &value)?
            };
            if !self
                .objects
                .set_typed_array_value(result_id, current, &element)?
            {
                return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
            }
        }
        Ok(result)
    }

    fn valid_current_typed_array_index(view: &TypedArrayView, index: f64) -> Result<Option<usize>> {
        if !index.is_finite() || index < 0.0 || view.is_out_of_bounds() {
            return Ok(None);
        }
        let index = Self::finite_nonnegative_integer_to_usize(index, COPY_RANGE_ERROR)?;
        if index >= view.length() {
            return Ok(None);
        }
        Ok(Some(index))
    }

    pub(super) fn eval_typed_array_slice_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        let start_index = self.typed_array_relative_index(args.first(), record.length, 0)?;
        let end_index =
            self.typed_array_relative_index(args.get(1), record.length, record.length)?;
        let count = end_index.saturating_sub(start_index);
        let (result, result_id, result_view) =
            self.typed_array_species_create_with_length(this_value, count, false)?;
        if result_view.buffer().is_immutable()
            && !record.view.buffer().shares_storage(result_view.buffer())
        {
            return Err(Error::type_error("ArrayBuffer is immutable"));
        }
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, core::iter::once(&result))?;
        if count == 0 {
            return Ok(result);
        }
        if record.view.is_out_of_bounds() {
            return Err(Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR));
        }
        let refreshed_end = end_index.min(record.view.length());
        let copy_count = refreshed_end.saturating_sub(start_index);
        if copy_count == 0 {
            return Ok(result);
        }
        if record.view.element_kind() == result_view.element_kind() {
            self.copy_typed_array_slice_bytes(&record.view, start_index, &result_view, copy_count)?;
            return Ok(result);
        }
        for offset in 0..copy_count {
            self.step()?;
            let source_index = start_index
                .checked_add(offset)
                .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
            let value = record
                .read(source_index)?
                .ok_or_else(|| Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR))?;
            let element =
                self.convert_typed_array_element_value(result_view.element_kind(), &value)?;
            if !self
                .objects
                .set_typed_array_value(result_id, offset, &element)?
            {
                return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
            }
        }
        Ok(result)
    }

    fn copy_typed_array_slice_bytes(
        &mut self,
        source: &TypedArrayView,
        start_index: usize,
        target: &TypedArrayView,
        count: usize,
    ) -> Result<()> {
        let bytes_per_element = source.element_kind().bytes_per_element();
        let relative_start = start_index
            .checked_mul(bytes_per_element)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let count_bytes = count
            .checked_mul(bytes_per_element)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        if source.buffer().shares_storage(target.buffer()) {
            let source_start = source
                .byte_offset()
                .checked_add(relative_start)
                .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
            let target_start = target.byte_offset();
            return source.buffer().with_slice_alias_bytes_mut(|bytes| {
                for byte_offset in 0..count_bytes {
                    self.step()?;
                    let source_index = source_start
                        .checked_add(byte_offset)
                        .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
                    let Some(byte) = bytes.get(source_index).copied() else {
                        return Err(Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR));
                    };
                    let target_index = target_start
                        .checked_add(byte_offset)
                        .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
                    let Some(destination) = bytes.get_mut(target_index) else {
                        return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
                    };
                    *destination = byte;
                }
                Ok(())
            });
        }
        let source_end = relative_start
            .checked_add(count_bytes)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let source_bytes = source.with_bytes(|bytes| {
            bytes
                .get(relative_start..source_end)
                .map(<[u8]>::to_vec)
                .ok_or_else(|| Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR))
        })?;
        target.with_bytes_mut(|target_bytes| {
            for (offset, byte) in source_bytes.iter().enumerate() {
                self.step()?;
                let Some(destination) = target_bytes.get_mut(offset) else {
                    return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
                };
                *destination = *byte;
            }
            Ok(())
        })
    }

    fn eval_typed_array_copy_within_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
        record.view.ensure_mutable()?;
        let target = self.typed_array_relative_index(args.first(), record.length, 0)?;
        let start = self.typed_array_relative_index(args.get(1), record.length, 0)?;
        let end = self.typed_array_relative_index(args.get(2), record.length, record.length)?;
        let initial_count = end
            .saturating_sub(start)
            .min(record.length.saturating_sub(target));
        if initial_count == 0 {
            return Ok(this_value.clone());
        }
        if record.view.is_out_of_bounds() {
            return Err(Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR));
        }
        let current_length = record.view.length();
        let count = initial_count
            .min(current_length.saturating_sub(start))
            .min(current_length.saturating_sub(target));
        if count == 0 {
            return Ok(this_value.clone());
        }
        let backward = start < target && target < start.saturating_add(count);
        let bytes_per_element = record.view.element_kind().bytes_per_element();
        record.view.with_bytes_mut(|bytes| {
            let mut scratch = vec![0_u8; bytes_per_element];
            if backward {
                for offset in (0..count).rev() {
                    self.step()?;
                    copy_typed_array_element(
                        bytes,
                        bytes_per_element,
                        start,
                        target,
                        offset,
                        &mut scratch,
                    )?;
                }
            } else {
                for offset in 0..count {
                    self.step()?;
                    copy_typed_array_element(
                        bytes,
                        bytes_per_element,
                        start,
                        target,
                        offset,
                        &mut scratch,
                    )?;
                }
            }
            Ok(())
        })?;
        Ok(this_value.clone())
    }

    fn eval_typed_array_reverse_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        if !args.is_empty() {
            return Err(Error::runtime(
                "TypedArray.prototype.reverse received routed arguments",
            ));
        }
        let record = self.typed_array_view_record(this_value)?;
        let bytes_per_element = record.view.element_kind().bytes_per_element();
        record.view.with_bytes_mut(|bytes| {
            let mut lower_copy = vec![0_u8; bytes_per_element];
            let mut upper_copy = vec![0_u8; bytes_per_element];
            for lower in 0..record.length / 2 {
                self.step()?;
                let upper = record
                    .length
                    .checked_sub(lower)
                    .and_then(|index| index.checked_sub(1))
                    .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
                let (lower_start, lower_end) = typed_array_element_range(lower, bytes_per_element)?;
                let (upper_start, upper_end) = typed_array_element_range(upper, bytes_per_element)?;
                let Some(lower_bytes) = bytes.get(lower_start..lower_end) else {
                    return Err(Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR));
                };
                lower_copy.copy_from_slice(lower_bytes);
                let Some(upper_bytes) = bytes.get(upper_start..upper_end) else {
                    return Err(Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR));
                };
                upper_copy.copy_from_slice(upper_bytes);
                let Some(lower_target) = bytes.get_mut(lower_start..lower_end) else {
                    return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
                };
                lower_target.copy_from_slice(&upper_copy);
                let Some(upper_target) = bytes.get_mut(upper_start..upper_end) else {
                    return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
                };
                upper_target.copy_from_slice(&lower_copy);
            }
            Ok(())
        })?;
        Ok(this_value.clone())
    }
}

fn copy_typed_array_element(
    bytes: &mut [u8],
    bytes_per_element: usize,
    source: usize,
    target: usize,
    offset: usize,
    scratch: &mut [u8],
) -> Result<()> {
    let source = source
        .checked_add(offset)
        .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
    let target = target
        .checked_add(offset)
        .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
    let (source_start, source_end) = typed_array_element_range(source, bytes_per_element)?;
    let Some(source_bytes) = bytes.get(source_start..source_end) else {
        return Err(Error::type_error(SOURCE_OUT_OF_BOUNDS_ERROR));
    };
    scratch.copy_from_slice(source_bytes);
    let (target_start, target_end) = typed_array_element_range(target, bytes_per_element)?;
    let Some(target_bytes) = bytes.get_mut(target_start..target_end) else {
        return Err(Error::type_error(RESULT_OUT_OF_BOUNDS_ERROR));
    };
    target_bytes.copy_from_slice(scratch);
    Ok(())
}

fn typed_array_element_range(index: usize, bytes_per_element: usize) -> Result<(usize, usize)> {
    let start = index
        .checked_mul(bytes_per_element)
        .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
    let end = start
        .checked_add(bytes_per_element)
        .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
    Ok((start, end))
}
