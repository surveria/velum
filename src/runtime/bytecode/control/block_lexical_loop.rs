use crate::{
    bytecode::{
        BytecodeBinding, BytecodeBlock, BytecodeInstruction, BytecodeNumericBinaryOp,
        BytecodeObjectProperty,
    },
    error::Result,
    runtime::{Context, binding::scope::BindingCell, numeric::number_to_i32},
    syntax::DeclKind,
    value::Value,
};

#[derive(Debug)]
pub(super) struct BytecodeBlockLexicalLoopFastPath<'a> {
    total: &'a BytecodeBinding,
    total_cell: BindingCell,
    outer_mask_i32: i32,
    inner_add: f64,
    bump_mask_i32: i32,
}

impl Context {
    pub(super) fn compile_block_lexical_loop_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeBlockLexicalLoopFastPath<'a>>> {
        let [BytecodeInstruction::ScopedBlock(outer_block)] = body.instructions() else {
            return Ok(None);
        };
        let [
            BytecodeInstruction::LoadBinding(outer_mask_source),
            BytecodeInstruction::PushLiteral(Value::Number(outer_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::ObjectLiteral { properties },
            BytecodeInstruction::DeclareBinding {
                name: record,
                kind: DeclKind::Let,
                has_init: true,
            },
            BytecodeInstruction::ScopedBlock(inner_block),
        ] = outer_block.instructions()
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(index, outer_mask_source)
            || !matches!(
                properties.as_ref(),
                [BytecodeObjectProperty::Static(property)] if property.as_str() == "value"
            )
        {
            return Ok(None);
        }
        let [
            BytecodeInstruction::LoadBinding(record_read),
            BytecodeInstruction::StaticMember { property },
            BytecodeInstruction::PushLiteral(Value::Number(inner_add)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::DeclareBinding {
                name: inner,
                kind: DeclKind::Let,
                has_init: true,
            },
            BytecodeInstruction::LoadBinding(inner_read),
            BytecodeInstruction::LoadBinding(outer_bump_source),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::DeclareBinding {
                name: bump,
                kind: DeclKind::Const,
                has_init: true,
            },
            BytecodeInstruction::LoadBinding(total_read),
            BytecodeInstruction::LoadBinding(bump_read),
            BytecodeInstruction::PushLiteral(Value::Number(bump_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(total_write),
            BytecodeInstruction::StoreLast,
        ] = inner_block.instructions()
        else {
            return Ok(None);
        };
        if property.name().as_str() != "value"
            || !same_bytecode_binding(record, record_read)
            || !same_bytecode_binding(inner, inner_read)
            || !same_bytecode_binding(index, outer_bump_source)
            || !same_bytecode_binding(bump, bump_read)
            || !same_bytecode_binding(total_read, total_write)
        {
            return Ok(None);
        }
        let Ok(outer_mask_i32) = number_to_i32(*outer_mask, "&") else {
            return Ok(None);
        };
        let Ok(bump_mask_i32) = number_to_i32(*bump_mask, "&") else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(total_write)? else {
            return Ok(None);
        };
        if self.builtin_value(total_write.name().name())?.is_some() {
            return Ok(None);
        }
        Ok(Some(BytecodeBlockLexicalLoopFastPath {
            total: total_write,
            total_cell,
            outer_mask_i32,
            inner_add: *inner_add,
            bump_mask_i32,
        }))
    }

    pub(super) fn block_lexical_loop_fast_path_ready(
        fast_path: &BytecodeBlockLexicalLoopFastPath<'_>,
    ) -> Result<bool> {
        Ok(matches!(
            fast_path.total_cell.value(fast_path.total.name())?,
            Value::Number(_)
        ))
    }

    pub(super) fn eval_block_lexical_loop_fast_path(
        &self,
        fast_path: &BytecodeBlockLexicalLoopFastPath<'_>,
        index: f64,
    ) -> Result<Option<Value>> {
        let Value::Number(total) = fast_path.total_cell.value(fast_path.total.name())? else {
            return Ok(None);
        };
        let record_value = masked_number(index, fast_path.outer_mask_i32)?;
        let inner = record_value + fast_path.inner_add;
        let bump = inner + index;
        let masked_bump = masked_number(bump, fast_path.bump_mask_i32)?;
        let value = self.checked_value(Value::Number(total + masked_bump))?;
        fast_path
            .total_cell
            .assign(fast_path.total.name(), value.clone())?;
        Ok(Some(value))
    }
}

fn masked_number(value: f64, mask_i32: i32) -> Result<f64> {
    let masked = number_to_i32(value, "&")? & mask_i32;
    Ok(f64::from(masked))
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}
