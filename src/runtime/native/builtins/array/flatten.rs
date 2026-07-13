use crate::{
    binding_metadata::BindingOperand,
    bytecode::{BytecodeBinding, BytecodeInstruction, BytecodeNumericBinaryOp},
    error::{Error, Result},
    runtime::{Context, call::RuntimeCallArgs},
    value::{FunctionId, Value},
};

const ARRAY_FLATTEN_INDEX_LIMIT_ERROR: &str = "array flatten index exceeded supported range";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum FlattenDepth {
    Finite(usize),
    Infinity,
}

impl Context {
    pub(in crate::runtime::native) fn eval_array_flat(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_flat(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_flat(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let source_length = self.array_like_length(this_value)?;
        let depth = self.flat_depth_arg(args)?;
        let result = self.array_species_create(this_value, 0)?;
        let mut target_index = 0;
        if !self.flatten_packed_array_into(this_value, &result, &mut target_index, depth)? {
            self.flatten_array_like_into(
                this_value,
                source_length,
                &result,
                &mut target_index,
                depth,
            )?;
        }
        Ok(result)
    }

    pub(in crate::runtime::native) fn eval_array_flat_map(
        &mut self,
        args: RuntimeCallArgs<'_>,
        this_value: &Value,
    ) -> Result<Value> {
        self.eval_direct_array_flat_map(args.as_slice(), this_value)
    }

    pub(in crate::runtime::native) fn eval_direct_array_flat_map(
        &mut self,
        args: &[Value],
        this_value: &Value,
    ) -> Result<Value> {
        Self::ensure_array_like_object(this_value)?;
        let source_length = self.array_like_length(this_value)?;
        let (callback, callback_this) = self.array_callback_and_this_arg(args)?;
        let result = self.array_species_create(this_value, 0)?;
        if let Some(values) =
            self.eval_packed_numeric_array_flat_map(callback, &callback_this, this_value)?
        {
            let mut target_index = 0;
            for value in values {
                self.append_flattened_value(&result, &mut target_index, value)?;
            }
            return Ok(result);
        }
        let mut target_index = 0;
        for source_index in 0..source_length {
            self.step()?;
            if !self.has_array_like_index(this_value, source_index)? {
                continue;
            }
            let value = self.get_array_like_index(this_value, source_index)?;
            let mapped = self.call_array_callback(
                callback,
                callback_this.clone(),
                &value,
                source_index,
                this_value,
            )?;
            self.flatten_value_into(mapped, &result, &mut target_index, FlattenDepth::Finite(1))?;
        }
        Ok(result)
    }

    fn eval_packed_numeric_array_flat_map(
        &mut self,
        callback: &Value,
        callback_this: &Value,
        this_value: &Value,
    ) -> Result<Option<Vec<Value>>> {
        if !self.optional_optimizations_enabled() || !matches!(callback_this, Value::Undefined) {
            return Ok(None);
        }
        let Value::Function(callback_id) = callback else {
            return Ok(None);
        };
        let Some(pattern) = self.compile_flat_map_array_callback(*callback_id)? else {
            return Ok(None);
        };
        let Some(source_values) = self.packed_numeric_flat_map_source_values(this_value)? else {
            return Ok(None);
        };
        let probe_args = [Value::Number(0.0), Value::Number(0.0), this_value.clone()];
        if pattern
            .elements
            .iter()
            .any(|source| Self::eval_flat_map_array_source(source, &probe_args).is_none())
        {
            return Ok(None);
        }
        let capacity = source_values
            .len()
            .checked_mul(pattern.elements.len())
            .ok_or_else(|| Error::limit(ARRAY_FLATTEN_INDEX_LIMIT_ERROR))?;
        let mut values = Vec::with_capacity(capacity);
        for (index, value) in source_values.iter().enumerate() {
            self.step()?;
            self.charge_runtime_steps(pattern.step_count)?;
            let index = Self::array_like_index_value(index)?;
            let callback_args = [value.clone(), index, this_value.clone()];
            for source in &pattern.elements {
                let Some(value) = Self::eval_flat_map_array_source(source, &callback_args) else {
                    return Ok(None);
                };
                values.push(value);
            }
        }
        Ok(Some(values))
    }

    fn flatten_array_like_into(
        &mut self,
        source: &Value,
        source_length: usize,
        result: &Value,
        target_index: &mut usize,
        depth: FlattenDepth,
    ) -> Result<()> {
        for source_index in 0..source_length {
            self.step()?;
            if !self.has_array_like_index(source, source_index)? {
                continue;
            }
            let value = self.get_array_like_index(source, source_index)?;
            self.flatten_value_into(value, result, target_index, depth)?;
        }
        Ok(())
    }

    fn flatten_value_into(
        &mut self,
        value: Value,
        result: &Value,
        target_index: &mut usize,
        depth: FlattenDepth,
    ) -> Result<()> {
        if let Some(next_depth) = depth.descend()
            && self.is_flattenable_array(&value)?
        {
            if self.flatten_packed_array_into(&value, result, target_index, next_depth)? {
                return Ok(());
            }
            let nested_length = self.array_like_length(&value)?;
            return self.flatten_array_like_into(
                &value,
                nested_length,
                result,
                target_index,
                next_depth,
            );
        }
        self.append_flattened_value(result, target_index, value)
    }

    fn flatten_packed_array_into(
        &mut self,
        source: &Value,
        result: &Value,
        target_index: &mut usize,
        depth: FlattenDepth,
    ) -> Result<bool> {
        let mut values = Vec::new();
        let Some(steps) = self.collect_packed_flat_values(source, depth, &mut values)? else {
            return Ok(false);
        };
        self.charge_runtime_steps(steps)?;
        for value in values {
            self.append_flattened_value(result, target_index, value)?;
        }
        Ok(true)
    }

    fn collect_packed_flat_values(
        &self,
        source: &Value,
        depth: FlattenDepth,
        values: &mut Vec<Value>,
    ) -> Result<Option<usize>> {
        let Value::Object(id) = source else {
            return Ok(None);
        };
        let Some(source_values) = self.objects.packed_array_values_if_array(*id)? else {
            return Ok(None);
        };
        let mut steps = source_values.len();
        for value in source_values {
            if let Some(next_depth) = depth.descend()
                && self.packed_value_is_flattenable_array(&value)?
            {
                let Some(nested_steps) =
                    self.collect_packed_flat_values(&value, next_depth, values)?
                else {
                    return Ok(None);
                };
                steps = steps
                    .checked_add(nested_steps)
                    .ok_or_else(|| Error::limit(ARRAY_FLATTEN_INDEX_LIMIT_ERROR))?;
            } else {
                values.push(value);
            }
        }
        Ok(Some(steps))
    }

    fn append_flattened_value(
        &mut self,
        result: &Value,
        target_index: &mut usize,
        value: Value,
    ) -> Result<()> {
        self.array_from_create_data_property(result, *target_index, value)?;
        *target_index = target_index
            .checked_add(1)
            .ok_or_else(|| Error::limit(ARRAY_FLATTEN_INDEX_LIMIT_ERROR))?;
        Ok(())
    }

    fn is_flattenable_array(&self, value: &Value) -> Result<bool> {
        self.semantic_is_array(value)
    }

    fn packed_value_is_flattenable_array(&self, value: &Value) -> Result<bool> {
        let Value::Object(id) = value else {
            return Ok(false);
        };
        Ok(self.objects.packed_array_values_if_array(*id)?.is_some())
    }

    fn compile_flat_map_array_callback(
        &self,
        callback: FunctionId,
    ) -> Result<Option<FlatMapArrayCallback>> {
        let function = self.function(callback)?;
        let bytecode = &function.bytecode;
        if bytecode.hoist_plan().lexical_declaration_count() != 0
            || bytecode.hoist_plan().var_declaration_count() != 0
            || bytecode.hoist_plan().function_declaration_count() != 0
        {
            return Ok(None);
        }
        let instructions = bytecode.body().instructions();
        let Some((last, prefix)) = instructions.split_last() else {
            return Ok(None);
        };
        if !matches!(
            last,
            BytecodeInstruction::Complete(crate::bytecode::BytecodeCompletion::Return)
        ) {
            return Ok(None);
        }
        let Some((array, body)) = prefix.split_last() else {
            return Ok(None);
        };
        let BytecodeInstruction::ArrayLiteral { len, .. } = array else {
            return Ok(None);
        };
        let mut stack = Vec::new();
        for instruction in body {
            match instruction {
                BytecodeInstruction::LoadBinding(binding) => {
                    let Some(source) =
                        Self::flat_map_array_source_for_binding(&function.param_frames, binding)?
                    else {
                        return Ok(None);
                    };
                    stack.push(source);
                }
                BytecodeInstruction::PushLiteral(Value::Number(value)) => {
                    stack.push(FlatMapArraySource::Literal(*value));
                }
                BytecodeInstruction::NumberBinary(op) => {
                    let Some(right) = stack.pop() else {
                        return Ok(None);
                    };
                    let Some(left) = stack.pop() else {
                        return Ok(None);
                    };
                    if !flat_map_array_op_is_supported(*op) {
                        return Ok(None);
                    }
                    stack.push(FlatMapArraySource::NumberBinary {
                        op: *op,
                        left: Box::new(left),
                        right: Box::new(right),
                    });
                }
                _ => return Ok(None),
            }
        }
        if stack.len() != *len {
            return Ok(None);
        }
        Ok(Some(FlatMapArrayCallback {
            elements: stack,
            step_count: instructions.len(),
        }))
    }

    fn flat_map_array_source_for_binding(
        param_frames: &[Option<crate::runtime::CompiledBindingFrame>],
        binding: &BytecodeBinding,
    ) -> Result<Option<FlatMapArraySource>> {
        let BindingOperand::Local { scope, slot } = binding.operand() else {
            return Ok(None);
        };
        let slot = slot.index()?;
        Ok(param_frames.iter().enumerate().find_map(|(index, frame)| {
            let frame = frame.as_ref()?;
            (frame.scope() == Some(scope) && frame.slot().index() == slot)
                .then_some(FlatMapArraySource::Param(index))
        }))
    }

    fn eval_flat_map_array_source(source: &FlatMapArraySource, args: &[Value]) -> Option<Value> {
        match source {
            FlatMapArraySource::Param(index) => {
                Some(args.get(*index).cloned().unwrap_or(Value::Undefined))
            }
            FlatMapArraySource::Literal(value) => Some(Value::Number(*value)),
            FlatMapArraySource::NumberBinary { op, left, right } => {
                let left = Self::eval_flat_map_array_source(left, args)?;
                let right = Self::eval_flat_map_array_source(right, args)?;
                Self::eval_flat_map_array_number_binary(*op, &left, &right)
            }
        }
    }

    fn eval_flat_map_array_number_binary(
        op: BytecodeNumericBinaryOp,
        left: &Value,
        right: &Value,
    ) -> Option<Value> {
        let (Value::Number(left), Value::Number(right)) = (left, right) else {
            return None;
        };
        let value = match op {
            BytecodeNumericBinaryOp::Add => left + right,
            BytecodeNumericBinaryOp::Sub => left - right,
            BytecodeNumericBinaryOp::Mul => left * right,
            BytecodeNumericBinaryOp::Div => left / right,
            BytecodeNumericBinaryOp::Rem => left % right,
            BytecodeNumericBinaryOp::Pow => {
                crate::runtime::numeric::number_exponentiate(*left, *right)
            }
            BytecodeNumericBinaryOp::BitAnd
            | BytecodeNumericBinaryOp::BitOr
            | BytecodeNumericBinaryOp::BitXor
            | BytecodeNumericBinaryOp::ShiftLeft
            | BytecodeNumericBinaryOp::ShiftRight
            | BytecodeNumericBinaryOp::ShiftRightUnsigned => return None,
        };
        Some(Value::Number(value))
    }

    fn packed_numeric_flat_map_source_values(&self, object: &Value) -> Result<Option<Vec<Value>>> {
        let Value::Object(id) = object else {
            return Ok(None);
        };
        let Some(values) = self.objects.packed_array_values_if_array(*id)? else {
            return Ok(None);
        };
        if values.iter().all(|value| matches!(value, Value::Number(_))) {
            return Ok(Some(values));
        }
        Ok(None)
    }

    fn flat_depth_arg(&mut self, args: &[Value]) -> Result<FlattenDepth> {
        let Some(value) = args.first() else {
            return Ok(FlattenDepth::Finite(1));
        };
        if matches!(value, Value::Undefined) {
            return Ok(FlattenDepth::Finite(1));
        }
        self.value_to_flatten_depth(value)
    }

    fn value_to_flatten_depth(&mut self, value: &Value) -> Result<FlattenDepth> {
        let integer = self.to_integer_or_infinity(value)?;
        if integer <= 0.0 {
            return Ok(FlattenDepth::Finite(0));
        }
        if !integer.is_finite() {
            return Ok(FlattenDepth::Infinity);
        }
        Self::finite_flatten_depth(integer)
    }

    fn finite_flatten_depth(value: f64) -> Result<FlattenDepth> {
        Self::finite_nonnegative_integer_to_usize(
            value,
            "array flatten depth exceeded supported range",
        )
        .map(FlattenDepth::Finite)
    }
}

impl FlattenDepth {
    const fn descend(self) -> Option<Self> {
        match self {
            Self::Finite(0) => None,
            Self::Finite(value) => Some(Self::Finite(value.saturating_sub(1))),
            Self::Infinity => Some(Self::Infinity),
        }
    }
}

#[derive(Debug, Clone)]
struct FlatMapArrayCallback {
    elements: Vec<FlatMapArraySource>,
    step_count: usize,
}

#[derive(Debug, Clone)]
enum FlatMapArraySource {
    Param(usize),
    Literal(f64),
    NumberBinary {
        op: BytecodeNumericBinaryOp,
        left: Box<Self>,
        right: Box<Self>,
    },
}

const fn flat_map_array_op_is_supported(op: BytecodeNumericBinaryOp) -> bool {
    matches!(
        op,
        BytecodeNumericBinaryOp::Add
            | BytecodeNumericBinaryOp::Sub
            | BytecodeNumericBinaryOp::Mul
            | BytecodeNumericBinaryOp::Div
            | BytecodeNumericBinaryOp::Rem
            | BytecodeNumericBinaryOp::Pow
    )
}
