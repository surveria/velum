use crate::{
    error::{Error, Result},
    runtime::{Context, native::TypedArrayFunctionKind, object::TypedArrayView, roots::VmRootKind},
    value::Value,
};

const SOURCE_OUT_OF_BOUNDS_ERROR: &str = "TypedArray slice source is out of bounds";
const RESULT_OUT_OF_BOUNDS_ERROR: &str = "TypedArray slice result became out of bounds";
const COPY_RANGE_ERROR: &str = "TypedArray slice byte range exceeded supported range";

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
            self.typed_array_species_create_with_length(this_value, count)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
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
        let source_start = source
            .byte_offset()
            .checked_add(relative_start)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let count_bytes = count
            .checked_mul(bytes_per_element)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let target_start = target.byte_offset();
        for byte_offset in 0..count_bytes {
            self.step()?;
            let source_index = source_start
                .checked_add(byte_offset)
                .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
            let target_index = target_start
                .checked_add(byte_offset)
                .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
            let byte = source.buffer().read::<1>(source_index)?;
            target.buffer().write(target_index, &byte)?;
        }
        Ok(())
    }

    fn eval_typed_array_copy_within_record(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let record = self.typed_array_view_record(this_value)?;
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
        if backward {
            for offset in (0..count).rev() {
                self.step()?;
                Self::copy_typed_array_element(&record.view, start, target, offset)?;
            }
        } else {
            for offset in 0..count {
                self.step()?;
                Self::copy_typed_array_element(&record.view, start, target, offset)?;
            }
        }
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
        for lower in 0..record.length / 2 {
            self.step()?;
            let upper = record
                .length
                .checked_sub(lower)
                .and_then(|index| index.checked_sub(1))
                .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
            let lower_bytes = Self::typed_array_element_bytes(&record.view, lower)?;
            let upper_bytes = Self::typed_array_element_bytes(&record.view, upper)?;
            Self::write_typed_array_element_bytes(&record.view, lower, &upper_bytes)?;
            Self::write_typed_array_element_bytes(&record.view, upper, &lower_bytes)?;
        }
        Ok(this_value.clone())
    }

    fn copy_typed_array_element(
        view: &TypedArrayView,
        source: usize,
        target: usize,
        offset: usize,
    ) -> Result<()> {
        let source = source
            .checked_add(offset)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let target = target
            .checked_add(offset)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        let bytes = Self::typed_array_element_bytes(view, source)?;
        Self::write_typed_array_element_bytes(view, target, &bytes)
    }

    fn typed_array_element_bytes(view: &TypedArrayView, index: usize) -> Result<Vec<u8>> {
        let start = Self::typed_array_element_byte_offset(view, index)?;
        let end = start
            .checked_add(view.element_kind().bytes_per_element())
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        view.buffer().copy_bytes(start, end)
    }

    fn write_typed_array_element_bytes(
        view: &TypedArrayView,
        index: usize,
        bytes: &[u8],
    ) -> Result<()> {
        let start = Self::typed_array_element_byte_offset(view, index)?;
        view.buffer().write(start, bytes)
    }

    fn typed_array_element_byte_offset(view: &TypedArrayView, index: usize) -> Result<usize> {
        let relative = index
            .checked_mul(view.element_kind().bytes_per_element())
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))?;
        view.byte_offset()
            .checked_add(relative)
            .ok_or_else(|| Error::limit(COPY_RANGE_ERROR))
    }
}
