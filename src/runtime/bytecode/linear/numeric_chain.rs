use crate::{
    bytecode::{BytecodeBinding, BytecodeInstruction, BytecodeNumericBinaryOp, BytecodeProperty},
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::BindingCell,
        numeric::{number_shift_count, number_to_i32, number_to_uint32},
    },
    syntax::BinaryOp,
    value::Value,
};

#[derive(Debug)]
pub(super) struct CompiledNumericBindingChain<'a> {
    pub(super) op: NumericBindingChain<'a>,
    pub(super) consumed: usize,
}

#[derive(Debug)]
pub(super) struct CompiledNumericCompoundBinding<'a> {
    pub(super) op: NumericCompoundBinding<'a>,
    pub(super) consumed: usize,
}

#[derive(Debug)]
pub(super) struct NumericBindingChain<'a> {
    source: &'a BytecodeBinding,
    source_cell: BindingCell,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    terms: Vec<NumericBindingChainTerm<'a>>,
}

#[derive(Debug)]
enum NumericBindingChainTerm<'a> {
    Literal {
        op: BytecodeNumericBinaryOp,
        right: f64,
    },
    BindingBitAndLiteral {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        mask: f64,
        op: BytecodeNumericBinaryOp,
    },
}

#[derive(Debug)]
pub(super) struct NumericCompoundBinding<'a> {
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    rhs: NumericCompoundRhs<'a>,
    op: BinaryOp,
}

#[derive(Debug)]
enum NumericCompoundRhs<'a> {
    Literal(f64),
    Binding {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
    },
    BindingBitAndLiteral {
        binding: &'a BytecodeBinding,
        cell: BindingCell,
        mask: f64,
    },
    StaticProperty {
        object: &'a BytecodeBinding,
        object_cell: BindingCell,
        property: &'a BytecodeProperty,
    },
}

impl Context {
    pub(super) fn compile_numeric_binding_chain<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledNumericBindingChain<'a>>> {
        let Some(BytecodeInstruction::LoadBinding(source)) = instructions.get(index) else {
            return Ok(None);
        };
        let Some(source_cell) = self.get_binding_bytecode(source)? else {
            return Ok(None);
        };

        let mut cursor = index
            .checked_add(1)
            .ok_or_else(|| Error::runtime("numeric chain index overflowed"))?;
        let mut terms = Vec::new();
        while let Some((term, consumed)) =
            self.compile_numeric_binding_chain_term(instructions, cursor)?
        {
            terms.push(term);
            cursor = cursor
                .checked_add(consumed)
                .ok_or_else(|| Error::runtime("numeric chain index overflowed"))?;
        }
        if terms.is_empty() {
            return Ok(None);
        }

        let Some(
            [
                BytecodeInstruction::StoreBinding(target),
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, cursor, 2)
        else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        let consumed = cursor
            .checked_add(2)
            .and_then(|end| end.checked_sub(index))
            .ok_or_else(|| Error::runtime("numeric chain length overflowed"))?;

        Ok(Some(CompiledNumericBindingChain {
            op: NumericBindingChain {
                source,
                source_cell,
                target,
                target_cell,
                terms,
            },
            consumed,
        }))
    }

    fn compile_numeric_binding_chain_term<'a>(
        &self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<(NumericBindingChainTerm<'a>, usize)>> {
        if let Some(
            [
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::NumberBinary(op),
            ],
        ) = instruction_window(instructions, index, 2)
        {
            return Ok(Some((
                NumericBindingChainTerm::Literal {
                    op: *op,
                    right: *right,
                },
                2,
            )));
        }

        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::NumberBinary(op),
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        Ok(Some((
            NumericBindingChainTerm::BindingBitAndLiteral {
                binding,
                cell,
                mask: *mask,
                op: *op,
            },
            4,
        )))
    }

    pub(super) fn compile_numeric_compound_binding<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledNumericCompoundBinding<'a>>> {
        if let Some(compound) = self.compile_numeric_compound_literal(instructions, index)? {
            return Ok(Some(compound));
        }
        if let Some(compound) = self.compile_numeric_compound_binding_rhs(instructions, index)? {
            return Ok(Some(compound));
        }
        if let Some(compound) =
            self.compile_numeric_compound_static_property_rhs(instructions, index)?
        {
            return Ok(Some(compound));
        }
        self.compile_numeric_compound_binding_bitand_literal(instructions, index)
    }

    fn compile_numeric_compound_literal<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledNumericCompoundBinding<'a>>> {
        let Some(
            [
                BytecodeInstruction::PushLiteral(Value::Number(right)),
                BytecodeInstruction::CompoundStoreBinding { name, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 3)
        else {
            return Ok(None);
        };
        self.compile_numeric_compound_op(name, *op, NumericCompoundRhs::Literal(*right), 3)
    }

    fn compile_numeric_compound_binding_rhs<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledNumericCompoundBinding<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::CompoundStoreBinding { name, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 3)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        self.compile_numeric_compound_op(
            name,
            *op,
            NumericCompoundRhs::Binding { binding, cell },
            3,
        )
    }

    fn compile_numeric_compound_static_property_rhs<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledNumericCompoundBinding<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(object),
                BytecodeInstruction::StaticMember { property },
                BytecodeInstruction::CompoundStoreBinding { name, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 4)
        else {
            return Ok(None);
        };
        let Some(object_cell) = self.get_binding_bytecode(object)? else {
            return Ok(None);
        };
        self.compile_numeric_compound_op(
            name,
            *op,
            NumericCompoundRhs::StaticProperty {
                object,
                object_cell,
                property,
            },
            4,
        )
    }

    fn compile_numeric_compound_binding_bitand_literal<'a>(
        &mut self,
        instructions: &'a [BytecodeInstruction],
        index: usize,
    ) -> Result<Option<CompiledNumericCompoundBinding<'a>>> {
        let Some(
            [
                BytecodeInstruction::LoadBinding(binding),
                BytecodeInstruction::PushLiteral(Value::Number(mask)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
                BytecodeInstruction::CompoundStoreBinding { name, op },
                BytecodeInstruction::StoreLast,
            ],
        ) = instruction_window(instructions, index, 5)
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        self.compile_numeric_compound_op(
            name,
            *op,
            NumericCompoundRhs::BindingBitAndLiteral {
                binding,
                cell,
                mask: *mask,
            },
            5,
        )
    }

    fn compile_numeric_compound_op<'a>(
        &mut self,
        target: &'a BytecodeBinding,
        op: BinaryOp,
        rhs: NumericCompoundRhs<'a>,
        consumed: usize,
    ) -> Result<Option<CompiledNumericCompoundBinding<'a>>> {
        if BytecodeNumericBinaryOp::from_binary(op).is_none() {
            return Ok(None);
        }
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target)? else {
            return Ok(None);
        };
        Ok(Some(CompiledNumericCompoundBinding {
            op: NumericCompoundBinding {
                target,
                target_cell,
                rhs,
                op,
            },
            consumed,
        }))
    }

    pub(super) fn eval_numeric_binding_chain(
        &mut self,
        state: &mut super::BytecodeState,
        chain: &NumericBindingChain<'_>,
    ) -> Result<()> {
        let initial = self.runtime_value(chain.source_cell.value(chain.source.name())?)?;
        if let Value::Number(number) = initial
            && let Some(value) = self.eval_numeric_binding_chain_number(number, &chain.terms)?
        {
            let value = self.checked_value(Value::Number(value))?;
            self.assign_bytecode_cell(chain.target, &chain.target_cell, value.clone())?;
            state.last = value;
            return Ok(());
        }

        let value = self.eval_numeric_binding_chain_slow_path(initial, &chain.terms)?;
        self.assign_bytecode_cell(chain.target, &chain.target_cell, value.clone())?;
        state.last = value;
        Ok(())
    }

    fn eval_numeric_binding_chain_number(
        &mut self,
        mut value: f64,
        terms: &[NumericBindingChainTerm<'_>],
    ) -> Result<Option<f64>> {
        for term in terms {
            let (op, right) = match term {
                NumericBindingChainTerm::Literal { op, right } => (*op, *right),
                NumericBindingChainTerm::BindingBitAndLiteral {
                    binding,
                    cell,
                    mask,
                    op,
                } => {
                    let rhs = self.runtime_value(cell.value(binding.name())?)?;
                    let Value::Number(rhs) = rhs else {
                        return Ok(None);
                    };
                    (
                        *op,
                        apply_number_binary(BytecodeNumericBinaryOp::BitAnd, rhs, *mask)?,
                    )
                }
            };
            value = apply_number_binary(op, value, right)?;
        }
        Ok(Some(value))
    }

    fn eval_numeric_binding_chain_slow_path(
        &mut self,
        mut value: Value,
        terms: &[NumericBindingChainTerm<'_>],
    ) -> Result<Value> {
        for term in terms {
            let (op, right) = match term {
                NumericBindingChainTerm::Literal { op, right } => (*op, Value::Number(*right)),
                NumericBindingChainTerm::BindingBitAndLiteral {
                    binding,
                    cell,
                    mask,
                    op,
                } => {
                    let rhs = self.runtime_value(cell.value(binding.name())?)?;
                    let mask = Value::Number(*mask);
                    (
                        *op,
                        self.eval_bytecode_number_binary(
                            BytecodeNumericBinaryOp::BitAnd,
                            &rhs,
                            &mask,
                        )?,
                    )
                }
            };
            value = self.eval_bytecode_number_binary(op, &value, &right)?;
        }
        Ok(value)
    }

    pub(super) fn eval_numeric_compound_binding(
        &mut self,
        state: &mut super::BytecodeState,
        compound: &NumericCompoundBinding<'_>,
    ) -> Result<()> {
        let current = self.runtime_value(compound.target_cell.value(compound.target.name())?)?;
        let right = self.eval_numeric_compound_rhs(&compound.rhs)?;
        if let (Value::Number(left), Value::Number(right_number)) = (&current, &right)
            && let Some(op) = BytecodeNumericBinaryOp::from_binary(compound.op)
        {
            let value = self.checked_value(Value::Number(apply_number_binary(
                op,
                *left,
                *right_number,
            )?))?;
            self.assign_bytecode_cell(compound.target, &compound.target_cell, value.clone())?;
            state.last = value;
            return Ok(());
        }

        let value = self.eval_bytecode_compound_value(compound.op, &current, &right)?;
        self.assign_bytecode_cell(compound.target, &compound.target_cell, value.clone())?;
        state.last = value;
        Ok(())
    }

    fn eval_numeric_compound_rhs(&mut self, rhs: &NumericCompoundRhs<'_>) -> Result<Value> {
        match rhs {
            NumericCompoundRhs::Literal(value) => Ok(Value::Number(*value)),
            NumericCompoundRhs::Binding { binding, cell } => {
                self.runtime_value(cell.value(binding.name())?)
            }
            NumericCompoundRhs::BindingBitAndLiteral {
                binding,
                cell,
                mask,
            } => {
                let value = self.runtime_value(cell.value(binding.name())?)?;
                self.eval_bytecode_number_binary(
                    BytecodeNumericBinaryOp::BitAnd,
                    &value,
                    &Value::Number(*mask),
                )
            }
            NumericCompoundRhs::StaticProperty {
                object,
                object_cell,
                property,
            } => {
                let object = self.runtime_value(object_cell.value(object.name())?)?;
                self.get_static_property_value(&object, property.name(), property.access())
            }
        }
    }
}

pub(in crate::runtime::bytecode::linear) fn apply_number_binary(
    op: BytecodeNumericBinaryOp,
    left: f64,
    right: f64,
) -> Result<f64> {
    let value = match op {
        BytecodeNumericBinaryOp::Add => left + right,
        BytecodeNumericBinaryOp::Sub => left - right,
        BytecodeNumericBinaryOp::Mul => left * right,
        BytecodeNumericBinaryOp::Div => left / right,
        BytecodeNumericBinaryOp::Rem => left % right,
        BytecodeNumericBinaryOp::Pow => left.powf(right),
        BytecodeNumericBinaryOp::BitAnd => {
            f64::from(number_to_i32(left, "&")? & number_to_i32(right, "&")?)
        }
        BytecodeNumericBinaryOp::BitOr => {
            f64::from(number_to_i32(left, "|")? | number_to_i32(right, "|")?)
        }
        BytecodeNumericBinaryOp::BitXor => {
            f64::from(number_to_i32(left, "^")? ^ number_to_i32(right, "^")?)
        }
        BytecodeNumericBinaryOp::ShiftLeft => {
            f64::from(number_to_i32(left, "<<")?.wrapping_shl(number_shift_count(right, "<<")?))
        }
        BytecodeNumericBinaryOp::ShiftRight => {
            f64::from(number_to_i32(left, ">>")?.wrapping_shr(number_shift_count(right, ">>")?))
        }
        BytecodeNumericBinaryOp::ShiftRightUnsigned => f64::from(
            number_to_uint32(left, ">>>")?.wrapping_shr(number_shift_count(right, ">>>")?),
        ),
    };
    Ok(value)
}

fn instruction_window(
    instructions: &[BytecodeInstruction],
    start: usize,
    len: usize,
) -> Option<&[BytecodeInstruction]> {
    let end = start.checked_add(len)?;
    instructions.get(start..end)
}
