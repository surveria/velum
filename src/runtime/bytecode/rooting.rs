use crate::{
    bytecode::BytecodeInstruction,
    error::Result,
    runtime::{Context, VmStorageKind, roots::VmRootKind, transient_roots::TransientRootScope},
    syntax::UnaryOp,
};

use super::state::BytecodeState;

impl Context {
    #[inline]
    pub(super) fn bytecode_instruction_root_scope(
        &self,
        state: &BytecodeState,
        instruction: &BytecodeInstruction,
    ) -> Result<TransientRootScope> {
        // An unregistered operand can survive only until the next collection
        // or re-entry point. Keep the original registrations for finite root
        // budgets too, so their failure point and accounting remain unchanged.
        if instruction_cannot_reenter(instruction)
            && self.limits.storage.max_count(VmStorageKind::TransientRoot) == usize::MAX
            && !self.bytecode_gc_is_pending()
        {
            return Ok(TransientRootScope::inactive());
        }
        self.synchronous_bytecode_root_scope(state)
    }

    #[inline]
    pub(super) fn synchronous_bytecode_root_scope(
        &self,
        state: &BytecodeState,
    ) -> Result<TransientRootScope> {
        if let Some((values, last)) = state.simple_synchronous_root_values() {
            return self.transient_bytecode_root_scope(values, last);
        }
        self.transient_root_scope(
            VmRootKind::TransientOperand,
            state.synchronous_root_values(),
        )
    }
}

// This is deliberately an allowlist: a new or unreviewed instruction keeps
// full roots. Allocation alone does not collect; automatic collection happens
// at the guarded bytecode safe point. Property/binding access, coercion, calls,
// structured execution and every linear segment retain their existing roots.
const fn instruction_cannot_reenter(instruction: &BytecodeInstruction) -> bool {
    matches!(
        instruction,
        BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::PushUndefined
            | BytecodeInstruction::LoadNewTarget
            | BytecodeInstruction::StoreLast
            | BytecodeInstruction::Pop
            | BytecodeInstruction::Duplicate
            | BytecodeInstruction::Unary(UnaryOp::Not | UnaryOp::Void)
            | BytecodeInstruction::TypeOfValue
            | BytecodeInstruction::DeleteValue
            | BytecodeInstruction::Jump(_)
            | BytecodeInstruction::JumpIfFalse(_)
            | BytecodeInstruction::JumpIfFalseKeep(_)
            | BytecodeInstruction::JumpIfTrueKeep(_)
            | BytecodeInstruction::JumpIfNullishKeep(_)
            | BytecodeInstruction::Complete(_)
    )
}
