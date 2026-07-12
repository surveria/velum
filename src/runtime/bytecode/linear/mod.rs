mod direct;
mod in_operator;
mod numeric_array_reduction;
mod numeric_chain;
mod property_chain;
mod property_numeric;
mod segment;

use crate::{
    bytecode::{
        BytecodeArrayIndex, BytecodeBinding, BytecodeDynamicProperty, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
        BytecodeProperty,
    },
    error::{Error, Result},
    runtime::{Context, binding::scope::BindingCell},
    syntax::{BinaryOp, DeclKind, StaticString, UpdateOp},
    value::Value,
};

use super::state::BytecodeState;
use numeric_chain::{NumericBindingChain, NumericCompoundBinding, NumericCompoundChain};
use property_chain::PropertyMutation;
pub(super) use segment::BytecodeLinearPlan;

#[derive(Debug)]
enum BytecodeLinearOp<'a> {
    PushLiteral(&'a Value),
    PushUndefined,
    LoadBinding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
    },
    StoreBinding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
    },
    DeclareVarBinding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        has_init: bool,
    },
    StoreLast,
    Pop,
    UpdateBinding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        op: UpdateOp,
        prefix: bool,
    },
    UpdateBindingStoreLast {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        op: UpdateOp,
        prefix: bool,
    },
    NumberBinary(BytecodeNumericBinaryOp),
    NumberCompare(BytecodeNumericCompareOp),
    NumberEquality(BytecodeNumericEqualityOp),
    CompoundStoreBinding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        op: BinaryOp,
    },
    CompareBindingNumber {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        op: BytecodeNumericCompareOp,
        right: f64,
    },
    DeclareVarFromBindingNumberBinary {
        source: &'a BytecodeBinding,
        source_cell: BindingCell,
        target: &'a BytecodeBinding,
        target_cell: BindingCell,
        op: BytecodeNumericBinaryOp,
        right: f64,
    },
    StoreBindingFromBindingNumberBinary {
        source: &'a BytecodeBinding,
        source_cell: BindingCell,
        target: &'a BytecodeBinding,
        target_cell: BindingCell,
        op: BytecodeNumericBinaryOp,
        right: f64,
    },
    AddArrayElementToBinding {
        target: &'a BytecodeBinding,
        target_cell: BindingCell,
        array: &'a BytecodeBinding,
        array_cell: BindingCell,
        index: &'a BytecodeBinding,
        index_cell: BindingCell,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
    },
    InStaticPropertyBinding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        property: &'a StaticString,
        access: BytecodeDynamicProperty,
        store_last: bool,
    },
    InArrayIndexMaskBinding {
        index: &'a BytecodeBinding,
        index_cell: BindingCell,
        mask: f64,
        array: &'a BytecodeBinding,
        array_cell: BindingCell,
        access: BytecodeDynamicProperty,
    },
    NumericBindingChain(NumericBindingChain<'a>),
    NumericCompoundBinding(NumericCompoundBinding<'a>),
    NumericCompoundChain(NumericCompoundChain<'a>),
    PropertyMutation(PropertyMutation<'a>),
    ArrayLength(&'a BytecodeProperty),
    ArrayIndexMember {
        property: &'a BytecodeProperty,
        index: BytecodeArrayIndex,
    },
    ComputedMember(BytecodeDynamicProperty),
}

impl Context {
    fn compile_bytecode_linear_peephole<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<(BytecodeLinearOp<'a>, usize)>> {
        if let Some(op) = self.compile_compare_binding_number(instructions, index)? {
            return Ok(Some((op, 4)));
        }
        if let Some(op) =
            self.compile_declare_var_from_binding_number_binary(instructions, index)?
        {
            return Ok(Some((op, 4)));
        }
        if let Some(op) =
            self.compile_store_binding_from_binding_number_binary(instructions, index)?
        {
            return Ok(Some((op, 5)));
        }
        if let Some(chain) = self.compile_numeric_binding_chain(instructions, index)? {
            return Ok(Some((
                BytecodeLinearOp::NumericBindingChain(chain.op),
                chain.consumed,
            )));
        }
        if let Some(chain) = self.compile_numeric_compound_chain(instructions, index)? {
            return Ok(Some((
                BytecodeLinearOp::NumericCompoundChain(chain.op),
                chain.consumed,
            )));
        }
        if let Some(compound) = self.compile_numeric_compound_binding(instructions, index)? {
            return Ok(Some((
                BytecodeLinearOp::NumericCompoundBinding(compound.op),
                compound.consumed,
            )));
        }
        if let Some(mutation) = self.compile_property_mutation(instructions, index)? {
            return Ok(Some((
                BytecodeLinearOp::PropertyMutation(mutation.op),
                mutation.consumed,
            )));
        }
        if let Some(op) = self.compile_update_binding_store_last(instructions, index)? {
            return Ok(Some((op, 2)));
        }
        if let Some(op) =
            self.compile_add_array_element_to_binding_with_mask(instructions, index)?
        {
            return Ok(Some((op, 9)));
        }
        if let Some(op) = self.compile_in_static_property_binding(instructions, index)? {
            return Ok(Some(op));
        }
        if let Some(op) = self.compile_in_array_index_mask_binding(instructions, index)? {
            return Ok(Some((op, 5)));
        }
        if let Some(op) = self.compile_add_array_element_to_binding(instructions, index)? {
            return Ok(Some((op, 7)));
        }
        Ok(None)
    }

    fn compile_compare_binding_number<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberCompare(op),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::CompareBindingNumber {
            binding,
            cell,
            op: *op,
            right: *right,
        }))
    }

    fn compile_declare_var_from_binding_number_binary<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(source),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberBinary(op),
                BytecodeInstruction::DeclareBinding {
                    name: target,
                    kind: DeclKind::Var,
                    has_init: true,
                },
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        let Some(source_cell) = self.get_binding_bytecode(source)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_binding_bytecode(target)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::DeclareVarFromBindingNumberBinary {
            source,
            source_cell,
            target,
            target_cell,
            op: *op,
            right: *right,
        }))
    }

    fn compile_store_binding_from_binding_number_binary<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(source),
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberBinary(op),
                BytecodeInstruction::StoreBinding(target),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 5)
        else {
            return Ok(None);
        };
        let Some(source_cell) = self.get_binding_bytecode(source)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        Ok(Some(
            BytecodeLinearOp::StoreBindingFromBindingNumberBinary {
                source,
                source_cell,
                target,
                target_cell,
                op: *op,
                right: *right,
            },
        ))
    }

    fn compile_update_binding_store_last<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::UpdateBinding { name, op, prefix },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 2)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(name)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::UpdateBindingStoreLast {
            binding: name,
            cell,
            op: *op,
            prefix: *prefix,
        }))
    }

    fn compile_add_array_element_to_binding_with_mask<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::ComputedMember { property },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 9)
        else {
            return Ok(None);
        };
        self.compile_add_array_element_to_binding_op(
            target_read,
            target_write,
            array,
            index_binding,
            Some(*mask),
            *property,
        )
    }

    fn compile_add_array_element_to_binding<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(target_read),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::LoadBinding(index_binding),
                BytecodeInstruction::ComputedMember { property },
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(target_write),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 7)
        else {
            return Ok(None);
        };
        self.compile_add_array_element_to_binding_op(
            target_read,
            target_write,
            array,
            index_binding,
            None,
            *property,
        )
    }

    fn compile_add_array_element_to_binding_op<'a>(
        &mut self,
        target_read: &'a BytecodeBinding,
        target_write: &'a BytecodeBinding,
        array: &'a BytecodeBinding,
        index: &'a BytecodeBinding,
        index_mask: Option<f64>,
        property: BytecodeDynamicProperty,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        if !same_bytecode_binding(target_read, target_write) {
            return Ok(None);
        }
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target_write)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(index)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeLinearOp::AddArrayElementToBinding {
            target: target_write,
            target_cell,
            array,
            array_cell,
            index,
            index_cell,
            index_mask,
            property,
        }))
    }

    fn compile_bytecode_linear_op<'a>(
        &mut self,
        instruction: &'a BytecodeInstruction,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        if let Some(op) = self.compile_linear_stack_binding_op(instruction)? {
            return Ok(Some(op));
        }
        if let Some(op) = compile_linear_numeric_member_op(instruction) {
            return Ok(Some(op));
        }
        Ok(None)
    }

    fn compile_linear_stack_binding_op<'a>(
        &mut self,
        instruction: &'a BytecodeInstruction,
    ) -> Result<Option<BytecodeLinearOp<'a>>> {
        let op = match instruction {
            BytecodeInstruction::PushLiteral(value) => BytecodeLinearOp::PushLiteral(value),
            BytecodeInstruction::PushUndefined => BytecodeLinearOp::PushUndefined,
            BytecodeInstruction::LoadBinding(binding) => {
                let Some(cell) = self.get_binding_bytecode(binding)? else {
                    return Ok(None);
                };
                BytecodeLinearOp::LoadBinding { binding, cell }
            }
            BytecodeInstruction::StoreBinding(binding) => {
                let Some(cell) = self.get_or_materialize_binding_bytecode(binding)? else {
                    return Ok(None);
                };
                BytecodeLinearOp::StoreBinding { binding, cell }
            }
            BytecodeInstruction::DeclareBinding {
                name,
                kind: DeclKind::Var,
                has_init,
            } => {
                let Some(cell) = self.get_binding_bytecode(name)? else {
                    return Ok(None);
                };
                BytecodeLinearOp::DeclareVarBinding {
                    binding: name,
                    cell,
                    has_init: *has_init,
                }
            }
            BytecodeInstruction::StoreLast => BytecodeLinearOp::StoreLast,
            BytecodeInstruction::Pop => BytecodeLinearOp::Pop,
            BytecodeInstruction::UpdateBinding { name, op, prefix } => {
                let Some(cell) = self.get_binding_bytecode(name)? else {
                    return Ok(None);
                };
                BytecodeLinearOp::UpdateBinding {
                    binding: name,
                    cell,
                    op: *op,
                    prefix: *prefix,
                }
            }
            BytecodeInstruction::CompoundStoreBinding { name, op } => {
                let Some(cell) = self.get_or_materialize_binding_bytecode(name)? else {
                    return Ok(None);
                };
                BytecodeLinearOp::CompoundStoreBinding {
                    binding: name,
                    cell,
                    op: *op,
                }
            }
            _ => return Ok(None),
        };
        Ok(Some(op))
    }

    fn eval_bytecode_linear_op(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        match op {
            BytecodeLinearOp::PushLiteral(_)
            | BytecodeLinearOp::PushUndefined
            | BytecodeLinearOp::LoadBinding { .. }
            | BytecodeLinearOp::StoreBinding { .. }
            | BytecodeLinearOp::DeclareVarBinding { .. }
            | BytecodeLinearOp::StoreLast
            | BytecodeLinearOp::Pop
            | BytecodeLinearOp::UpdateBinding { .. }
            | BytecodeLinearOp::CompoundStoreBinding { .. } => {
                self.eval_linear_stack_binding_op(state, op)
            }
            BytecodeLinearOp::NumberBinary(_)
            | BytecodeLinearOp::NumberCompare(_)
            | BytecodeLinearOp::NumberEquality(_) => self.eval_linear_numeric_op(state, op),
            BytecodeLinearOp::CompareBindingNumber { .. }
            | BytecodeLinearOp::DeclareVarFromBindingNumberBinary { .. }
            | BytecodeLinearOp::StoreBindingFromBindingNumberBinary { .. }
            | BytecodeLinearOp::AddArrayElementToBinding { .. }
            | BytecodeLinearOp::InStaticPropertyBinding { .. }
            | BytecodeLinearOp::InArrayIndexMaskBinding { .. }
            | BytecodeLinearOp::NumericBindingChain(_)
            | BytecodeLinearOp::NumericCompoundBinding(_)
            | BytecodeLinearOp::NumericCompoundChain(_)
            | BytecodeLinearOp::UpdateBindingStoreLast { .. }
            | BytecodeLinearOp::PropertyMutation(_) => self.eval_linear_peephole_op(state, op),
            BytecodeLinearOp::ArrayLength(_)
            | BytecodeLinearOp::ArrayIndexMember { .. }
            | BytecodeLinearOp::ComputedMember(_) => self.eval_linear_member_op(state, op),
        }
    }

    fn eval_linear_stack_binding_op(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        match op {
            BytecodeLinearOp::PushLiteral(value) => {
                state.stack.push(self.runtime_value((*value).clone())?);
            }
            BytecodeLinearOp::PushUndefined => {
                state.stack.push(Value::Undefined);
            }
            BytecodeLinearOp::LoadBinding { binding, cell } => {
                state
                    .stack
                    .push(self.checked_value(cell.value(binding.name())?)?);
            }
            BytecodeLinearOp::StoreBinding { binding, cell } => {
                let value = state.stack.pop()?;
                self.assign_bytecode_cell(binding, cell, value.clone())?;
                state.stack.push(value);
            }
            BytecodeLinearOp::DeclareVarBinding {
                binding,
                cell,
                has_init,
            } => {
                if *has_init {
                    let value = state.stack.pop()?;
                    self.assign_bytecode_cell(binding, cell, value)?;
                }
            }
            BytecodeLinearOp::StoreLast => {
                state.last = state.stack.pop()?;
            }
            BytecodeLinearOp::Pop => {
                state.stack.pop()?;
            }
            BytecodeLinearOp::UpdateBinding {
                binding,
                cell,
                op,
                prefix,
            } => {
                let old_value = cell.value(binding.name())?;
                let (old_value, new_value) = self.bytecode_update_values(&old_value, *op)?;
                self.checked_value(new_value.clone())?;
                self.assign_bytecode_cell(binding, cell, new_value.clone())?;
                state
                    .stack
                    .push(if *prefix { new_value } else { old_value });
            }
            BytecodeLinearOp::CompoundStoreBinding { binding, cell, op } => {
                let right = state.stack.pop()?;
                let old_value = cell.value(binding.name())?;
                let value = self.eval_bytecode_compound_value(*op, &old_value, &right)?;
                self.assign_bytecode_cell(binding, cell, value.clone())?;
                state.stack.push(value);
            }
            _ => return Err(Error::runtime("bytecode linear stack op mismatch")),
        }
        Ok(())
    }

    fn eval_linear_numeric_op(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        let right = state.stack.pop()?;
        let left = state.stack.pop()?;
        let value = match op {
            BytecodeLinearOp::NumberBinary(op) => {
                self.eval_bytecode_number_binary(*op, &left, &right)?
            }
            BytecodeLinearOp::NumberCompare(op) => {
                self.eval_bytecode_number_compare(*op, &left, &right)?
            }
            BytecodeLinearOp::NumberEquality(op) => {
                self.eval_bytecode_number_equality(*op, &left, &right)?
            }
            _ => return Err(Error::runtime("bytecode linear numeric op mismatch")),
        };
        state.stack.push(value);
        Ok(())
    }

    fn eval_linear_peephole_op(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        match op {
            BytecodeLinearOp::CompareBindingNumber {
                binding,
                cell,
                op,
                right,
            } => {
                let left = self.runtime_value(cell.value(binding.name())?)?;
                state.last =
                    self.eval_bytecode_number_compare(*op, &left, &Value::Number(*right))?;
            }
            BytecodeLinearOp::DeclareVarFromBindingNumberBinary {
                source,
                source_cell,
                target,
                target_cell,
                op,
                right,
            } => {
                let left = self.runtime_value(source_cell.value(source.name())?)?;
                let value = self.eval_bytecode_number_binary(*op, &left, &Value::Number(*right))?;
                self.assign_bytecode_cell(target, target_cell, value)?;
                state.last = Value::Undefined;
            }
            BytecodeLinearOp::StoreBindingFromBindingNumberBinary {
                source,
                source_cell,
                target,
                target_cell,
                op,
                right,
            } => {
                let left = self.runtime_value(source_cell.value(source.name())?)?;
                let value = self.eval_bytecode_number_binary(*op, &left, &Value::Number(*right))?;
                self.assign_bytecode_cell(target, target_cell, value.clone())?;
                state.last = value;
            }
            BytecodeLinearOp::AddArrayElementToBinding { .. } => {
                self.eval_add_array_element_to_binding(state, op)?;
            }
            BytecodeLinearOp::InStaticPropertyBinding { .. } => {
                self.eval_in_static_property_binding(state, op)?;
            }
            BytecodeLinearOp::InArrayIndexMaskBinding { .. } => {
                self.eval_in_array_index_mask_binding(state, op)?;
            }
            BytecodeLinearOp::NumericBindingChain(chain) => {
                self.eval_numeric_binding_chain(state, chain)?;
            }
            BytecodeLinearOp::NumericCompoundBinding(compound) => {
                self.eval_numeric_compound_binding(state, compound)?;
            }
            BytecodeLinearOp::NumericCompoundChain(chain) => {
                self.eval_numeric_compound_chain(state, chain)?;
            }
            BytecodeLinearOp::UpdateBindingStoreLast {
                binding,
                cell,
                op,
                prefix,
            } => {
                let old_value = cell.value(binding.name())?;
                let (old_value, new_value) = self.bytecode_update_values(&old_value, *op)?;
                self.checked_value(new_value.clone())?;
                self.assign_bytecode_cell(binding, cell, new_value.clone())?;
                state.last = if *prefix { new_value } else { old_value };
            }
            BytecodeLinearOp::PropertyMutation(mutation) => {
                self.eval_property_mutation(state, mutation)?;
            }
            _ => return Err(Error::runtime("bytecode linear peephole op mismatch")),
        }
        Ok(())
    }

    fn eval_add_array_element_to_binding(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        let BytecodeLinearOp::AddArrayElementToBinding {
            target,
            target_cell,
            array,
            array_cell,
            index,
            index_cell,
            index_mask,
            property,
        } = op
        else {
            return Err(Error::runtime("bytecode linear array add op mismatch"));
        };
        let left = self.runtime_value(target_cell.value(target.name())?)?;
        let object = self.runtime_value(array_cell.value(array.name())?)?;
        let mut property_value = self.runtime_value(index_cell.value(index.name())?)?;
        if let Some(mask) = index_mask {
            property_value = self.eval_bytecode_number_binary(
                BytecodeNumericBinaryOp::BitAnd,
                &property_value,
                &Value::Number(*mask),
            )?;
        }
        let element =
            if let Some(value) = self.eval_dynamic_array_index_member(&object, &property_value)? {
                value
            } else {
                let key = self.dynamic_property_key(&property_value)?;
                self.get_cached_dynamic_property_value(&object, &key, property.access())?
            };
        let value =
            self.eval_bytecode_number_binary(BytecodeNumericBinaryOp::Add, &left, &element)?;
        self.assign_bytecode_cell(target, target_cell, value.clone())?;
        state.last = value;
        Ok(())
    }

    fn eval_linear_member_op(
        &mut self,
        state: &mut BytecodeState,
        op: &BytecodeLinearOp<'_>,
    ) -> Result<()> {
        match op {
            BytecodeLinearOp::ArrayLength(property) => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_array_length(&object, property)?);
            }
            BytecodeLinearOp::ArrayIndexMember { property, index } => {
                let object = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_array_index_member(&object, property, *index)?);
            }
            BytecodeLinearOp::ComputedMember(property) => {
                let property_value = state.stack.pop()?;
                let object = state.stack.pop()?;
                if let Some(value) =
                    self.eval_dynamic_array_index_member(&object, &property_value)?
                {
                    state.stack.push(value);
                    return Ok(());
                }
                let key = self.dynamic_property_key(&property_value)?;
                state.stack.push(self.get_cached_dynamic_property_value(
                    &object,
                    &key,
                    property.access(),
                )?);
            }
            _ => return Err(Error::runtime("bytecode linear member op mismatch")),
        }
        Ok(())
    }
}

fn instruction_window(
    instructions: &[BytecodeInstruction],
    start: usize,
    len: usize,
) -> Option<&[BytecodeInstruction]> {
    let end = start.checked_add(len)?;
    instructions.get(start..end)
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}

const fn compile_linear_numeric_member_op(
    instruction: &BytecodeInstruction,
) -> Option<BytecodeLinearOp<'_>> {
    let op = match instruction {
        BytecodeInstruction::NumberBinary(op) => BytecodeLinearOp::NumberBinary(*op),
        BytecodeInstruction::NumberCompare(op) => BytecodeLinearOp::NumberCompare(*op),
        BytecodeInstruction::NumberEquality(op) => BytecodeLinearOp::NumberEquality(*op),
        BytecodeInstruction::ArrayLength { property } => BytecodeLinearOp::ArrayLength(property),
        BytecodeInstruction::ArrayIndexMember { property, index } => {
            BytecodeLinearOp::ArrayIndexMember {
                property,
                index: *index,
            }
        }
        BytecodeInstruction::ComputedMember { property } => {
            BytecodeLinearOp::ComputedMember(*property)
        }
        _ => return None,
    };
    Some(op)
}
