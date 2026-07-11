use crate::{
    bytecode::{BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeForInTarget},
    error::{Error, Result},
    runtime::Context,
    runtime::abstract_operations::{IteratorSource, IteratorStep},
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::control::Completion,
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::control_continuation::{
    BytecodeControlHandle, BytecodeControlRecord, BytecodeControlStateSlot, BytecodeLoopPhase,
};
use super::state::{BytecodeState, bytecode_loop_completion};

impl Context {
    pub(super) fn eval_bytecode_for_of(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        target: &BytecodeForInTarget,
        object: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let iterable = self.eval_bytecode_expression(object)?;
        let source = self.get_iterator(iterable)?;
        let completion = match target {
            BytecodeForInTarget::Binding {
                name,
                kind: kind @ (DeclKind::Let | DeclKind::Const),
            } => self.eval_for_of_lexical_binding(name, *kind, source, body, labels)?,
            BytecodeForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => self.eval_for_of_assignment_loop(source, body, labels, |context, value| {
                context.assign_bytecode(name, value)
            })?,
            BytecodeForInTarget::PatternBinding { pattern, kind } => {
                self.eval_for_of_pattern_loop(source, pattern, *kind, body, labels)?
            }
            BytecodeForInTarget::Assignment(target) => {
                self.eval_for_of_assignment_loop(source, body, labels, |context, value| {
                    context.assign_bytecode_target(target, value)
                })?
            }
        };
        Ok(Self::store_or_return_completion(state, completion, next))
    }

    fn eval_for_of_lexical_binding(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        source: IteratorSource,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        self.ensure_extra_binding_capacity(0)?;
        let atom = self.intern_static_name_atom(name.name().name())?;
        let frame = self.compiled_local_binding_frame(name.name())?;
        let mutable = kind != DeclKind::Const;
        let mut scope = BindingScope::new();
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_of(source))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let resumes_body = *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Body;
            if !resumes_body {
                *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
                let step =
                    self.run_bytecode_iterator_action(handle, &mut control, |context, source| {
                        context.step()?;
                        context.iterator_step(source)
                    })?;
                let value = match step {
                    IteratorStep::Value(value) => value,
                    IteratorStep::Done => break,
                    IteratorStep::Abrupt(completion) => {
                        return Self::finish_for_of_control(self, handle, completion);
                    }
                };
                let binding_result = self.run_bytecode_control_action_result(&control, |context| {
                    let inserted = scope.insert_or_replace_at_optional_slot(
                        atom,
                        BindingCell::new(value, mutable, kind),
                        frame.map(crate::runtime::CompiledBindingFrame::slot),
                    )?;
                    if let Some(frame) = frame {
                        Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)?;
                    }
                    context.push_lexical_scope_with(scope)?;
                    if let Err(error) = context.remember_active_static_binding(name.name(), atom) {
                        if context.pop_lexical_scope()?.is_none() {
                            return Err(Error::runtime(
                                "bytecode for-of lexical scope disappeared after binding failure",
                            ));
                        }
                        return Err(error);
                    }
                    Ok(())
                });
                if let Err(error) = binding_result {
                    return self.close_for_of_error(handle, control, error);
                }
            }
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Body;
            let body_result = self.run_bytecode_control_segment_result(
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, body_state| context.eval_bytecode_block_with_state(body, body_state),
            );
            if body_result
                .as_ref()
                .is_ok_and(Completion::suspends_execution)
            {
                let completion = body_result?;
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            let removed_scope = self.pop_lexical_scope();
            scope = match removed_scope {
                Ok(Some(scope)) => scope,
                Ok(None) => {
                    return self.close_for_of_error(
                        handle,
                        control,
                        Error::runtime("bytecode for-of lexical scope disappeared"),
                    );
                }
                Err(error) => return self.close_for_of_error(handle, control, error),
            };
            let completion = match body_result {
                Ok(completion) => completion,
                Err(error) => {
                    return self.close_for_of_error(handle, control, error);
                }
            };
            let (_, last) = control.for_of_state_mut()?;
            if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                return self.close_for_of_completion(handle, control, completion);
            }
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
        }
        let (_, last) = control.for_of_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        Self::finish_for_of_control(self, handle, completion)
    }

    fn eval_for_of_assignment_loop(
        &mut self,
        source: IteratorSource,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
        mut assign: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<Completion> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_of(source))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let resumes_body = *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Body;
            if !resumes_body {
                *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
                let step =
                    self.run_bytecode_iterator_action(handle, &mut control, |context, source| {
                        context.step()?;
                        context.iterator_step(source)
                    })?;
                let value = match step {
                    IteratorStep::Value(value) => value,
                    IteratorStep::Done => break,
                    IteratorStep::Abrupt(completion) => {
                        return Self::finish_for_of_control(self, handle, completion);
                    }
                };
                let assign_result = self
                    .run_bytecode_control_action_result(&control, |context| assign(context, value));
                if let Err(error) = assign_result {
                    return self.close_for_of_error(handle, control, error);
                }
            }
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Body;
            let completion = self.run_bytecode_control_segment_result(
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, body_state| context.eval_bytecode_block_with_state(body, body_state),
            );
            let completion = match completion {
                Ok(completion) => completion,
                Err(error) => return self.close_for_of_error(handle, control, error),
            };
            if completion.suspends_execution() {
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            let (_, last) = control.for_of_state_mut()?;
            if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                return self.close_for_of_completion(handle, control, completion);
            }
            *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
        }
        let (_, last) = control.for_of_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        Self::finish_for_of_control(self, handle, completion)
    }

    pub(super) fn close_for_of_completion(
        &mut self,
        handle: BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        completion: Completion,
    ) -> Result<Completion> {
        let completion =
            self.run_bytecode_iterator_action(handle, &mut control, |context, source| {
                context.iterator_close(source, completion)
            })?;
        Self::finish_for_of_control(self, handle, completion)
    }

    pub(super) fn close_for_of_error(
        &mut self,
        handle: BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        error: Error,
    ) -> Result<Completion> {
        let result: Result<()> =
            self.run_bytecode_iterator_action(handle, &mut control, |context, source| {
                Err(context.iterator_close_on_error(source, error))
            });
        match result {
            Ok(()) => self.finish_bytecode_control_result(
                handle,
                Err(Error::runtime(
                    "iterator error close unexpectedly succeeded",
                )),
            ),
            Err(error) => Err(error),
        }
    }

    pub(super) fn finish_for_of_control(
        &mut self,
        handle: BytecodeControlHandle,
        completion: Completion,
    ) -> Result<Completion> {
        self.finish_bytecode_control_result(handle, Ok(completion))
    }
}
