use crate::{syntax::StaticString, value::Value};

use super::{
    BytecodeBlock,
    numeric::{BytecodeNumericBinaryOp, BytecodeNumericEqualityOp},
    types::{BytecodeBinding, BytecodeCompletion, BytecodeInstruction},
};

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeDirectThrow {
    Literal(Value),
    String(StaticString),
    Undefined,
}

impl BytecodeDirectThrow {
    pub(crate) fn from_unscoped_block_start(block: &BytecodeBlock) -> Option<Self> {
        let first = block.instructions().first()?;
        let second = block.instructions().get(1)?;
        if !matches!(
            second,
            BytecodeInstruction::Complete(BytecodeCompletion::Throw)
        ) {
            return None;
        }
        match first {
            BytecodeInstruction::PushLiteral(value) => Some(Self::Literal(value.clone())),
            BytecodeInstruction::PushString(value) => Some(Self::String(value.clone())),
            BytecodeInstruction::PushUndefined => Some(Self::Undefined),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeCatchFastPath {
    StrictStringIncrement {
        test: BytecodeBinding,
        expected: StaticString,
        target: BytecodeBinding,
        addend: f64,
    },
}

impl BytecodeCatchFastPath {
    pub(crate) fn from_unscoped_body(
        param: Option<&BytecodeBinding>,
        block: &BytecodeBlock,
        scoped: bool,
    ) -> Option<Self> {
        if scoped {
            return None;
        }
        let param = param?;
        let instructions = block.instructions();
        let Some(BytecodeInstruction::LoadBinding(test)) = instructions.first() else {
            return None;
        };
        if !same_bytecode_binding(test, param) {
            return None;
        }
        let Some(BytecodeInstruction::PushString(expected)) = instructions.get(1) else {
            return None;
        };
        if !matches!(
            instructions.get(2),
            Some(BytecodeInstruction::NumberEquality(
                BytecodeNumericEqualityOp::StrictEqual
            ))
        ) {
            return None;
        }
        if !matches!(
            instructions.get(3),
            Some(BytecodeInstruction::JumpIfFalse(address)) if address.index() == 10
        ) {
            return None;
        }
        let Some(BytecodeInstruction::LoadBinding(target_read)) = instructions.get(4) else {
            return None;
        };
        let Some(BytecodeInstruction::PushLiteral(Value::Number(addend))) = instructions.get(5)
        else {
            return None;
        };
        if !matches!(
            instructions.get(6),
            Some(BytecodeInstruction::NumberBinary(
                BytecodeNumericBinaryOp::Add
            ))
        ) {
            return None;
        }
        let Some(BytecodeInstruction::StoreBinding(target_write)) = instructions.get(7) else {
            return None;
        };
        if !same_bytecode_binding(target_read, target_write) {
            return None;
        }
        if !matches!(instructions.get(8), Some(BytecodeInstruction::StoreLast)) {
            return None;
        }
        if !matches!(
            instructions.get(9),
            Some(BytecodeInstruction::Jump(address)) if address.index() == 12
        ) {
            return None;
        }
        if !matches!(
            instructions.get(10),
            Some(BytecodeInstruction::PushUndefined)
        ) {
            return None;
        }
        if !matches!(instructions.get(11), Some(BytecodeInstruction::StoreLast)) {
            return None;
        }
        if instructions.get(12).is_some() {
            return None;
        }
        Some(Self::StrictStringIncrement {
            test: test.clone(),
            expected: expected.clone(),
            target: target_read.clone(),
            addend: *addend,
        })
    }
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}
