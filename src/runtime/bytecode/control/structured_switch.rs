use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBlock, BytecodeInstruction, BytecodeNumericBinaryOp,
        BytecodeSwitchCase,
    },
    error::{Error, Result},
    runtime::{Context, abstract_operations::number_strict_equality, control::Completion},
    value::Value,
};

use super::super::{
    control_continuation::{
        BytecodeControlHandle, BytecodeControlRecord, BytecodeControlStateSlot,
    },
    state::BytecodeState,
};

#[derive(Debug, Clone, PartialEq)]
enum BytecodeSwitchStartIndex {
    Resolved(Option<usize>),
    Completion(Completion),
    Unsupported,
}

pub(super) fn numeric_switch_case_test(test: &BytecodeBlock) -> Option<f64> {
    let [
        BytecodeInstruction::PushLiteral(Value::Number(value)),
        BytecodeInstruction::StoreLast,
    ] = test.instructions()
    else {
        return None;
    };
    Some(*value)
}

impl Context {
    pub(super) fn eval_bytecode_switch(
        &mut self,
        state: &mut BytecodeState,
        discriminant: &BytecodeBlock,
        cases: &Rc<[BytecodeSwitchCase]>,
        scoped: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let start = match self.bytecode_switch_start_index(discriminant, cases)? {
            BytecodeSwitchStartIndex::Resolved(Some(start)) => start,
            BytecodeSwitchStartIndex::Resolved(None) => {
                state.last = Value::Undefined;
                state.pc = next;
                return Ok(None);
            }
            BytecodeSwitchStartIndex::Completion(completion) => return Ok(Some(completion)),
            BytecodeSwitchStartIndex::Unsupported => {
                return Err(Error::runtime("bytecode switch start remained unresolved"));
            }
        };
        let resumes = self.resumes_bytecode_control();
        let handle = self.push_bytecode_control(BytecodeControlRecord::switch(start))?;
        let control = self.checkout_bytecode_control(handle)?;
        let result = if scoped {
            if !resumes && let Err(error) = self.push_lexical_scope() {
                return self.finish_bytecode_control_result(handle, Err(error));
            }
            let completion = self.eval_bytecode_switch_cases(handle, control, cases);
            if completion
                .as_ref()
                .is_ok_and(|(_, completion)| completion.suspends_execution())
            {
                completion
            } else {
                let removed = self.pop_lexical_scope();
                match completion {
                    Err(error) => {
                        removed?;
                        return Err(error);
                    }
                    Ok(completion) => match removed {
                        Ok(Some(_scope)) => Ok(completion),
                        Ok(None) => {
                            return self.finish_bytecode_control_result(
                                handle,
                                Err(Error::runtime("bytecode switch lexical scope disappeared")),
                            );
                        }
                        Err(error) => {
                            return self.finish_bytecode_control_result(handle, Err(error));
                        }
                    },
                }
            }
        } else {
            self.eval_bytecode_switch_cases(handle, control, cases)
        };
        let (control, completion) = result?;
        if completion.suspends_execution() {
            self.park_bytecode_control(handle, control)?;
            return Ok(Some(completion));
        }
        self.finish_bytecode_control_result(
            handle,
            Ok(Self::store_or_return_completion(state, completion, next)),
        )
    }

    fn bytecode_switch_start_index(
        &mut self,
        discriminant: &BytecodeBlock,
        cases: &[BytecodeSwitchCase],
    ) -> Result<BytecodeSwitchStartIndex> {
        match self.bytecode_numeric_switch_start_index(discriminant, cases)? {
            start @ (BytecodeSwitchStartIndex::Resolved(_)
            | BytecodeSwitchStartIndex::Completion(_)) => return Ok(start),
            BytecodeSwitchStartIndex::Unsupported => {}
        }
        let discriminant = match self.eval_bytecode_block(discriminant)? {
            Completion::Normal(value) => value,
            completion => return Ok(BytecodeSwitchStartIndex::Completion(completion)),
        };
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
            if numeric_switch_case_test(test).is_none() {
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
            let Some(test) = numeric_switch_case_test(test) else {
                return Ok(BytecodeSwitchStartIndex::Unsupported);
            };
            if number_strict_equality(test, discriminant) {
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
    ) -> Result<BytecodeSwitchStartIndex> {
        let mut default_index = None;
        for (index, case) in cases.iter().enumerate() {
            let Some(test) = &case.test else {
                default_index = Some(index);
                continue;
            };
            let value = match self.eval_bytecode_block(test)? {
                Completion::Normal(value) => value,
                completion => return Ok(BytecodeSwitchStartIndex::Completion(completion)),
            };
            if value == *discriminant {
                return Ok(BytecodeSwitchStartIndex::Resolved(Some(index)));
            }
        }
        Ok(BytecodeSwitchStartIndex::Resolved(default_index))
    }

    fn eval_bytecode_switch_cases(
        &mut self,
        handle: BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        cases: &[BytecodeSwitchCase],
    ) -> Result<(BytecodeControlRecord, Completion)> {
        loop {
            let (next_case, _) = control.switch_state_mut()?;
            let Some(case) = cases.get(*next_case) else {
                break;
            };
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, body_state| {
                    context.eval_bytecode_block_with_state(&case.body, body_state)
                },
            )?;
            if !completion.suspends_execution() {
                let (next_case, _) = control.switch_state_mut()?;
                *next_case = next_case
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("bytecode switch case index overflowed"))?;
            }
            let (_, last) = control.switch_state_mut()?;
            match completion {
                Completion::Normal(value) => *last = value,
                Completion::Break { label: None, value } => {
                    return Ok((control, Completion::Normal(value)));
                }
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::ReturnDirect(_)
                | Completion::Break { .. }
                | Completion::Continue(_)
                | Completion::Suspended(_)
                | Completion::GeneratorStart
                | Completion::Yielded(_)
                | Completion::YieldedIteratorResult(_)) => return Ok((control, completion)),
            }
        }
        let (_, last) = control.switch_state_mut()?;
        let value = std::mem::replace(last, Value::Undefined);
        Ok((control, Completion::Normal(value)))
    }
}
