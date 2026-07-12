use super::{BytecodeBlock, BytecodeDynamicProperty, BytecodeProperty};

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeSuperProperty {
    Static(BytecodeProperty),
    Computed {
        expression: BytecodeBlock,
        operand: BytecodeDynamicProperty,
    },
}
