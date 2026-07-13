use crate::bytecode::{BytecodeBinding, BytecodeInstruction};

pub(super) fn instruction_window(
    instructions: &[BytecodeInstruction],
    start: usize,
    len: usize,
) -> Option<&[BytecodeInstruction]> {
    let end = start.checked_add(len)?;
    instructions.get(start..end)
}

pub(super) fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}
