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
        BytecodeControlHandle, BytecodeControlRecord, BytecodeControlStateSlot, BytecodeSwitchPhase,
    },
    state::BytecodeState,
};

#[derive(Debug)]
enum BytecodeSwitchSelection {
    Selected(BytecodeControlRecord),
    NoMatch(BytecodeControlRecord),
    Completion(BytecodeControlRecord, Completion),
}

enum BytecodeNumericSwitchStart {
    Resolved(Option<usize>),
    Unsupported,
}

fn numeric_switch_case_test(test: &BytecodeBlock) -> Option<f64> {
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
    pub(super) fn eval_bytecode_switch_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let BytecodeInstruction::Switch {
            discriminant,
            cases,
            scoped,
            scope_init,
        } = instruction
        else {
            return Err(Error::runtime("bytecode switch instruction mismatch"));
        };
        self.eval_bytecode_switch(
            state,
            discriminant,
            cases,
            *scoped,
            scope_init.as_ref(),
            next,
        )
    }

    pub(super) fn eval_bytecode_switch(
        &mut self,
        state: &mut BytecodeState,
        discriminant: &BytecodeBlock,
        cases: &Rc<[BytecodeSwitchCase]>,
        scoped: bool,
        scope_init: Option<&BytecodeBlock>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let resumes = self.resumes_bytecode_control();
        let numeric_start = if resumes {
            BytecodeNumericSwitchStart::Unsupported
        } else {
            self.bytecode_numeric_switch_start(discriminant, cases)?
        };
        if let Some(completion) = self.prepare_bytecode_switch_scope(scoped, resumes, scope_init)? {
            return Ok(Some(completion));
        }
        let initial_record = match numeric_start {
            BytecodeNumericSwitchStart::Resolved(Some(start)) => {
                BytecodeControlRecord::switch_at(start)
            }
            BytecodeNumericSwitchStart::Resolved(None) => {
                if scoped {
                    self.pop_bytecode_switch_scope()?;
                }
                state.last = Value::Undefined;
                state.pc = next;
                return Ok(None);
            }
            BytecodeNumericSwitchStart::Unsupported => BytecodeControlRecord::switch(),
        };
        let handle = self.push_bytecode_control(initial_record)?;
        let control = self.checkout_bytecode_control(handle)?;
        let control =
            match self.eval_bytecode_switch_selection(handle, control, discriminant, cases) {
                Ok(BytecodeSwitchSelection::Selected(control)) => control,
                Ok(BytecodeSwitchSelection::NoMatch(_control)) => {
                    if scoped {
                        self.pop_bytecode_switch_scope()?;
                    }
                    self.finish_bytecode_control(handle)?;
                    state.last = Value::Undefined;
                    state.pc = next;
                    return Ok(None);
                }
                Ok(BytecodeSwitchSelection::Completion(control, completion))
                    if completion.suspends_execution() =>
                {
                    self.park_bytecode_control(handle, control)?;
                    return Ok(Some(completion));
                }
                Ok(BytecodeSwitchSelection::Completion(_control, completion)) => {
                    if scoped {
                        self.pop_bytecode_switch_scope()?;
                    }
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
                Err(error) => {
                    if scoped {
                        self.pop_bytecode_switch_scope()?;
                    }
                    return Err(error);
                }
            };
        let result = if scoped {
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

    fn pop_bytecode_switch_scope(&mut self) -> Result<()> {
        if self.pop_lexical_scope()?.is_none() {
            return Err(Error::runtime("bytecode switch lexical scope disappeared"));
        }
        Ok(())
    }

    fn prepare_bytecode_switch_scope(
        &mut self,
        scoped: bool,
        resumes: bool,
        scope_init: Option<&BytecodeBlock>,
    ) -> Result<Option<Completion>> {
        if !scoped || resumes {
            return Ok(None);
        }
        self.push_lexical_scope()?;
        let Some(scope_init) = scope_init else {
            return Ok(None);
        };
        match self.eval_bytecode_block(scope_init) {
            Ok(Completion::Normal(_)) => Ok(None),
            Ok(completion) => {
                self.pop_bytecode_switch_scope()?;
                Ok(Some(completion))
            }
            Err(error) => {
                self.pop_bytecode_switch_scope()?;
                Err(error)
            }
        }
    }

    fn bytecode_numeric_switch_start(
        &mut self,
        discriminant: &BytecodeBlock,
        cases: &[BytecodeSwitchCase],
    ) -> Result<BytecodeNumericSwitchStart> {
        let mut default_index = None;
        for case in cases {
            let Some(test) = &case.test else {
                continue;
            };
            if numeric_switch_case_test(test).is_none() {
                return Ok(BytecodeNumericSwitchStart::Unsupported);
            }
        }
        let Some(discriminant) = self.bytecode_numeric_switch_discriminant(discriminant)? else {
            return Ok(BytecodeNumericSwitchStart::Unsupported);
        };
        for (index, case) in cases.iter().enumerate() {
            let Some(test) = &case.test else {
                default_index = Some(index);
                continue;
            };
            let Some(test) = numeric_switch_case_test(test) else {
                return Ok(BytecodeNumericSwitchStart::Unsupported);
            };
            if number_strict_equality(test, discriminant) {
                return Ok(BytecodeNumericSwitchStart::Resolved(Some(index)));
            }
        }
        Ok(BytecodeNumericSwitchStart::Resolved(default_index))
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

    fn eval_bytecode_switch_selection(
        &mut self,
        handle: BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        discriminant: &BytecodeBlock,
        cases: &[BytecodeSwitchCase],
    ) -> Result<BytecodeSwitchSelection> {
        if *control.switch_selection_mut()?.0 == BytecodeSwitchPhase::Discriminant {
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, selection_state| {
                    context.eval_bytecode_block_with_state(discriminant, selection_state)
                },
            )?;
            match completion {
                Completion::Normal(value) => {
                    let (phase, _, _, stored_discriminant) = control.switch_selection_mut()?;
                    *stored_discriminant = Some(value);
                    *phase = BytecodeSwitchPhase::CaseTest;
                }
                completion => {
                    return Ok(BytecodeSwitchSelection::Completion(control, completion));
                }
            }
        }
        loop {
            let (phase, next_case, default_case, _) = control.switch_selection_mut()?;
            if *phase == BytecodeSwitchPhase::Body {
                return Ok(BytecodeSwitchSelection::Selected(control));
            }
            let Some(case) = cases.get(*next_case) else {
                if let Some(default_case) = *default_case {
                    *next_case = default_case;
                    *phase = BytecodeSwitchPhase::Body;
                    return Ok(BytecodeSwitchSelection::Selected(control));
                }
                return Ok(BytecodeSwitchSelection::NoMatch(control));
            };
            let case_index = *next_case;
            let Some(test) = &case.test else {
                *default_case = Some(case_index);
                *next_case = next_case
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("bytecode switch case index overflowed"))?;
                continue;
            };
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, selection_state| {
                    context.eval_bytecode_block_with_state(test, selection_state)
                },
            )?;
            match completion {
                Completion::Normal(value) => {
                    let (phase, next_case, _, discriminant) = control.switch_selection_mut()?;
                    let discriminant = discriminant.as_ref().ok_or_else(|| {
                        Error::runtime("bytecode switch discriminant disappeared")
                    })?;
                    if value == *discriminant {
                        *phase = BytecodeSwitchPhase::Body;
                        return Ok(BytecodeSwitchSelection::Selected(control));
                    }
                    *next_case = next_case
                        .checked_add(1)
                        .ok_or_else(|| Error::runtime("bytecode switch case index overflowed"))?;
                }
                completion => {
                    return Ok(BytecodeSwitchSelection::Completion(control, completion));
                }
            }
        }
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
                | Completion::Continue { .. }
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
