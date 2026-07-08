use crate::{
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, control::Completion},
    value::Value,
};

const ARRAY_SORT_COMPARATOR_ERROR: &str =
    "Array.prototype.sort comparator must be undefined or callable";

/// Result of a spec `CompareArrayElements` step encoded as a total ordering.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SortOrder {
    Less,
    Equal,
    Greater,
}

impl Context {
    pub(in crate::runtime::native) fn eval_array_sort(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_sort(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_sort(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let comparator = Self::array_sort_comparator(args)?;
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let items = self.array_collect_present(this_value, length)?;
        let present = items.len();
        let sorted = self.array_sort_items(items, comparator.as_ref())?;
        for (index, value) in sorted.into_iter().enumerate() {
            self.step()?;
            self.set_array_like_index(this_value, index, value)?;
        }
        for index in present..length {
            self.step()?;
            self.delete_array_like_index(this_value, index)?;
        }
        Ok(this_value.clone())
    }

    pub(in crate::runtime::native) fn eval_array_to_sorted(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_to_sorted(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_to_sorted(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        let comparator = Self::array_sort_comparator(args)?;
        Self::ensure_array_like_object(this_value)?;
        let length = self.array_like_length(this_value)?;
        let mut items = Vec::new();
        for index in 0..length {
            self.step()?;
            items.push(self.get_array_like_index(this_value, index)?);
        }
        let sorted = self.array_sort_items(items, comparator.as_ref())?;
        self.create_array_from_elements(sorted)
    }

    /// Collect the values at every present index in `[0, length)`.
    fn array_collect_present(&mut self, this_value: &Value, length: usize) -> Result<Vec<Value>> {
        let mut items = Vec::new();
        for index in 0..length {
            self.step()?;
            if self.has_array_like_index(this_value, index)? {
                items.push(self.get_array_like_index(this_value, index)?);
            }
        }
        Ok(items)
    }

    fn array_sort_comparator(args: &[Value]) -> Result<Option<Value>> {
        match args.first() {
            None | Some(Value::Undefined) => Ok(None),
            Some(value) if Self::is_callable(value) => Ok(Some(value.clone())),
            Some(_) => Err(Error::type_error(ARRAY_SORT_COMPARATOR_ERROR)),
        }
    }

    /// Stable merge sort that propagates abrupt comparator completions.
    fn array_sort_items(
        &mut self,
        mut items: Vec<Value>,
        comparator: Option<&Value>,
    ) -> Result<Vec<Value>> {
        let len = items.len();
        if len <= 1 {
            return Ok(items);
        }
        let right = items.split_off(len / 2);
        let left = self.array_sort_items(items, comparator)?;
        let right = self.array_sort_items(right, comparator)?;
        self.array_sort_merge(left, right, comparator)
    }

    fn array_sort_merge(
        &mut self,
        left: Vec<Value>,
        right: Vec<Value>,
        comparator: Option<&Value>,
    ) -> Result<Vec<Value>> {
        let total = left
            .len()
            .checked_add(right.len())
            .ok_or_else(|| Error::limit("array sort length exceeded supported range"))?;
        let mut merged = Vec::with_capacity(total);
        let mut i = 0usize;
        let mut j = 0usize;
        while i < left.len() && j < right.len() {
            self.step()?;
            let (Some(x), Some(y)) = (left.get(i), right.get(j)) else {
                break;
            };
            // Stable: keep the left element when the ordering is not Greater.
            if self.array_compare_elements(x, y, comparator)? == SortOrder::Greater {
                if let Some(value) = right.get(j).cloned() {
                    merged.push(value);
                }
                j += 1;
            } else {
                if let Some(value) = left.get(i).cloned() {
                    merged.push(value);
                }
                i += 1;
            }
        }
        merged.extend(left.into_iter().skip(i));
        merged.extend(right.into_iter().skip(j));
        Ok(merged)
    }

    fn array_compare_elements(
        &mut self,
        x: &Value,
        y: &Value,
        comparator: Option<&Value>,
    ) -> Result<SortOrder> {
        let x_undef = matches!(x, Value::Undefined);
        let y_undef = matches!(y, Value::Undefined);
        if x_undef || y_undef {
            return Ok(match (x_undef, y_undef) {
                (true, true) => SortOrder::Equal,
                (true, false) => SortOrder::Greater,
                (false, _) => SortOrder::Less,
            });
        }
        if let Some(comparator) = comparator {
            let result = self.array_call_comparator(comparator, x, y)?;
            let number = Self::value_to_number(&result);
            return Ok(if number.is_nan() || number == 0.0 {
                SortOrder::Equal
            } else if number < 0.0 {
                SortOrder::Less
            } else {
                SortOrder::Greater
            });
        }
        let x_string = self.string_argument_text(x)?;
        let y_string = self.string_argument_text(y)?;
        Ok(Self::compare_code_units(&x_string, &y_string))
    }

    fn array_call_comparator(&mut self, comparator: &Value, x: &Value, y: &Value) -> Result<Value> {
        let call_args = [x.clone(), y.clone()];
        match self.eval_call_completion(comparator.clone(), &call_args, Value::Undefined)? {
            Completion::Normal(value) => Ok(value),
            completion => completion.into_result(),
        }
    }

    /// Lexicographic comparison over UTF-16 code units, matching the default
    /// `Array.prototype.sort` comparator.
    fn compare_code_units(left: &str, right: &str) -> SortOrder {
        let mut left_units = left.encode_utf16();
        let mut right_units = right.encode_utf16();
        loop {
            match (left_units.next(), right_units.next()) {
                (Some(left_unit), Some(right_unit)) => {
                    if left_unit < right_unit {
                        return SortOrder::Less;
                    }
                    if left_unit > right_unit {
                        return SortOrder::Greater;
                    }
                }
                (Some(_), None) => return SortOrder::Greater,
                (None, Some(_)) => return SortOrder::Less,
                (None, None) => return SortOrder::Equal,
            }
        }
    }
}
