use crate::{syntax::StaticString, value::Value};

use super::{BytecodeBlock, BytecodeCompletion, types::BytecodeInstruction};

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
