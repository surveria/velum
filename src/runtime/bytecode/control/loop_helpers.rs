use crate::{
    bytecode::{BytecodeBinding, BytecodeNumericBinaryOp, BytecodeNumericCompareOp},
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    value::Value,
};

pub(super) fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}

pub(super) fn fast_loop_compare(op: BytecodeNumericCompareOp, left: f64, right: f64) -> bool {
    match op {
        BytecodeNumericCompareOp::Less => left < right,
        BytecodeNumericCompareOp::LessEqual => left <= right,
        BytecodeNumericCompareOp::Greater => left > right,
        BytecodeNumericCompareOp::GreaterEqual => left >= right,
    }
}

impl Context {
    pub(super) fn fast_array_index_value(
        &mut self,
        object: &Value,
        index: Option<usize>,
    ) -> Result<Option<Value>> {
        let (Value::Object(id), Some(index)) = (object, index) else {
            return Ok(None);
        };
        self.objects
            .array_index_value_if_array(*id, index)?
            .map(|value| self.runtime_value(value))
            .transpose()
    }

    pub(super) fn assign_fast_path_cell(
        &self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        value: Value,
    ) -> Result<()> {
        let value = self.checked_value(value)?;
        cell.assign(binding.name(), value)
    }

    pub(super) fn masked_binding_value(
        &mut self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        mask: f64,
        mask_i32: i32,
    ) -> Result<Value> {
        let value = self.runtime_value(cell.value(binding.name())?)?;
        if let Value::Number(number) = value {
            let masked = number_to_i32(number, "&")? & mask_i32;
            return Ok(Value::Number(f64::from(masked)));
        }
        self.eval_bytecode_number_binary(
            BytecodeNumericBinaryOp::BitAnd,
            &value,
            &Value::Number(mask),
        )
    }

    pub(super) fn masked_binding_index(
        &mut self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        mask: f64,
        mask_i32: i32,
    ) -> Result<(Value, Option<usize>)> {
        let value = self.runtime_value(cell.value(binding.name())?)?;
        if let Value::Number(number) = value {
            let index = number_to_i32(number, "&")? & mask_i32;
            return Ok((Value::Number(f64::from(index)), usize::try_from(index).ok()));
        }
        let property = self.eval_bytecode_number_binary(
            BytecodeNumericBinaryOp::BitAnd,
            &value,
            &Value::Number(mask),
        )?;
        Ok((property, None))
    }
}
