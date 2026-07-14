use crate::{
    binding_metadata::BindingOperand,
    bytecode::{BytecodeBinding, BytecodeInstruction, BytecodeNumericBinaryOp},
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs, control::Completion, roots::VmRootKind},
    value::{FunctionId, Value},
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum NumericSortOrder {
    Ascending,
    Descending,
}

impl NumericSortOrder {
    const fn is_descending(self) -> bool {
        matches!(self, Self::Descending)
    }
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
        let comparator = self.array_sort_comparator(args)?;
        Self::ensure_array_like_object(this_value)?;
        if let Some(value) = self.eval_packed_numeric_array_sort(this_value, comparator.as_ref())? {
            return Ok(value);
        }
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
        let comparator = self.array_sort_comparator(args)?;
        Self::ensure_array_like_object(this_value)?;
        if let Some(value) =
            self.eval_packed_numeric_array_to_sorted(this_value, comparator.as_ref())?
        {
            return Ok(value);
        }
        let length = self.array_like_length(this_value)?;
        let result = self.create_intrinsic_array_with_length(length)?;
        let _result_scope =
            self.transient_root_scope(VmRootKind::TransientTemporary, std::iter::once(&result))?;
        let mut items = Vec::new();
        for index in 0..length {
            self.step()?;
            items.push(self.get_array_like_index(this_value, index)?);
        }
        let sorted = self.array_sort_items(items, comparator.as_ref())?;
        for (index, value) in sorted.into_iter().enumerate() {
            self.array_from_create_data_property(&result, index, value)?;
        }
        Ok(result)
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

    fn array_sort_comparator(&self, args: &[Value]) -> Result<Option<Value>> {
        match args.first() {
            None | Some(Value::Undefined) => Ok(None),
            Some(value) if self.semantic_is_callable(value)? => Ok(Some(value.clone())),
            Some(_) => Err(Error::type_error(ARRAY_SORT_COMPARATOR_ERROR)),
        }
    }

    fn eval_packed_numeric_array_sort(
        &mut self,
        this_value: &Value,
        comparator: Option<&Value>,
    ) -> Result<Option<Value>> {
        let Some(order) = self.array_numeric_sort_order(comparator)? else {
            return Ok(None);
        };
        let Value::Object(id) = this_value else {
            return Ok(None);
        };
        if !self
            .objects
            .sort_packed_default_numeric_array_if_array(*id, order.is_descending())?
        {
            return Ok(None);
        }
        if let Some(length) = self.objects.array_len_if_array(*id)? {
            self.charge_runtime_steps(length)?;
        }
        Ok(Some(this_value.clone()))
    }

    fn eval_packed_numeric_array_to_sorted(
        &mut self,
        this_value: &Value,
        comparator: Option<&Value>,
    ) -> Result<Option<Value>> {
        let Some(order) = self.array_numeric_sort_order(comparator)? else {
            return Ok(None);
        };
        let Some(mut values) = self.packed_numeric_sort_values(this_value)? else {
            return Ok(None);
        };
        Self::sort_numeric_values(&mut values, order);
        self.charge_runtime_steps(values.len())?;
        let values = values.into_iter().map(Value::Number).collect();
        self.create_array_from_elements(values).map(Some)
    }

    fn packed_numeric_sort_values(&self, object: &Value) -> Result<Option<Vec<f64>>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(values) = self.objects.packed_default_array_values_if_array(*id)? else {
            return Ok(None);
        };
        let mut numbers = Vec::with_capacity(values.len());
        for value in values {
            let Value::Number(number) = value else {
                return Ok(None);
            };
            if number.is_nan() {
                return Ok(None);
            }
            numbers.push(number);
        }
        Ok(Some(numbers))
    }

    fn sort_numeric_values(values: &mut [f64], order: NumericSortOrder) {
        values.sort_by(|left, right| numeric_sort_ordering(*left, *right, order));
    }

    fn array_numeric_sort_order(
        &self,
        comparator: Option<&Value>,
    ) -> Result<Option<NumericSortOrder>> {
        if !self.optional_optimizations_enabled() {
            return Ok(None);
        }
        let Some(Value::Function(callback)) = comparator else {
            return Ok(None);
        };
        self.compile_numeric_sort_comparator(*callback)
    }

    fn compile_numeric_sort_comparator(
        &self,
        callback: FunctionId,
    ) -> Result<Option<NumericSortOrder>> {
        if self.is_class_constructor(callback)? {
            return Ok(None);
        }
        let function = self.function(callback)?;
        let bytecode = &function.bytecode;
        if bytecode.hoist_plan().lexical_declaration_count() != 0
            || bytecode.hoist_plan().var_declaration_count() != 0
            || bytecode.hoist_plan().function_declaration_count() != 0
        {
            return Ok(None);
        }
        let [
            BytecodeInstruction::LoadBinding(left),
            BytecodeInstruction::LoadBinding(right),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Sub),
            BytecodeInstruction::Complete(crate::bytecode::BytecodeCompletion::Return),
        ] = bytecode.body().instructions()
        else {
            return Ok(None);
        };
        let Some(left) = Self::sort_comparator_param_index(&function.param_frames, left)? else {
            return Ok(None);
        };
        let Some(right) = Self::sort_comparator_param_index(&function.param_frames, right)? else {
            return Ok(None);
        };
        Ok(match (left, right) {
            (0, 1) => Some(NumericSortOrder::Ascending),
            (1, 0) => Some(NumericSortOrder::Descending),
            _ => None,
        })
    }

    fn sort_comparator_param_index(
        param_frames: &[Option<crate::runtime::CompiledBindingFrame>],
        binding: &BytecodeBinding,
    ) -> Result<Option<usize>> {
        let BindingOperand::Local { scope, slot } = binding.operand() else {
            return Ok(None);
        };
        let slot = slot.index()?;
        Ok(param_frames.iter().enumerate().find_map(|(index, frame)| {
            let frame = frame.as_ref()?;
            (frame.scope() == Some(scope) && frame.slot().index() == slot).then_some(index)
        }))
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
            let number = self.to_number(&result)?;
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
        match self.call(comparator, &call_args, Value::Undefined)? {
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

fn numeric_sort_ordering(left: f64, right: f64, order: NumericSortOrder) -> std::cmp::Ordering {
    let result = match order {
        NumericSortOrder::Ascending => left - right,
        NumericSortOrder::Descending => right - left,
    };
    if result.is_nan() || result == 0.0 {
        return std::cmp::Ordering::Equal;
    }
    if result < 0.0 {
        std::cmp::Ordering::Less
    } else {
        std::cmp::Ordering::Greater
    }
}
