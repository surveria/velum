use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeArrayIndex, BytecodeBinding, BytecodeBlock, BytecodeCompletion,
        BytecodeInstruction, BytecodeNumericBinaryOp, BytecodeProperty, BytecodeSwitchCase,
    },
    error::{Error, Result},
    runtime::{Context, control::Completion},
    value::Value,
};

use super::BytecodeState;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BytecodeSwitchStartIndex {
    Resolved(Option<usize>),
    Unsupported,
}

type DirectSwitchCaseArrayAdd<'a> = (
    &'a BytecodeBinding,
    &'a BytecodeBinding,
    &'a BytecodeProperty,
    &'a BytecodeArrayIndex,
    &'a BytecodeBinding,
    bool,
);

impl Context {
    pub(super) fn eval_bytecode_switch(
        &mut self,
        state: &mut BytecodeState,
        discriminant: &BytecodeBlock,
        cases: &Rc<[BytecodeSwitchCase]>,
        scoped: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let Some(start) = self.bytecode_switch_start_index(discriminant, cases)? else {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(None);
        };
        let completion = if scoped {
            self.push_lexical_scope();
            let completion = self.eval_bytecode_switch_cases(cases, start);
            let removed = self.pop_lexical_scope();
            if removed.is_none() {
                return Err(Error::runtime("bytecode switch lexical scope disappeared"));
            }
            completion?
        } else {
            self.eval_bytecode_switch_cases(cases, start)?
        };
        Ok(Self::store_or_return_completion(state, completion, next))
    }

    fn bytecode_switch_start_index(
        &mut self,
        discriminant: &BytecodeBlock,
        cases: &[BytecodeSwitchCase],
    ) -> Result<Option<usize>> {
        match self.bytecode_numeric_switch_start_index(discriminant, cases)? {
            BytecodeSwitchStartIndex::Resolved(start) => return Ok(start),
            BytecodeSwitchStartIndex::Unsupported => {}
        }

        let discriminant = self.eval_bytecode_expression(discriminant)?;
        self.bytecode_generic_switch_start_index(&discriminant, cases)
    }

    fn bytecode_numeric_switch_start_index(
        &mut self,
        discriminant: &BytecodeBlock,
        cases: &[BytecodeSwitchCase],
    ) -> Result<BytecodeSwitchStartIndex> {
        let mut default_index = None;
        for case in cases {
            let Some(test) = &case.test else {
                continue;
            };
            if bytecode_numeric_switch_case_test(test).is_none() {
                return Ok(BytecodeSwitchStartIndex::Unsupported);
            }
        }

        let Some(discriminant) = self.bytecode_numeric_switch_discriminant(discriminant)? else {
            return Ok(BytecodeSwitchStartIndex::Unsupported);
        };

        for (index, case) in cases.iter().enumerate() {
            let Some(test) = &case.test else {
                default_index = Some(index);
                continue;
            };
            let Some(test) = bytecode_numeric_switch_case_test(test) else {
                return Ok(BytecodeSwitchStartIndex::Unsupported);
            };
            if bytecode_switch_number_equal(test, discriminant) {
                return Ok(BytecodeSwitchStartIndex::Resolved(Some(index)));
            }
        }
        Ok(BytecodeSwitchStartIndex::Resolved(default_index))
    }

    fn bytecode_numeric_switch_discriminant(
        &mut self,
        discriminant: &BytecodeBlock,
    ) -> Result<Option<f64>> {
        let [
            BytecodeInstruction::LoadBinding(binding),
            BytecodeInstruction::PushLiteral(Value::Number(mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::StoreLast,
        ] = discriminant.instructions()
        else {
            return Ok(None);
        };
        let Some(cell) = self.get_binding_bytecode(binding)? else {
            return Ok(None);
        };
        let value = self.runtime_value(cell.value(binding.name())?)?;
        let value = self.eval_bytecode_number_binary(
            BytecodeNumericBinaryOp::BitAnd,
            &value,
            &Value::Number(*mask),
        )?;
        let Value::Number(value) = value else {
            return Ok(None);
        };
        Ok(Some(value))
    }

    fn bytecode_generic_switch_start_index(
        &mut self,
        discriminant: &Value,
        cases: &[BytecodeSwitchCase],
    ) -> Result<Option<usize>> {
        let mut default_index = None;
        for (index, case) in cases.iter().enumerate() {
            let Some(test) = &case.test else {
                default_index = Some(index);
                continue;
            };
            if self.eval_bytecode_expression(test)? == *discriminant {
                return Ok(Some(index));
            }
        }
        Ok(default_index)
    }

    fn eval_bytecode_switch_cases(
        &mut self,
        cases: &[BytecodeSwitchCase],
        start: usize,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        for case in cases.iter().skip(start) {
            let completion =
                if let Some(completion) = self.eval_bytecode_direct_switch_case_body(&case.body)? {
                    completion
                } else {
                    self.eval_bytecode_block(&case.body)?
                };
            if let Some(completion) = Self::switch_case_completion(&mut last, completion) {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_bytecode_direct_switch_case_body(
        &mut self,
        body: &BytecodeBlock,
    ) -> Result<Option<Completion>> {
        let Some((target_read, array, property, index, target_write, has_break)) =
            direct_switch_case_array_add(body)
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(target_read, target_write) {
            return Ok(None);
        }
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target_write)? else {
            return Ok(None);
        };
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        let left = self.runtime_value(target_cell.value(target_write.name())?)?;
        let object = self.runtime_value(array_cell.value(array.name())?)?;
        let element = self.eval_bytecode_array_index_member(&object, property, *index)?;
        let value =
            self.eval_bytecode_number_binary(BytecodeNumericBinaryOp::Add, &left, &element)?;
        self.assign_bytecode_cell(target_write, &target_cell, value.clone())?;
        if has_break {
            return Ok(Some(Completion::Break { label: None, value }));
        }
        Ok(Some(Completion::Normal(value)))
    }

    fn switch_case_completion(last: &mut Value, completion: Completion) -> Option<Completion> {
        match completion {
            Completion::Normal(value) => {
                *last = value;
                None
            }
            Completion::Break { label: None, value } => Some(Completion::Normal(value)),
            completion @ (Completion::Throw(_)
            | Completion::Return(_)
            | Completion::Break { .. }
            | Completion::Continue(_)) => Some(completion),
        }
    }
}

fn bytecode_numeric_switch_case_test(test: &BytecodeBlock) -> Option<f64> {
    let [
        BytecodeInstruction::PushLiteral(Value::Number(value)),
        BytecodeInstruction::StoreLast,
    ] = test.instructions()
    else {
        return None;
    };
    Some(*value)
}

fn direct_switch_case_array_add(body: &BytecodeBlock) -> Option<DirectSwitchCaseArrayAdd<'_>> {
    if let Some(
        [
            BytecodeInstruction::LoadBinding(target_read),
            BytecodeInstruction::LoadBinding(array),
            BytecodeInstruction::ArrayIndexMember { property, index },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
        ],
    ) = exact_instruction_window(body, 6)
    {
        return Some((target_read, array, property, index, target_write, false));
    }
    let Some(
        [
            BytecodeInstruction::LoadBinding(target_read),
            BytecodeInstruction::LoadBinding(array),
            BytecodeInstruction::ArrayIndexMember { property, index },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::Complete(BytecodeCompletion::Break(None)),
        ],
    ) = exact_instruction_window(body, 7)
    else {
        return None;
    };
    Some((target_read, array, property, index, target_write, true))
}

fn exact_instruction_window(body: &BytecodeBlock, len: usize) -> Option<&[BytecodeInstruction]> {
    let instructions = body.instructions();
    if instructions.len() == len {
        return Some(instructions);
    }
    None
}

fn same_bytecode_binding(left: &BytecodeBinding, right: &BytecodeBinding) -> bool {
    left.operand() == right.operand() && left.name().as_str() == right.name().as_str()
}

const fn bytecode_switch_number_equal(left: f64, right: f64) -> bool {
    if left.is_nan() || right.is_nan() {
        return false;
    }
    let left_bits = left.to_bits();
    let right_bits = right.to_bits();
    let left_magnitude = left_bits & !F64_SIGN_BIT;
    let right_magnitude = right_bits & !F64_SIGN_BIT;
    if left_magnitude == 0 && right_magnitude == 0 {
        return true;
    }
    left_bits == right_bits
}

const F64_SIGN_BIT: u64 = 1_u64 << 63;
