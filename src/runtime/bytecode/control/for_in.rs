use crate::{
    bytecode::{BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeForInTarget},
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::{BindingCell, BindingScope},
        control::Completion,
    },
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::super::{
    control_continuation::{
        BytecodeControlHandle, BytecodeControlRecord, BytecodeControlStateSlot, BytecodeLoopPhase,
    },
    state::{BytecodeState, bytecode_loop_completion},
};

impl Context {
    pub(super) fn eval_bytecode_for_in(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        target: &BytecodeForInTarget,
        object: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let keys = if self.resumes_bytecode_control() {
            Vec::new()
        } else {
            let object = match self.eval_bytecode_block(object)? {
                Completion::Normal(value) => value,
                completion @ (Completion::TailCall(_)
                | Completion::Throw(_)
                | Completion::Suspended(_)
                | Completion::GeneratorStart
                | Completion::Yielded(_)
                | Completion::YieldedIteratorResult(_)) => return Ok(Some(completion)),
                completion @ (Completion::Return(_)
                | Completion::ReturnDirect(_)
                | Completion::Break { .. }
                | Completion::Continue { .. }) => completion.into_result()?,
            };
            self.enumerable_keys(&object)?
        };
        let completion = match target {
            BytecodeForInTarget::Binding {
                name,
                kind:
                    kind @ (DeclKind::Let | DeclKind::Const | DeclKind::Using | DeclKind::AwaitUsing),
            } => self.eval_bytecode_for_in_lexical_binding(name, *kind, keys, body, labels)?,
            BytecodeForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => {
                self.eval_bytecode_for_in_assignment_loop(keys, body, labels, |context, key| {
                    let value = context.heap_string_value(&key)?;
                    context.assign_bytecode(name, value)
                })?
            }
            BytecodeForInTarget::PatternBinding { pattern, kind } => self
                .eval_for_in_pattern_loop(
                    keys,
                    pattern,
                    crate::bytecode::BytecodeDestructureMode::Declaration(*kind),
                    body,
                    labels,
                )?,
            BytecodeForInTarget::PatternAssignment(pattern) => self.eval_for_in_pattern_loop(
                keys,
                pattern,
                crate::bytecode::BytecodeDestructureMode::Assignment,
                body,
                labels,
            )?,
            BytecodeForInTarget::Assignment(target) => {
                self.eval_bytecode_for_in_assignment_loop(keys, body, labels, |context, key| {
                    let value = context.heap_string_value(&key)?;
                    context.assign_bytecode_target(target, value)
                })?
            }
        };
        Ok(Self::store_or_return_completion(state, completion, next))
    }

    fn eval_bytecode_for_in_lexical_binding(
        &mut self,
        name: &BytecodeBinding,
        kind: DeclKind,
        keys: Vec<String>,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
    ) -> Result<Completion> {
        self.ensure_extra_binding_capacity(0)?;
        let atom = self.intern_static_name_atom(name.name().name())?;
        let frame = self.compiled_local_binding_frame(name.name())?;
        let mutable = kind.is_mutable();
        let mut scope = BindingScope::new();
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_in(keys))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let resumes_body = control.for_in_state_mut()?.0 == &BytecodeLoopPhase::Body;
            if !resumes_body {
                let (phase, keys, _) = control.for_in_state_mut()?;
                *phase = BytecodeLoopPhase::Initialize;
                let Some(key) = keys.next() else {
                    break;
                };
                self.run_bytecode_control_action(handle, &control, |context| {
                    context.step()?;
                    let value = context.heap_string_value(&key)?;
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
                                "bytecode for-in lexical scope disappeared after binding failure",
                            ));
                        }
                        return Err(error);
                    }
                    Ok(())
                })?;
            }
            *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Body;
            let body_result = self.run_bytecode_control_segment(
                handle,
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
            let completion = match body_result {
                Ok(completion) => completion,
                Err(error) => {
                    removed_scope?;
                    return Err(error);
                }
            };
            scope = match removed_scope {
                Ok(Some(scope)) => scope,
                Ok(None) => {
                    return self.finish_bytecode_control_result(
                        handle,
                        Err(Error::runtime("bytecode for-in lexical scope disappeared")),
                    );
                }
                Err(error) => {
                    return self.finish_bytecode_control_result(handle, Err(error));
                }
            };
            let (_, _, last) = control.for_in_state_mut()?;
            if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                return Self::finish_for_in_control(self, handle, completion);
            }
            *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Initialize;
        }
        let (_, _, last) = control.for_in_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        Self::finish_for_in_control(self, handle, completion)
    }

    fn eval_bytecode_for_in_assignment_loop(
        &mut self,
        keys: Vec<String>,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
        mut assign: impl FnMut(&mut Self, String) -> Result<()>,
    ) -> Result<Completion> {
        let handle = self.push_bytecode_control(BytecodeControlRecord::for_in(keys))?;
        let mut control = self.checkout_bytecode_control(handle)?;
        loop {
            let resumes_body = control.for_in_state_mut()?.0 == &BytecodeLoopPhase::Body;
            if !resumes_body {
                let (phase, keys, _) = control.for_in_state_mut()?;
                *phase = BytecodeLoopPhase::Initialize;
                let Some(key) = keys.next() else {
                    break;
                };
                self.run_bytecode_control_action(handle, &control, |context| {
                    context.step()?;
                    assign(context, key)
                })?;
            }
            *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Body;
            let completion = self.run_bytecode_control_segment(
                handle,
                &mut control,
                BytecodeControlStateSlot::Body,
                |context, body_state| context.eval_bytecode_block_with_state(body, body_state),
            )?;
            if completion.suspends_execution() {
                self.park_bytecode_control(handle, control)?;
                return Ok(completion);
            }
            let (_, _, last) = control.for_in_state_mut()?;
            if let Some(completion) = bytecode_loop_completion(last, completion, labels) {
                return Self::finish_for_in_control(self, handle, completion);
            }
            *control.for_in_state_mut()?.0 = BytecodeLoopPhase::Initialize;
        }
        let (_, _, last) = control.for_in_state_mut()?;
        let completion = Completion::Normal(std::mem::replace(last, Value::Undefined));
        Self::finish_for_in_control(self, handle, completion)
    }

    fn finish_for_in_control(
        &mut self,
        handle: BytecodeControlHandle,
        completion: Completion,
    ) -> Result<Completion> {
        self.finish_bytecode_control_result(handle, Ok(completion))
    }
}
