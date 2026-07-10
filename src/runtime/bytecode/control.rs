mod array_add_loop;
mod array_fill_loop;
mod block_lexical_loop;
mod compound_assignment_loop;
mod constructor_prototype_loop;
mod for_in;
mod for_loop;
mod function_apply_has_instance_loop;
mod loop_helpers;
mod object_literal_loop;
mod string_concat_loop;
mod structured_do_while;
mod switch_for_loop;
mod try_catch;
mod try_catch_loop;
mod try_finally_loop;
mod update_expression_loop;
mod while_loop;

use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBlock, BytecodeInstruction, BytecodeNumericBinaryOp,
        BytecodeSwitchCase,
    },
    error::{Error, Result},
    runtime::control::Completion,
    runtime::{
        Context,
        abstract_operations::{number_strict_equality, to_boolean},
    },
    syntax::StaticName,
    value::Value,
};

use super::{
    control_continuation::{
        BytecodeControlRecord, BytecodeControlStateSlot, BytecodeLoopKind, BytecodeLoopPhase,
    },
    linear::BytecodeLinearPlan,
    state::{
        BytecodeState, bytecode_loop_completion, init_completion_to_result, loop_label_matches,
    },
};
use for_loop::BytecodeForBodyFastPath;
use try_catch::BytecodeTryParts;

#[derive(Debug, Clone, Copy)]
struct BytecodeForParts<'a> {
    init: Option<&'a BytecodeBlock>,
    condition: Option<&'a BytecodeBlock>,
    update: Option<&'a BytecodeBlock>,
    body: &'a BytecodeBlock,
    labels: Option<&'a [StaticName]>,
    scoped: bool,
}

struct BytecodeForPlans<'a> {
    condition: Option<BytecodeLinearPlan<'a>>,
    body_fast_path: Option<BytecodeForBodyFastPath<'a>>,
    body: Option<BytecodeLinearPlan<'a>>,
    update: Option<BytecodeLinearPlan<'a>>,
}

#[derive(Debug, Clone, PartialEq)]
enum BytecodeCondition {
    Value(bool),
    Completion(Completion),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum BytecodeSwitchStartIndex {
    Resolved(Option<usize>),
    Unsupported,
}

impl<'a> BytecodeForParts<'a> {
    const fn new(
        init: Option<&'a BytecodeBlock>,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
        labels: Option<&'a [StaticName]>,
        scoped: bool,
    ) -> Self {
        Self {
            init,
            condition,
            update,
            body,
            labels,
            scoped,
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

impl Context {
    pub(super) fn eval_bytecode_control_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::While {
                labels,
                condition,
                body,
            } => self.eval_bytecode_while(state, labels.as_deref(), condition, body, next),
            BytecodeInstruction::DoWhile {
                labels,
                body,
                condition,
            } => self.eval_bytecode_do_while(state, labels.as_deref(), body, condition, next),
            BytecodeInstruction::For {
                labels,
                init,
                condition,
                update,
                body,
                scoped,
            } => {
                let parts = BytecodeForParts::new(
                    init.as_ref(),
                    condition.as_ref(),
                    update.as_ref(),
                    body,
                    labels.as_deref(),
                    *scoped,
                );
                self.eval_bytecode_for(state, parts, next)
            }
            BytecodeInstruction::ForIn {
                labels,
                target,
                object,
                body,
            } => self.eval_bytecode_for_in(state, labels.as_deref(), target, object, body, next),
            BytecodeInstruction::ForOf {
                labels,
                target,
                object,
                body,
            } => self.eval_bytecode_for_of(state, labels.as_deref(), target, object, body, next),
            BytecodeInstruction::DestructurePattern { pattern, kind } => {
                self.eval_bytecode_destructure_instruction(state, pattern, *kind, next)
            }
            BytecodeInstruction::Switch {
                discriminant,
                cases,
                scoped,
            } => self.eval_bytecode_switch(state, discriminant, cases, *scoped, next),
            BytecodeInstruction::Try {
                body,
                body_scoped,
                body_direct_throw,
                try_fast_path,
                catch,
                finally_body,
                finally_scoped,
            } => {
                let parts = BytecodeTryParts::new(
                    body,
                    *body_scoped,
                    body_direct_throw.as_ref(),
                    try_fast_path.as_deref(),
                    catch.as_ref(),
                    finally_body.as_ref(),
                    *finally_scoped,
                );
                self.eval_bytecode_try(state, parts, next)
            }
            BytecodeInstruction::Label { label, body } => {
                self.eval_bytecode_label(state, label, body, next)
            }
            BytecodeInstruction::ScopedBlock(block) => {
                let completion = self.eval_bytecode_scoped_block(block)?;
                Ok(Self::store_or_return_completion(state, completion, next))
            }
            BytecodeInstruction::Jump(target) => {
                state.pc = *target;
                Ok(None)
            }
            BytecodeInstruction::JumpIfFalse(target) => {
                let value = state.stack.pop()?;
                state.pc = if to_boolean(&value) { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::JumpIfFalseKeep(target) => {
                let value = state.stack.peek()?;
                state.pc = if to_boolean(value) { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::JumpIfTrueKeep(target) => {
                let value = state.stack.peek()?;
                state.pc = if to_boolean(value) { *target } else { next };
                Ok(None)
            }
            BytecodeInstruction::Complete(completion) => {
                state.complete(completion.clone()).map(Some)
            }
            _ => Err(Error::runtime("bytecode control instruction mismatch")),
        }
    }

    pub(super) fn store_or_return_completion(
        state: &mut BytecodeState,
        completion: Completion,
        next: BytecodeAddress,
    ) -> Option<Completion> {
        match completion {
            Completion::Normal(value) => {
                state.last = value;
                state.pc = next;
                None
            }
            completion => Some(completion),
        }
    }

    fn eval_bytecode_scoped_block(&mut self, block: &BytecodeBlock) -> Result<Completion> {
        self.push_lexical_scope()?;
        let result = self.eval_bytecode_block(block);
        let removed = self.pop_lexical_scope()?;
        if removed.is_none() {
            return Err(Error::runtime("bytecode lexical scope disappeared"));
        }
        result
    }

    fn eval_bytecode_while(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        condition: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        if let Some(fast_path) = self.compile_bytecode_while_loop_fast_path(condition, body)?
            && self.bytecode_while_loop_fast_path_ready(&fast_path)?
        {
            return self.eval_bytecode_while_loop_fast_path(state, next, &fast_path);
        }
        let condition_plan = self.compile_bytecode_linear_plan(condition)?;
        let body_plan = self.compile_bytecode_linear_plan(body)?;
        let handle = self
            .push_bytecode_control(BytecodeControlRecord::loop_record(BytecodeLoopKind::While))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            *control.loop_state_mut(BytecodeLoopKind::While)?.0 = BytecodeLoopPhase::Condition;
            let condition_result = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Condition,
                |context, condition_state| {
                    context.eval_bytecode_condition_with_state(
                        condition,
                        condition_plan.as_ref(),
                        condition_state,
                    )
                },
            )?;
            match condition_result {
                BytecodeCondition::Value(true) => {}
                BytecodeCondition::Value(false) => break,
                BytecodeCondition::Completion(completion) => {
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
            }
            if let Err(error) = self.step() {
                return self.finish_bytecode_control_result(handle, Err(error));
            }
            *control.loop_state_mut(BytecodeLoopKind::While)?.0 = BytecodeLoopPhase::Body;
            let body_completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, body_state| {
                    context.eval_bytecode_block_with_linear_plan(
                        body,
                        body_plan.as_ref(),
                        body_state,
                    )
                },
            )?;
            let (_, last) = control.loop_state_mut(BytecodeLoopKind::While)?;
            match body_completion {
                Completion::Normal(value) => *last = value,
                Completion::Continue(None) => {}
                Completion::Continue(Some(target)) if loop_label_matches(labels, &target) => {}
                Completion::Break { label: None, value } => {
                    *last = value;
                    break;
                }
                Completion::Break {
                    label: Some(target),
                    value,
                } if loop_label_matches(labels, &target) => {
                    *last = value;
                    break;
                }
                completion @ (Completion::Break { .. }
                | Completion::Continue(Some(_))
                | Completion::Throw(_)
                | Completion::Return(_)) => {
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
            }
        }
        let (_, last) = control.loop_state_mut(BytecodeLoopKind::While)?;
        state.last = std::mem::replace(last, Value::Undefined);
        state.pc = next;
        self.finish_bytecode_control_result(handle, Ok(None))
    }

    fn eval_bytecode_for(
        &mut self,
        state: &mut BytecodeState,
        parts: BytecodeForParts<'_>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        if parts.scoped {
            self.push_lexical_scope()?;
        }
        let result = self.eval_bytecode_for_loop(state, parts, next);
        if parts.scoped {
            let removed = self.pop_lexical_scope()?;
            if removed.is_none() {
                return Err(Error::runtime("bytecode for lexical scope disappeared"));
            }
        }
        result
    }

    fn eval_bytecode_for_loop(
        &mut self,
        state: &mut BytecodeState,
        parts: BytecodeForParts<'_>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let handle =
            self.push_bytecode_control(BytecodeControlRecord::loop_record(BytecodeLoopKind::For))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        self.eval_structured_for_init(handle, &mut control, parts.init)?;
        let fast_path = self.run_bytecode_control_action(handle, &control, |context| {
            let fast_path = context.compile_bytecode_for_loop_fast_path(
                parts.condition,
                parts.update,
                parts.body,
            )?;
            match fast_path {
                Some(fast_path) if context.bytecode_for_loop_fast_path_ready(&fast_path)? => {
                    Ok(Some(fast_path))
                }
                Some(_) | None => Ok(None),
            }
        })?;
        if let Some(fast_path) = fast_path {
            let result = self.eval_bytecode_for_loop_fast_path(state, next, &fast_path);
            return self.finish_bytecode_control_result(handle, result);
        }
        let plans = self.run_bytecode_control_action(handle, &control, |context| {
            context.compile_structured_for_plans(parts)
        })?;
        loop {
            if let Some(condition) = parts.condition {
                *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Condition;
                let condition_result = self.run_bytecode_control_segment(
                    handle,
                    &mut control,
                    BytecodeControlStateSlot::Condition,
                    |context, condition_state| {
                        context.eval_bytecode_condition_with_state(
                            condition,
                            plans.condition.as_ref(),
                            condition_state,
                        )
                    },
                )?;
                match condition_result {
                    BytecodeCondition::Value(true) => {}
                    BytecodeCondition::Value(false) => break,
                    BytecodeCondition::Completion(completion) => {
                        return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                    }
                }
            }
            if let Err(error) = self.step() {
                return self.finish_bytecode_control_result(handle, Err(error));
            }
            *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Body;
            let body_completion = self.eval_structured_for_body(
                handle,
                &mut control,
                parts.body,
                plans.body_fast_path.as_ref(),
                plans.body.as_ref(),
            )?;
            let (_, last) = control.loop_state_mut(BytecodeLoopKind::For)?;
            if let Some(completion) = bytecode_loop_completion(last, body_completion, parts.labels)
            {
                if let Completion::Normal(value) = completion {
                    *last = value;
                    break;
                }
                return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
            }
            if let Some(update) = parts.update {
                *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Update;
                let _value = self.run_bytecode_control_segment(
                    handle,
                    &mut control,
                    BytecodeControlStateSlot::Update,
                    |context, update_state| {
                        context.eval_bytecode_expression_with_plan(
                            update,
                            plans.update.as_ref(),
                            update_state,
                        )
                    },
                )?;
            }
        }
        let (_, last) = control.loop_state_mut(BytecodeLoopKind::For)?;
        state.last = std::mem::replace(last, Value::Undefined);
        state.pc = next;
        self.finish_bytecode_control_result(handle, Ok(None))
    }

    fn eval_structured_for_init(
        &mut self,
        handle: super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        init: Option<&BytecodeBlock>,
    ) -> Result<()> {
        let Some(init) = init else {
            return Ok(());
        };
        let completion = self.run_bytecode_control_segment(
            handle,
            control,
            BytecodeControlStateSlot::Condition,
            |context, init_state| context.eval_bytecode_block_with_state(init, init_state),
        )?;
        if let Err(error) = init_completion_to_result(completion) {
            return self.finish_bytecode_control_result(handle, Err(error));
        }
        Ok(())
    }

    fn compile_structured_for_plans<'a>(
        &mut self,
        parts: BytecodeForParts<'a>,
    ) -> Result<BytecodeForPlans<'a>> {
        let condition = if let Some(condition) = parts.condition {
            self.compile_bytecode_linear_plan(condition)?
        } else {
            None
        };
        let body_fast_path = self.compile_bytecode_for_body_fast_path(parts.body)?;
        let body = if body_fast_path.is_none() {
            self.compile_bytecode_linear_plan(parts.body)?
        } else {
            None
        };
        let update = if let Some(update) = parts.update {
            self.compile_bytecode_linear_plan(update)?
        } else {
            None
        };
        Ok(BytecodeForPlans {
            condition,
            body_fast_path,
            body,
            update,
        })
    }

    fn eval_structured_for_body(
        &mut self,
        handle: super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        body: &BytecodeBlock,
        fast_path: Option<&BytecodeForBodyFastPath<'_>>,
        body_plan: Option<&BytecodeLinearPlan<'_>>,
    ) -> Result<Completion> {
        if let Some(fast_path) = fast_path {
            return self.run_bytecode_control_action(handle, control, |context| {
                context.eval_bytecode_for_body_fast_path(fast_path)
            });
        }
        self.run_bytecode_control_segment(
            handle,
            control,
            BytecodeControlStateSlot::Body,
            |context, body_state| {
                context.eval_bytecode_block_with_linear_plan(body, body_plan, body_state)
            },
        )
    }

    fn eval_bytecode_condition_with_state(
        &mut self,
        condition: &BytecodeBlock,
        plan: Option<&BytecodeLinearPlan<'_>>,
        state: &mut BytecodeState,
    ) -> Result<BytecodeCondition> {
        if let Some(completion) = self.eval_bytecode_linear_direct_condition(condition, plan)? {
            return Ok(match completion {
                Completion::Normal(value) => BytecodeCondition::Value(to_boolean(&value)),
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::Break { .. }
                | Completion::Continue(_)) => BytecodeCondition::Completion(completion),
            });
        }
        match self.eval_bytecode_block_with_linear_plan(condition, plan, state)? {
            Completion::Normal(value) => Ok(BytecodeCondition::Value(to_boolean(&value))),
            completion @ (Completion::Throw(_)
            | Completion::Return(_)
            | Completion::Break { .. }
            | Completion::Continue(_)) => Ok(BytecodeCondition::Completion(completion)),
        }
    }

    fn eval_bytecode_switch(
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
        let handle = self.push_bytecode_control(BytecodeControlRecord::switch(start))?;
        let control = self.checkout_bytecode_control(handle)?;
        let result = if scoped {
            if let Err(error) = self.push_lexical_scope() {
                return self.finish_bytecode_control_result(handle, Err(error));
            }
            let completion = self.eval_bytecode_switch_cases(handle, control, cases);
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
        } else {
            self.eval_bytecode_switch_cases(handle, control, cases)
        };
        let (_, completion) = result?;
        self.finish_bytecode_control_result(
            handle,
            Ok(Self::store_or_return_completion(state, completion, next)),
        )
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
        handle: super::control_continuation::BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        cases: &[BytecodeSwitchCase],
    ) -> Result<(BytecodeControlRecord, Completion)> {
        loop {
            let (next_case, _) = control.switch_state_mut()?;
            let Some(case) = cases.get(*next_case) else {
                break;
            };
            *next_case = next_case
                .checked_add(1)
                .ok_or_else(|| Error::runtime("bytecode switch case index overflowed"))?;
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, body_state| {
                    context.eval_bytecode_block_with_state(&case.body, body_state)
                },
            )?;
            let (_, last) = control.switch_state_mut()?;
            match completion {
                Completion::Normal(value) => *last = value,
                Completion::Break { label: None, value } => {
                    return Ok((control, Completion::Normal(value)));
                }
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::Break { .. }
                | Completion::Continue(_)) => return Ok((control, completion)),
            }
        }
        let (_, last) = control.switch_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        Ok((control, completion))
    }

    fn eval_bytecode_label(
        &mut self,
        state: &mut BytecodeState,
        label: &crate::syntax::StaticName,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match self.eval_bytecode_block(body)? {
            Completion::Break {
                label: Some(target),
                value,
            } if target == *label => {
                state.last = value;
                state.pc = next;
                Ok(None)
            }
            completion => Ok(Self::store_or_return_completion(state, completion, next)),
        }
    }
}
