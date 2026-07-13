use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, roots::VmRootKind},
    value::{ErrorName, Value},
};

const ARRAY_COPY_INDEX_ERROR: &str = "array index exceeded supported range";
const ARRAY_WITH_RANGE_ERROR: &str = "Array.prototype.with index out of range";

impl Context {
    pub(in crate::runtime::native) fn eval_array_to_reversed(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_to_reversed(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_to_reversed(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::eval_array_discard_args(args);
        Self::ensure_array_like_object(this_value)?;
        if let Some(value) = self.eval_packed_array_to_reversed(this_value)? {
            return Ok(value);
        }
        let length = self.array_like_length(this_value)?;
        let result = self.create_intrinsic_array_with_length(length)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        for offset in 0..length {
            self.step()?;
            let from = length
                .checked_sub(offset)
                .and_then(|value| value.checked_sub(1))
                .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
            let value = self.get_array_like_index(this_value, from)?;
            self.array_from_create_data_property(&result, offset, value)?;
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_array_to_spliced(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_to_spliced(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_to_spliced(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let start = self.array_slice_bound(args.first(), length, 0)?;
        let (skip_count, items) = self.array_splice_counts(args, length, start)?;
        let new_length = Self::array_spliced_length(length, skip_count, items.len())?;
        if let Some(value) =
            self.eval_packed_array_to_spliced(this_value, start, skip_count, &items, new_length)?
        {
            return Ok(value);
        }
        let result = self.create_intrinsic_array_with_length(new_length)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        let mut write = 0_usize;
        for index in 0..start {
            self.step()?;
            let value = self.get_array_like_index(this_value, index)?;
            self.array_from_create_data_property(&result, write, value)?;
            write = write
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
        }
        for value in items {
            self.array_from_create_data_property(&result, write, value)?;
            write = write
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
        }
        let mut read = start
            .checked_add(skip_count)
            .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
        while write < new_length {
            self.step()?;
            let value = self.get_array_like_index(this_value, read)?;
            self.array_from_create_data_property(&result, write, value)?;
            read = read
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
            write = write
                .checked_add(1)
                .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
        }
        Ok(result)
    }

    fn eval_packed_array_to_reversed(&mut self, this_value: &Value) -> Result<Option<Value>> {
        let Some(mut values) = self.packed_default_array_copy_values(this_value)? else {
            return Ok(None);
        };
        self.charge_runtime_steps(values.len())?;
        values.reverse();
        self.create_array_from_elements(values).map(Some)
    }

    fn eval_packed_array_to_spliced(
        &mut self,
        this_value: &Value,
        start: usize,
        skip_count: usize,
        items: &[Value],
        new_length: usize,
    ) -> Result<Option<Value>> {
        let Some(values) = self.packed_default_array_copy_values(this_value)? else {
            return Ok(None);
        };
        let read = start
            .checked_add(skip_count)
            .ok_or_else(|| Error::limit(ARRAY_COPY_INDEX_ERROR))?;
        let Some(prefix) = values.get(0..start) else {
            return Ok(None);
        };
        let Some(tail) = values.get(read..) else {
            return Ok(None);
        };
        let mut elements = Vec::with_capacity(new_length);
        elements.extend_from_slice(prefix);
        elements.extend_from_slice(items);
        elements.extend_from_slice(tail);
        self.charge_runtime_steps(values.len())?;
        self.create_array_from_elements(elements).map(Some)
    }

    fn packed_default_array_copy_values(&self, value: &Value) -> Result<Option<Vec<Value>>> {
        let Value::Object(id) = value else {
            return Ok(None);
        };
        self.objects.packed_default_array_values_if_array(*id)
    }

    pub(in crate::runtime::native) fn eval_array_with(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_with(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_with(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let relative = self.to_integer_or_infinity(args.first().unwrap_or(&Value::Undefined))?;
        let length_f64 = Self::usize_to_number(length, ARRAY_COPY_INDEX_ERROR)?;
        let target = if relative >= 0.0 {
            relative
        } else {
            length_f64 + relative
        };
        if target < 0.0 || target >= length_f64 {
            return Err(Error::exception(
                ErrorName::RangeError,
                ARRAY_WITH_RANGE_ERROR,
            ));
        }
        let actual = Self::array_clamp_index(target, length)?;
        let value = args.get(1).cloned().unwrap_or(Value::Undefined);
        let result = self.create_intrinsic_array_with_length(length)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        for index in 0..length {
            self.step()?;
            let copied = if index == actual {
                value.clone()
            } else {
                self.get_array_like_index(this_value, index)?
            };
            self.array_from_create_data_property(&result, index, copied)?;
        }
        Ok(result)
    }
}
