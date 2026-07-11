use crate::{
    bytecode::{BytecodeAddress, BytecodeBlock},
    error::Result,
    runtime::{Context, control::Completion},
    syntax::StaticName,
    value::Value,
};

use super::{
    super::{
        control_continuation::{
            BytecodeControlRecord, BytecodeControlStateSlot, BytecodeLoopKind, BytecodeLoopPhase,
        },
        state::{BytecodeState, loop_label_matches},
    },
    BytecodeCondition,
};

impl Context {
    pub(super) fn eval_bytecode_do_while(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        body: &BytecodeBlock,
        condition: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let body_plan = self.compile_bytecode_linear_plan(body)?;
        let condition_plan = self.compile_bytecode_linear_plan(condition)?;
        let handle = self.push_bytecode_control(BytecodeControlRecord::loop_record(
            BytecodeLoopKind::DoWhile,
        ))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let resumes_condition = *control.loop_state_mut(BytecodeLoopKind::DoWhile)?.0
                == BytecodeLoopPhase::Condition;
            if !resumes_condition {
                let resumes_body = *control.loop_state_mut(BytecodeLoopKind::DoWhile)?.0
                    == BytecodeLoopPhase::Body;
                if !resumes_body && let Err(error) = self.step() {
                    return self.finish_bytecode_control_result(handle, Err(error));
                }
                *control.loop_state_mut(BytecodeLoopKind::DoWhile)?.0 = BytecodeLoopPhase::Body;
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
                let (_, last) = control.loop_state_mut(BytecodeLoopKind::DoWhile)?;
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
                    completion @ (Completion::Suspended(_)
                    | Completion::GeneratorStart
                    | Completion::Yielded(_)
                    | Completion::YieldedIteratorResult(_)) => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Some(completion));
                    }
                }
            }
            *control.loop_state_mut(BytecodeLoopKind::DoWhile)?.0 = BytecodeLoopPhase::Condition;
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
                BytecodeCondition::Value(true) => {
                    *control.loop_state_mut(BytecodeLoopKind::DoWhile)?.0 =
                        BytecodeLoopPhase::Initialize;
                }
                BytecodeCondition::Value(false) => break,
                BytecodeCondition::Completion(completion) if completion.suspends_execution() => {
                    self.park_bytecode_control(handle, control)?;
                    return Ok(Some(completion));
                }
                BytecodeCondition::Completion(completion) => {
                    return self.finish_bytecode_control_result(handle, Ok(Some(completion)));
                }
            }
        }
        let (_, last) = control.loop_state_mut(BytecodeLoopKind::DoWhile)?;
        state.last = std::mem::replace(last, Value::Undefined);
        state.pc = next;
        self.finish_bytecode_control_result(handle, Ok(None))
    }
}
