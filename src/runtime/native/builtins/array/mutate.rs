use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::Value,
};

const ARRAY_MUTATE_INDEX_ERROR: &str = "array index exceeded supported range";

impl Context {
    pub(in crate::runtime::native) fn eval_array_splice(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_splice(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_splice(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let start = Self::array_slice_bound(args.first(), length, 0)?;
        let (delete_count, items) = Self::array_splice_counts(args, length, start)?;
        if let Some(value) =
            self.eval_packed_array_splice(this_value, length, start, delete_count, &items)?
        {
            return Ok(value);
        }
        let removed = self.array_collect_removed(this_value, start, delete_count)?;
        self.array_splice_shift(this_value, length, start, delete_count, items.len())?;
        for (offset, item) in items.iter().enumerate() {
            self.step()?;
            let target = start
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
            self.set_array_like_index(this_value, target, item.clone())?;
        }
        let new_length = Self::array_spliced_length(length, delete_count, items.len())?;
        self.set_array_like_length(this_value, new_length)?;
        Ok(removed)
    }

    fn eval_packed_array_splice(
        &mut self,
        this_value: &Value,
        length: usize,
        start: usize,
        delete_count: usize,
        items: &[Value],
    ) -> Result<Option<Value>> {
        let Value::Object(id) = this_value else {
            return Ok(None);
        };
        let Some(removed_values) = self.objects.splice_packed_default_array_if_array(
            *id,
            start,
            delete_count,
            items,
            self.limits.max_object_properties,
        )?
        else {
            return Ok(None);
        };
        self.charge_runtime_steps(length)?;
        let removed = self.create_array_from_elements(removed_values)?;
        Ok(Some(removed))
    }

    /// Copy the deleted range `[start, start + delete_count)` into a fresh array.
    fn array_collect_removed(
        &mut self,
        this_value: &Value,
        start: usize,
        delete_count: usize,
    ) -> Result<Value> {
        let removed = self.create_array_callback_result(delete_count)?;
        for offset in 0..delete_count {
            self.step()?;
            let from = start
                .checked_add(offset)
                .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
            if self.has_array_like_index(this_value, from)? {
                let value = self.get_array_like_index(this_value, from)?;
                self.set_array_like_index(&removed, offset, value)?;
            }
        }
        Ok(removed)
    }

    /// Shift the tail elements to open or close the gap left by the splice.
    fn array_splice_shift(
        &mut self,
        this_value: &Value,
        length: usize,
        start: usize,
        delete_count: usize,
        item_count: usize,
    ) -> Result<()> {
        let tail = length
            .checked_sub(delete_count)
            .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
        if item_count < delete_count {
            for k in start..tail {
                self.step()?;
                let from = k
                    .checked_add(delete_count)
                    .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
                let to = k
                    .checked_add(item_count)
                    .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
                self.array_move_or_delete(this_value, from, to)?;
            }
            let new_length = Self::array_spliced_length(length, delete_count, item_count)?;
            for k in (new_length..length).rev() {
                self.step()?;
                self.delete_array_like_index(this_value, k)?;
            }
        } else if item_count > delete_count {
            for k in (start..tail).rev() {
                self.step()?;
                let from = k
                    .checked_add(delete_count)
                    .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
                let to = k
                    .checked_add(item_count)
                    .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
                self.array_move_or_delete(this_value, from, to)?;
            }
        }
        Ok(())
    }

    fn array_move_or_delete(&mut self, this_value: &Value, from: usize, to: usize) -> Result<()> {
        if self.has_array_like_index(this_value, from)? {
            let value = self.get_array_like_index(this_value, from)?;
            self.set_array_like_index(this_value, to, value)
        } else {
            self.delete_array_like_index(this_value, to)
        }
    }

    pub(super) fn array_splice_counts(
        args: &[Value],
        length: usize,
        start: usize,
    ) -> Result<(usize, Vec<Value>)> {
        let max_delete = length
            .checked_sub(start)
            .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
        if args.len() <= 1 {
            let delete_count = if args.is_empty() { 0 } else { max_delete };
            return Ok((delete_count, Vec::new()));
        }
        let requested =
            Self::array_to_integer_or_infinity(args.get(1).unwrap_or(&Value::Undefined));
        let delete_count = Self::array_clamp_index(requested, max_delete)?;
        let items = args.get(2..).unwrap_or(&[]).to_vec();
        Ok((delete_count, items))
    }

    pub(super) fn array_spliced_length(
        length: usize,
        delete_count: usize,
        item_count: usize,
    ) -> Result<usize> {
        length
            .checked_sub(delete_count)
            .and_then(|value| value.checked_add(item_count))
            .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))
    }

    pub(in crate::runtime::native) fn eval_array_fill(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_fill(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_fill(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let value = args.first().cloned().unwrap_or(Value::Undefined);
        let start = Self::array_slice_bound(args.get(1), length, 0)?;
        let end = Self::array_slice_bound(args.get(2), length, length)?;
        for index in start..end {
            self.step()?;
            self.set_array_like_index(this_value, index, value.clone())?;
        }
        Ok(this_value.clone())
    }

    pub(in crate::runtime::native) fn eval_array_copy_within(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_copy_within(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_copy_within(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let to = Self::array_slice_bound(args.first(), length, 0)?;
        let from = Self::array_slice_bound(args.get(1), length, 0)?;
        let end = Self::array_slice_bound(args.get(2), length, length)?;
        let count = end.saturating_sub(from).min(length.saturating_sub(to));
        if count == 0 {
            return Ok(this_value.clone());
        }
        let backward = from < to && to < from.saturating_add(count);
        if backward {
            for offset in (0..count).rev() {
                self.step()?;
                self.array_copy_within_step(this_value, from, to, offset)?;
            }
        } else {
            for offset in 0..count {
                self.step()?;
                self.array_copy_within_step(this_value, from, to, offset)?;
            }
        }
        Ok(this_value.clone())
    }

    fn array_copy_within_step(
        &mut self,
        this_value: &Value,
        from: usize,
        to: usize,
        offset: usize,
    ) -> Result<()> {
        let from_index = from
            .checked_add(offset)
            .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
        let to_index = to
            .checked_add(offset)
            .ok_or_else(|| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
        self.array_move_or_delete(this_value, from_index, to_index)
    }

    pub(in crate::runtime::native) fn eval_array_at(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_at(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_at(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let relative =
            Self::array_to_integer_or_infinity(args.first().unwrap_or(&Value::Undefined));
        let length_f64 = u32::try_from(length)
            .map(f64::from)
            .map_err(|_| Error::limit(ARRAY_MUTATE_INDEX_ERROR))?;
        let target = if relative >= 0.0 {
            relative
        } else {
            length_f64 + relative
        };
        if target < 0.0 || target >= length_f64 {
            return Ok(Value::Undefined);
        }
        let index = Self::array_clamp_index(target, length)?;
        self.get_array_like_index(this_value, index)
    }
}
