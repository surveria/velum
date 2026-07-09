use crate::{
    bytecode::{BytecodeBinding, BytecodeNumericCompareOp},
    error::Result,
    runtime::{Context, binding::scope::BindingCell},
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
    pub(super) fn assign_fast_path_cell(
        &self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        value: Value,
    ) -> Result<()> {
        let value = self.checked_value(value)?;
        cell.assign(binding.name(), value)
    }
}
