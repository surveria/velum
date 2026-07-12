use crate::{
    bytecode::{BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeForInTarget},
    error::{Error, Result},
    runtime::Context,
    runtime::abstract_operations::{
        AsyncIteratorCloseStep, AsyncIteratorContinuation, AsyncIteratorStep, ForOfIterator,
        IteratorStep,
    },
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::control::{Completion, runtime_exception_value},
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::control_continuation::{
    BytecodeControlHandle, BytecodeControlRecord, BytecodeControlStateSlot, BytecodeLoopPhase,
};
use super::state::{BytecodeState, bytecode_loop_completion};

#[derive(Debug, Clone, Copy)]
pub(super) struct BytecodeForOfParts<'a> {
    labels: Option<&'a [StaticName]>,
    target: &'a BytecodeForInTarget,
    object: &'a BytecodeBlock,
    body: &'a BytecodeBlock,
    asynchronous: bool,
}

impl<'a> BytecodeForOfParts<'a> {
    pub(super) const fn new(
        labels: Option<&'a [StaticName]>,
        target: &'a BytecodeForInTarget,
        object: &'a BytecodeBlock,
        body: &'a BytecodeBlock,
        asynchronous: bool,
    ) -> Self {
        Self {
            labels,
            target,
            object,
            body,
            asynchronous,
        }
    }
}

impl Context {
    pub(super) fn eval_bytecode_for_of(
        &mut self,
        state: &mut BytecodeState,
        parts: BytecodeForOfParts<'_>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let BytecodeForOfParts {
            labels,
            target,
            object,
            body,
            asynchronous,
        } = parts;
        let iterator = if self.resumes_bytecode_control() {
            None
        } else {
            let iterable = self.eval_bytecode_expression(object)?;
            if asynchronous {
                let (source, await_yielded_values) = self.get_async_iterator(&iterable)?;
                Some(ForOfIterator::Asynchronous(AsyncIteratorContinuation::new(
                    source,
                    await_yielded_values,
                )))
            } else {
                Some(ForOfIterator::Synchronous(self.get_iterator(&iterable)?))
            }
        };
        let completion = match target {
            BytecodeForInTarget::Binding {
                name,
                kind:
                    kind @ (DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing),
            } => self.eval_for_of_lexical_binding(name, *kind, iterator, body, labels)?,
            BytecodeForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => self.eval_for_of_assignment_loop(iterator, body, labels, |context, value| {
                context.assign_bytecode(name, value)
            })?,
            BytecodeForInTarget::PatternBinding { pattern, kind } => self
                .eval_for_of_pattern_loop(
                    iterator,
                    pattern,
                    crate::bytecode::BytecodeDestructureMode::Declaration(*kind),
                    body,
                    labels,
                )?,
            BytecodeForInTarget::PatternAssignment(pattern) => self.eval_for_of_pattern_loop(
                iterator,
                pattern,
                crate::bytecode::BytecodeDestructureMode::Assignment,
                body,
                labels,
            )?,
            BytecodeForInTarget::Assignment(target) => {
                self.eval_for_of_assignment_loop(iterator, body, labels, |context, value| {
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
        iterator: Option<ForOfIterator>,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        self.ensure_extra_binding_capacity(0)?;
        let atom = self.intern_static_name_atom(name.name().name())?;
        let frame = self.compiled_local_binding_frame(name.name())?;
        let mutable = kind.is_mutable();
        let mut scope = BindingScope::new();
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_of(iterator))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        if *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Close {
            return self.resume_for_of_close(handle, control);
        }
        loop {
            let resumes_body = *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Body;
            if !resumes_body {
                *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
                let value = match self.next_for_of_value(handle, &mut control)? {
                    ForOfNext::Value(value) => value,
                    ForOfNext::Done => break,
                    ForOfNext::Abrupt(completion) => {
                        return Self::finish_for_of_control(self, handle, completion);
                    }
                    ForOfNext::Await(awaited) => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Completion::Suspended(awaited));
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
        iterator: Option<ForOfIterator>,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
        mut assign: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<Completion> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_of(iterator))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        if *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Close {
            return self.resume_for_of_close(handle, control);
        }
        loop {
            let resumes_body = *control.for_of_state_mut()?.0 == BytecodeLoopPhase::Body;
            if !resumes_body {
                *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Initialize;
                let value = match self.next_for_of_value(handle, &mut control)? {
                    ForOfNext::Value(value) => value,
                    ForOfNext::Done => break,
                    ForOfNext::Abrupt(completion) => {
                        return Self::finish_for_of_control(self, handle, completion);
                    }
                    ForOfNext::Await(awaited) => {
                        self.park_bytecode_control(handle, control)?;
                        return Ok(Completion::Suspended(awaited));
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
        let asynchronous = matches!(
            control.for_of_iterator_mut()?,
            ForOfIterator::Asynchronous(_)
        );
        if asynchronous {
            return self.drive_async_for_of_close(handle, control, Some(completion));
        }
        let completion =
            self.run_bytecode_for_of_action(handle, &mut control, |context, iterator| {
                context.iterator_close(iterator.source_mut(), completion)
            })?;
        Self::finish_for_of_control(self, handle, completion)
    }

    pub(super) fn close_for_of_error(
        &mut self,
        handle: BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        error: Error,
    ) -> Result<Completion> {
        let asynchronous = matches!(
            control.for_of_iterator_mut()?,
            ForOfIterator::Asynchronous(_)
        );
        if asynchronous {
            let Some(value) = runtime_exception_value(self, &error)? else {
                return self.finish_bytecode_control_result(handle, Err(error));
            };
            return self.close_for_of_completion(handle, control, Completion::Throw(value));
        }
        let result: Result<()> =
            self.run_bytecode_for_of_action(handle, &mut control, |context, iterator| {
                Err(context.iterator_close_on_error(iterator.source_mut(), error))
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

    pub(super) fn resume_for_of_close(
        &mut self,
        handle: BytecodeControlHandle,
        control: BytecodeControlRecord,
    ) -> Result<Completion> {
        self.drive_async_for_of_close(handle, control, None)
    }

    fn drive_async_for_of_close(
        &mut self,
        handle: BytecodeControlHandle,
        mut control: BytecodeControlRecord,
        completion: Option<Completion>,
    ) -> Result<Completion> {
        let resume = control.take_for_of_resume()?;
        let step = self.run_bytecode_for_of_action(handle, &mut control, |context, iterator| {
            let ForOfIterator::Asynchronous(continuation) = iterator else {
                return Err(Error::runtime(
                    "asynchronous iterator close lost its iterator mode",
                ));
            };
            context.async_iterator_close(continuation, completion, resume)
        })?;
        match step {
            AsyncIteratorCloseStep::Await(awaited) => {
                *control.for_of_state_mut()?.0 = BytecodeLoopPhase::Close;
                control.mark_for_of_awaiting()?;
                self.park_bytecode_control(handle, control)?;
                Ok(Completion::Suspended(awaited))
            }
            AsyncIteratorCloseStep::Complete(completion) => {
                Self::finish_for_of_control(self, handle, completion)
            }
        }
    }

    pub(super) fn next_for_of_value(
        &mut self,
        handle: BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
    ) -> Result<ForOfNext> {
        let resume = control.take_for_of_resume()?;
        let step = self.run_bytecode_for_of_action(handle, control, |context, iterator| {
            context.step()?;
            match iterator {
                ForOfIterator::Synchronous(source) => {
                    if resume.is_some() {
                        return Err(Error::runtime(
                            "synchronous iterator received an await completion",
                        ));
                    }
                    Ok(match context.iterator_step(source)? {
                        IteratorStep::Value(value) => ForOfNext::Value(value),
                        IteratorStep::Done => ForOfNext::Done,
                        IteratorStep::Abrupt(completion) => ForOfNext::Abrupt(completion),
                    })
                }
                ForOfIterator::Asynchronous(continuation) => {
                    Ok(match context.async_iterator_step(continuation, resume)? {
                        AsyncIteratorStep::Await(awaited) => ForOfNext::Await(awaited),
                        AsyncIteratorStep::Value(value) => ForOfNext::Value(value),
                        AsyncIteratorStep::Done => ForOfNext::Done,
                        AsyncIteratorStep::Abrupt(completion)
                        | AsyncIteratorStep::AbruptWithOpenIterator(completion) => {
                            ForOfNext::Abrupt(completion)
                        }
                    })
                }
            }
        })?;
        if matches!(step, ForOfNext::Await(_)) {
            control.mark_for_of_awaiting()?;
        }
        Ok(step)
    }
}

pub(super) enum ForOfNext {
    Await(crate::runtime::promise::PromiseId),
    Value(Value),
    Done,
    Abrupt(Completion),
}
