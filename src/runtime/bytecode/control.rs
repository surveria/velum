mod for_in;
mod structured_do_while;
mod structured_switch;
mod try_catch;

use crate::{
    bytecode::{BytecodeAddress, BytecodeBlock, BytecodeInstruction},
    error::{Error, Result},
    runtime::control::{Completion, Suspension},
    runtime::{Context, abstract_operations::to_boolean, resource_scope::ScopeDisposal},
    syntax::StaticName,
    value::Value,
};

use super::{
    control_continuation::{
        BytecodeControlRecord, BytecodeControlStateSlot, BytecodeLoopKind, BytecodeLoopPhase,
    },
    for_of::BytecodeForOfParts,
    linear::BytecodeLinearPlan,
    state::{
        BytecodeState, ScopeDisposalResumeBehavior, bytecode_loop_completion, loop_label_matches,
    },
};
use try_catch::BytecodeTryParts;

#[derive(Debug, Clone, Copy)]
struct BytecodeForParts<'a> {
    init: Option<&'a BytecodeBlock>,
    condition: Option<&'a BytecodeBlock>,
    update: Option<&'a BytecodeBlock>,
    body: &'a BytecodeBlock,
    labels: Option<&'a [StaticName]>,
    scoped: bool,
    per_iteration: bool,
}

struct BytecodeForPlans<'a> {
    condition: Option<BytecodeLinearPlan<'a>>,
    body: Option<BytecodeLinearPlan<'a>>,
    update: Option<BytecodeLinearPlan<'a>>,
}

enum StructuredForAction {
    Continue,
    Break,
    Completion(Completion),
}

#[derive(Debug, Clone, PartialEq)]
enum BytecodeCondition {
    Value(bool),
    Completion(Completion),
}

impl<'a> BytecodeForParts<'a> {
    const fn new(
        init: Option<&'a BytecodeBlock>,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
        labels: Option<&'a [StaticName]>,
        scoped: bool,
        per_iteration: bool,
    ) -> Self {
        Self {
            init,
            condition,
            update,
            body,
            labels,
            scoped,
            per_iteration,
        }
    }
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
            BytecodeInstruction::With { body } => self.eval_bytecode_with(state, body, next),
            BytecodeInstruction::For {
                labels,
                init,
                condition,
                update,
                body,
                scoped,
                per_iteration,
            } => {
                let parts = BytecodeForParts::new(
                    init.as_ref(),
                    condition.as_ref(),
                    update.as_ref(),
                    body,
                    labels.as_deref(),
                    *scoped,
                    *per_iteration,
                );
                self.eval_bytecode_for(state, parts, next)
            }
            BytecodeInstruction::ForIn {
                labels,
                target,
                object,
                body,
            } => self.eval_bytecode_for_in(state, labels.as_deref(), target, object, body, next),
            instruction @ BytecodeInstruction::ForOf { .. } => {
                self.eval_bytecode_for_of_instruction(state, instruction, next)
            }
            BytecodeInstruction::DestructurePattern { pattern, mode } => {
                self.eval_bytecode_destructure_instruction(state, pattern, *mode, next)
            }
            instruction @ BytecodeInstruction::Switch { .. } => {
                self.eval_bytecode_switch_instruction(state, instruction, next)
            }
            BytecodeInstruction::Try {
                body,
                body_scoped,
                body_direct_throw,
                catch,
                finally_body,
                finally_scoped,
            } => {
                let parts = BytecodeTryParts::new(
                    body,
                    *body_scoped,
                    body_direct_throw.as_ref(),
                    catch.as_ref(),
                    finally_body.as_ref(),
                    *finally_scoped,
                );
                self.eval_bytecode_try(state, parts, next)
            }
            BytecodeInstruction::Label { label, body } => {
                self.eval_bytecode_label(state, label, body, next)
            }
            instruction @ BytecodeInstruction::ScopedBlock { .. } => {
                self.eval_bytecode_scoped_block_dispatch(state, instruction, next)
            }
            BytecodeInstruction::Jump(target) => {
                state.pc = *target;
                Ok(None)
            }
            BytecodeInstruction::JumpIfFalse(target) => {
                let value = state.stack.pop()?;
                let condition = to_boolean(self, &value)?;
                state.pc = if condition { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::JumpIfFalseKeep(target) => {
                let value = state.stack.peek()?;
                let condition = to_boolean(self, value)?;
                state.pc = if condition { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::JumpIfTrueKeep(target) => {
                let value = state.stack.peek()?;
                let condition = to_boolean(self, value)?;
                state.pc = if condition { *target } else { next };
                Ok(None)
            }
            BytecodeInstruction::JumpIfNullishKeep(target) => {
                let value = state.stack.peek()?;
                state.pc = if matches!(value, Value::Undefined | Value::Null) {
                    *target
                } else {
                    next
                };
                Ok(None)
            }
            BytecodeInstruction::Complete(completion) => {
                state.complete(completion.clone()).map(Some)
            }
            _ => Err(Error::runtime("bytecode control instruction mismatch")),
        }
    }

    fn eval_bytecode_scoped_block_dispatch(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let BytecodeInstruction::ScopedBlock {
            block,
            var_hoist_plan,
            preserve_last,
            push_result,
        } = instruction
        else {
            return Err(Error::runtime("bytecode scoped block instruction mismatch"));
        };
        self.eval_bytecode_scoped_block_instruction(
            state,
            block,
            var_hoist_plan.as_deref(),
            *preserve_last,
            *push_result,
            next,
        )
    }

    fn eval_bytecode_for_of_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let BytecodeInstruction::ForOf {
            labels,
            target,
            object,
            body,
            asynchronous,
        } = instruction
        else {
            return Err(Error::runtime("bytecode for-of instruction mismatch"));
        };
        let parts = BytecodeForOfParts::new(labels.as_deref(), target, object, body, *asynchronous);
        self.eval_bytecode_for_of(state, parts, next)
    }

    fn eval_bytecode_scoped_block_instruction(
        &mut self,
        state: &mut BytecodeState,
        block: &BytecodeBlock,
        var_hoist_plan: Option<&crate::bytecode::BytecodeHoistPlan>,
        preserve_last: bool,
        push_result: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let completion = if let Some(completion) = self.take_resumed_bytecode_child(block)? {
            completion
        } else {
            self.push_lexical_scope()?;
            if let Some(plan) = var_hoist_plan
                && let Err(error) = self.hoist_bytecode_var_declarations(plan)
            {
                if self.pop_lexical_scope()?.is_none() {
                    return Err(Error::runtime("bytecode lexical scope disappeared"));
                }
                return Err(error);
            }
            let result = self.eval_bytecode_block(block);
            if result.as_ref().is_ok_and(Completion::suspends_execution) {
                return result.map(Some);
            }
            match result {
                Ok(completion) => completion,
                Err(error) => {
                    if self.pop_lexical_scope()?.is_none() {
                        return Err(Error::runtime("bytecode lexical scope disappeared"));
                    }
                    return Err(error);
                }
            }
        };
        let Some(removed) = self.pop_lexical_scope()? else {
            return Err(Error::runtime("bytecode lexical scope disappeared"));
        };
        match self.begin_dispose_binding_scope(removed, completion.clone())? {
            ScopeDisposal::Complete(completion) => {
                if push_result && let Completion::Normal(value) = completion {
                    state.stack.push(value);
                    state.pc = next;
                    return Ok(None);
                }
                if preserve_last && matches!(completion, Completion::Normal(_)) {
                    state.pc = next;
                    return Ok(None);
                }
                Ok(Self::store_or_return_completion(state, completion, next))
            }
            ScopeDisposal::Await(awaited) => {
                state.pc = next;
                state.store_scope_disposal(
                    completion,
                    ScopeDisposalResumeBehavior::Continue {
                        preserve_last,
                        push_result,
                    },
                )?;
                state.mark_await_suspended();
                Ok(Some(Completion::Suspend(Suspension::Await(awaited))))
            }
        }
    }

    fn eval_bytecode_with(
        &mut self,
        state: &mut BytecodeState,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        if let Some(completion) = self.take_resumed_bytecode_child(body)? {
            self.pop_with_environment()?;
            return Ok(Self::store_or_return_completion(state, completion, next));
        }
        let value = state.stack.pop()?;
        let object = self.object_to_object(&value)?;
        self.push_with_environment(object)?;
        let result = self.eval_bytecode_block(body);
        if result.as_ref().is_ok_and(Completion::suspends_execution) {
            return result.map(Some);
        }
        self.pop_with_environment()?;
        result.map(|completion| Self::store_or_return_completion(state, completion, next))
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

    fn eval_bytecode_while(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        condition: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let condition_plan = self.bind_bytecode_linear_plan(condition)?;
        let body_plan = self.bind_bytecode_linear_plan(body)?;
        let handle = self
            .push_bytecode_control(BytecodeControlRecord::loop_record(BytecodeLoopKind::While))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let resumes_body =
                *control.loop_state_mut(BytecodeLoopKind::While)?.0 == BytecodeLoopPhase::Body;
            if !resumes_body {
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
                    BytecodeCondition::Completion(completion)
                        if completion.suspends_execution() =>
                    {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Some(completion));
                    }
                    BytecodeCondition::Completion(completion) => {
                        return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                    }
                }
            }
            if !resumes_body && let Err(error) = self.step() {
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
                Completion::Normal(value) | Completion::Continue { label: None, value } => {
                    *last = value;
                }
                Completion::Continue {
                    label: Some(target),
                    value,
                } if loop_label_matches(labels, &target) => *last = value,
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
                completion @ (Completion::TailCall(_)
                | Completion::Break { .. }
                | Completion::Continue { label: Some(_), .. }
                | Completion::Throw(_)
                | Completion::Return(_)
                | Completion::ReturnDirect(_)) => {
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
                completion @ Completion::Suspend(_) => {
                    self.park_bytecode_control(handle, control)?;
                    return Ok(Some(completion));
                }
            }
            *control.loop_state_mut(BytecodeLoopKind::While)?.0 = BytecodeLoopPhase::Condition;
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
        let resumes = self.resumes_bytecode_control();
        if parts.scoped && !resumes {
            self.push_lexical_scope()?;
        }
        let result = self.eval_bytecode_for_loop(state, parts, next);
        if result.as_ref().is_ok_and(|completion| {
            completion
                .as_ref()
                .is_some_and(Completion::suspends_execution)
        }) {
            return result;
        }
        if parts.scoped {
            let Some(removed) = self.pop_lexical_scope()? else {
                return Err(Error::runtime("bytecode for lexical scope disappeared"));
            };
            return match result {
                Ok(completion) => {
                    let was_normal = completion.is_none();
                    let original =
                        completion.unwrap_or_else(|| Completion::Normal(state.last.clone()));
                    match self.begin_dispose_binding_scope(removed, original.clone())? {
                        ScopeDisposal::Complete(completion) => {
                            if was_normal && let Completion::Normal(value) = completion {
                                state.last = value;
                                return Ok(None);
                            }
                            Ok(Some(completion))
                        }
                        ScopeDisposal::Await(awaited) => {
                            state.pc = next;
                            state.store_scope_disposal(
                                original,
                                ScopeDisposalResumeBehavior::Continue {
                                    preserve_last: false,
                                    push_result: false,
                                },
                            )?;
                            state.mark_await_suspended();
                            Ok(Some(Completion::Suspend(Suspension::Await(awaited))))
                        }
                    }
                }
                Err(error) => Err(error),
            };
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
        let resumed =
            *control.loop_state_mut(BytecodeLoopKind::For)?.0 != BytecodeLoopPhase::Initialize;
        if !resumed {
            if let Some(init) = parts.init {
                let completion = self.run_bytecode_control_segment(
                    handle,
                    &mut control,
                    BytecodeControlStateSlot::Condition,
                    |context, init_state| context.eval_bytecode_block_with_state(init, init_state),
                )?;
                if completion.suspends_execution() {
                    self.park_bytecode_control(handle, control)?;
                    return Ok(Some(completion));
                }
                if !matches!(completion, Completion::Normal(_)) {
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
            }
            if parts.per_iteration {
                self.run_bytecode_control_action(handle, &control, Self::freshen_lexical_scope)?;
            }
            *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Condition;
            let reduction = if parts.per_iteration {
                None
            } else {
                self.run_bytecode_control_action(handle, &control, |context| {
                    context.bind_numeric_array_reduction_plan(
                        parts.condition,
                        parts.update,
                        parts.body,
                    )
                })?
            };
            if let Some(reduction) = reduction {
                match self.eval_numeric_array_reduction_plan(state, next, &reduction) {
                    Ok(true) => return self.finish_bytecode_control_result(handle, Ok(None)),
                    Ok(false) => {}
                    Err(error) => {
                        return self.finish_bytecode_control_result(handle, Err(error));
                    }
                }
            }
        }
        let mut plans = self.run_bytecode_control_action(handle, &control, |context| {
            context.compile_structured_for_plans(parts)
        })?;
        loop {
            match self.eval_structured_for_condition(handle, &mut control, parts, &plans)? {
                StructuredForAction::Continue => {}
                StructuredForAction::Break => break,
                StructuredForAction::Completion(completion) => {
                    if completion.suspends_execution() {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Some(completion));
                    }
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
            }
            match self.eval_structured_for_iteration(handle, &mut control, parts, &plans)? {
                StructuredForAction::Continue => {}
                StructuredForAction::Break => break,
                StructuredForAction::Completion(completion) => {
                    if completion.suspends_execution() {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Some(completion));
                    }
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
            }
            if let StructuredForAction::Completion(completion) =
                self.eval_structured_for_update(handle, &mut control, parts, &mut plans)?
            {
                self.park_bytecode_control(handle, control)?;
                return Ok(Some(completion));
            }
        }
        let (_, last) = control.loop_state_mut(BytecodeLoopKind::For)?;
        state.last = std::mem::replace(last, Value::Undefined);
        state.pc = next;
        self.finish_bytecode_control_result(handle, Ok(None))
    }

    fn eval_structured_for_condition(
        &mut self,
        handle: super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        parts: BytecodeForParts<'_>,
        plans: &BytecodeForPlans<'_>,
    ) -> Result<StructuredForAction> {
        let phase = *control.loop_state_mut(BytecodeLoopKind::For)?.0;
        if matches!(phase, BytecodeLoopPhase::Body | BytecodeLoopPhase::Update) {
            return Ok(StructuredForAction::Continue);
        }
        let Some(condition) = parts.condition else {
            return Ok(StructuredForAction::Continue);
        };
        *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Condition;
        let result = self.run_bytecode_control_segment(
            handle,
            control,
            BytecodeControlStateSlot::Condition,
            |context, state| {
                context.eval_bytecode_condition_with_state(
                    condition,
                    plans.condition.as_ref(),
                    state,
                )
            },
        )?;
        Ok(match result {
            BytecodeCondition::Value(true) => StructuredForAction::Continue,
            BytecodeCondition::Value(false) => StructuredForAction::Break,
            BytecodeCondition::Completion(completion) => {
                StructuredForAction::Completion(completion)
            }
        })
    }

    fn eval_structured_for_iteration(
        &mut self,
        handle: super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        parts: BytecodeForParts<'_>,
        plans: &BytecodeForPlans<'_>,
    ) -> Result<StructuredForAction> {
        let phase = *control.loop_state_mut(BytecodeLoopKind::For)?.0;
        if phase == BytecodeLoopPhase::Update {
            return Ok(StructuredForAction::Continue);
        }
        if phase != BytecodeLoopPhase::Body {
            self.run_bytecode_control_action(handle, control, Self::step)?;
        }
        *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Body;
        let completion =
            self.eval_structured_for_body(handle, control, parts.body, plans.body.as_ref())?;
        let (_, last) = control.loop_state_mut(BytecodeLoopKind::For)?;
        let Some(completion) = bytecode_loop_completion(last, completion, parts.labels) else {
            return Ok(StructuredForAction::Continue);
        };
        if let Completion::Normal(value) = completion {
            *last = value;
            return Ok(StructuredForAction::Break);
        }
        Ok(StructuredForAction::Completion(completion))
    }

    fn eval_structured_for_update<'a>(
        &mut self,
        handle: super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        parts: BytecodeForParts<'a>,
        plans: &mut BytecodeForPlans<'a>,
    ) -> Result<StructuredForAction> {
        let phase = *control.loop_state_mut(BytecodeLoopKind::For)?.0;
        if parts.per_iteration && phase != BytecodeLoopPhase::Update {
            self.run_bytecode_control_action(handle, control, Self::freshen_lexical_scope)?;
            *plans = self.run_bytecode_control_action(handle, control, |context| {
                context.compile_structured_for_plans(parts)
            })?;
        }
        if let Some(update) = parts.update {
            *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Update;
            let completion = self.run_bytecode_control_segment(
                handle,
                control,
                BytecodeControlStateSlot::Update,
                |context, state| {
                    context.eval_bytecode_expression_with_plan(update, plans.update.as_ref(), state)
                },
            )?;
            if completion.suspends_execution() {
                return Ok(StructuredForAction::Completion(completion));
            }
            if let Err(error) = completion.into_result() {
                return self.finish_bytecode_control_result(handle, Err(error));
            }
        }
        *control.loop_state_mut(BytecodeLoopKind::For)?.0 = BytecodeLoopPhase::Condition;
        Ok(StructuredForAction::Continue)
    }

    fn compile_structured_for_plans<'a>(
        &mut self,
        parts: BytecodeForParts<'a>,
    ) -> Result<BytecodeForPlans<'a>> {
        let condition = if let Some(condition) = parts.condition {
            self.bind_bytecode_linear_plan(condition)?
        } else {
            None
        };
        let body = self.bind_bytecode_linear_plan(parts.body)?;
        let update = if let Some(update) = parts.update {
            self.bind_bytecode_linear_plan(update)?
        } else {
            None
        };
        Ok(BytecodeForPlans {
            condition,
            body,
            update,
        })
    }

    fn eval_structured_for_body(
        &mut self,
        handle: super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        body: &BytecodeBlock,
        body_plan: Option<&BytecodeLinearPlan<'_>>,
    ) -> Result<Completion> {
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
                Completion::Normal(value) => BytecodeCondition::Value(to_boolean(self, &value)?),
                completion @ (Completion::TailCall(_)
                | Completion::Throw(_)
                | Completion::Return(_)
                | Completion::ReturnDirect(_)
                | Completion::Break { .. }
                | Completion::Continue { .. }
                | Completion::Suspend(_)) => BytecodeCondition::Completion(completion),
            });
        }
        match self.eval_bytecode_block_with_linear_plan(condition, plan, state)? {
            Completion::Normal(value) => Ok(BytecodeCondition::Value(to_boolean(self, &value)?)),
            completion @ (Completion::TailCall(_)
            | Completion::Throw(_)
            | Completion::Return(_)
            | Completion::ReturnDirect(_)
            | Completion::Break { .. }
            | Completion::Continue { .. }
            | Completion::Suspend(_)) => Ok(BytecodeCondition::Completion(completion)),
        }
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
