use crate::{
    bytecode::{BytecodeAddress, BytecodeAssignmentTarget, BytecodeInstruction},
    error::{Error, Result},
    runtime::{
        Context,
        control::{Completion, Suspension},
    },
    syntax::StaticString,
    value::Value,
};

use super::state::BytecodeState;

impl Context {
    pub(super) fn eval_bytecode_stack_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::BeginPrivateEnvironment { names } => {
                self.begin_private_environment(state, names.clone())?;
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::PushLiteral(_)
            | BytecodeInstruction::PushString(_)
            | BytecodeInstruction::PushUndefined => {
                self.eval_bytecode_push_instruction(state, instruction, next)
            }
            BytecodeInstruction::TemplateConcat { part_count } => {
                self.eval_bytecode_template_concat(state, *part_count, next)
            }
            BytecodeInstruction::GetTemplateObject { site, quasis } => {
                self.eval_bytecode_get_template_object(state, *site, quasis, next)
            }
            BytecodeInstruction::StringConcat { .. }
            | BytecodeInstruction::StringConcatStatic { .. } => {
                self.eval_bytecode_string_concat_instruction(state, instruction, next)
            }
            BytecodeInstruction::CreateRegExp { pattern, flags } => {
                self.eval_bytecode_create_regexp(state, pattern, flags, next)
            }
            BytecodeInstruction::LoadThis => {
                state.stack.push(self.current_this()?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::ImportMeta => {
                state.stack.push(self.import_meta_value()?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::LoadNewTarget => {
                state.stack.push(self.current_new_target()?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::LoadBinding(binding) => {
                state.stack.push(self.eval_bytecode_identifier(binding)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StoreBinding(binding) => {
                let value = state.stack.pop()?;
                self.assign_bytecode_or_create_sloppy_global(binding, value.clone())?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::StoreAnnexBVar(name) => {
                let value = state.stack.pop()?;
                self.assign_annex_b_var(name, value.clone())?;
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::HoistLexicalBinding { name, kind } => {
                self.eval_bytecode_lexical_hoist(state, name, *kind, next)
            }
            BytecodeInstruction::ResolveBinding(_)
            | BytecodeInstruction::StoreResolvedBinding(_) => {
                self.eval_bytecode_resolved_binding_instruction(state, instruction, next)
            }
            BytecodeInstruction::DeclareBinding {
                name,
                kind,
                has_init,
            } => self.eval_bytecode_declare_binding(state, name, *kind, *has_init, next),
            BytecodeInstruction::StoreLast => {
                state.last = state.stack.pop()?;
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::Pop | BytecodeInstruction::Duplicate => {
                Self::eval_bytecode_stack_shape_instruction(state, instruction, next)
            }
            BytecodeInstruction::Unary(_) | BytecodeInstruction::NumberUnary(_) => {
                self.eval_bytecode_unary_instruction(state, instruction, next)
            }
            BytecodeInstruction::Await => self.eval_bytecode_await_instruction(state, next),
            BytecodeInstruction::GeneratorStart => {
                Ok(Some(Self::eval_generator_start(state, next)))
            }
            BytecodeInstruction::Yield { delegate } => {
                self.eval_bytecode_yield_instruction(state, next, *delegate)
            }
            BytecodeInstruction::NullishCoalescing { right } => {
                self.eval_bytecode_nullish_coalescing(state, right, next)
            }
            BytecodeInstruction::TypeOfBinding(_) | BytecodeInstruction::TypeOfValue => {
                self.eval_bytecode_typeof_instruction(state, instruction, next)
            }
            BytecodeInstruction::ToPropertyKey => {
                let value = state.stack.pop()?;
                state.stack.push(self.to_property_key_value(&value)?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode stack instruction mismatch")),
        }
    }

    fn eval_bytecode_stack_shape_instruction(
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::Pop => {
                state.stack.pop()?;
            }
            BytecodeInstruction::Duplicate => {
                let value = state.stack.peek()?.clone();
                state.stack.push(value);
            }
            _ => return Err(Error::runtime("bytecode stack shape instruction mismatch")),
        }
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_push_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = match instruction {
            BytecodeInstruction::PushLiteral(value) => self.runtime_value(value.clone())?,
            BytecodeInstruction::PushString(value) => self.static_string_value(value)?,
            BytecodeInstruction::PushUndefined => Value::Undefined,
            _ => return Err(Error::runtime("bytecode push instruction mismatch")),
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_lexical_hoist(
        &mut self,
        state: &mut BytecodeState,
        name: &crate::bytecode::BytecodeBinding,
        kind: crate::syntax::DeclKind,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        self.hoist_bytecode_lexical_binding(name, kind)?;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_typeof_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = match instruction {
            BytecodeInstruction::TypeOfBinding(binding) => {
                self.eval_bytecode_typeof_binding(binding)?
            }
            BytecodeInstruction::TypeOfValue => {
                let value = state.stack.pop()?;
                let type_name = self.semantic_type_name(&value)?;
                self.heap_string_value(type_name)?
            }
            _ => return Err(Error::runtime("typeof instruction mismatch")),
        };
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn eval_generator_start(state: &mut BytecodeState, next: BytecodeAddress) -> Completion {
        state.pc = next;
        state.mark_generator_start_suspended();
        Completion::Suspend(Suspension::GeneratorStart)
    }

    fn eval_bytecode_resolved_binding_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::ResolveBinding(binding) => {
                let target = BytecodeAssignmentTarget::Binding(binding.clone());
                let reference = self.eval_bytecode_assignment_reference(&target)?;
                state.push_resolved_binding(reference);
            }
            BytecodeInstruction::StoreResolvedBinding(_) => {
                let value = state.stack.pop()?;
                let reference = state.pop_resolved_binding()?;
                reference.set(self, value.clone())?;
                state.stack.push(value);
            }
            _ => return Err(Error::runtime("resolved binding instruction mismatch")),
        }
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_declare_binding(
        &mut self,
        state: &mut BytecodeState,
        name: &crate::bytecode::BytecodeBinding,
        kind: crate::syntax::DeclKind,
        has_init: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = if has_init {
            Some(state.stack.pop()?)
        } else {
            None
        };
        self.eval_bytecode_declaration(name, kind, value)?;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_await_instruction(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let value = state.stack.pop()?;
        match self.eval_bytecode_await(value)? {
            Completion::Normal(value) => {
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            Completion::Throw(value) => Ok(Some(Completion::Throw(value))),
            completion @ Completion::TailCall(_) => Ok(Some(completion)),
            completion @ Completion::Suspend(Suspension::Await(_)) => {
                state.pc = next;
                state.mark_await_suspended();
                Ok(Some(completion))
            }
            completion @ (Completion::Return(_)
            | Completion::ReturnDirect(_)
            | Completion::Break { .. }
            | Completion::Continue { .. }
            | Completion::Suspend(_)) => completion.into_result().map(|_| None),
        }
    }

    fn eval_bytecode_yield_instruction(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
        delegate: bool,
    ) -> Result<Option<Completion>> {
        if delegate {
            return self.eval_bytecode_yield_delegate_instruction(state, next);
        }
        let value = state.stack.pop()?;
        state.pc = next;
        state.mark_yield_suspended();
        Ok(Some(Completion::Suspend(Suspension::Yield(value))))
    }

    fn eval_bytecode_yield_delegate_instruction(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        use crate::runtime::abstract_operations::{YieldDelegateContinuation, YieldDelegateStep};

        let (mut continuation, resume) = if let Some(stored) = state.take_yield_delegate() {
            stored
        } else {
            let iterable = state.stack.pop()?;
            let asynchronous = self.current_function_is_async_generator()?;
            let (source, await_yielded_values) = if asynchronous {
                self.get_async_iterator(&iterable)?
            } else {
                (self.get_iterator(&iterable)?, false)
            };
            (
                YieldDelegateContinuation::new(source, asynchronous, await_yielded_values),
                None,
            )
        };
        match self.yield_delegate_step(&mut continuation, resume)? {
            YieldDelegateStep::Await(awaited) => {
                state.store_yield_delegate(continuation)?;
                state.mark_await_suspended();
                Ok(Some(Completion::Suspend(Suspension::Await(awaited))))
            }
            YieldDelegateStep::Yielded(value) => {
                state.store_yield_delegate(continuation)?;
                state.mark_yield_suspended();
                Ok(Some(Completion::Suspend(Suspension::Yield(value))))
            }
            YieldDelegateStep::DelegatedYield(delegated) => {
                state.store_yield_delegate(continuation)?;
                state.mark_yield_suspended();
                Ok(Some(Completion::Suspend(Suspension::DelegatedYield(
                    delegated,
                ))))
            }
            YieldDelegateStep::Complete(value) => {
                state.stack.push(value);
                state.pc = next;
                Ok(None)
            }
            YieldDelegateStep::Return(value) => Ok(Some(Completion::Return(value))),
            YieldDelegateStep::Abrupt(completion) => Ok(Some(completion)),
        }
    }

    fn current_function_is_async_generator(&self) -> Result<bool> {
        let function = self
            .activation_frames
            .iter()
            .rev()
            .find_map(crate::runtime::activation::ActivationFrame::function_id)
            .ok_or_else(|| Error::runtime("yield delegation function activation disappeared"))?;
        Ok(self.function(function)?.kind.is_async_generator())
    }

    fn eval_bytecode_nullish_coalescing(
        &mut self,
        state: &mut BytecodeState,
        right: &crate::bytecode::BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let left = state.stack.peek()?.clone();
        if matches!(left, Value::Undefined | Value::Null) {
            match self.eval_bytecode_block(right)? {
                Completion::Normal(value) => {
                    state.stack.pop()?;
                    state.stack.push(value);
                }
                completion @ (Completion::TailCall(_)
                | Completion::Throw(_)
                | Completion::Suspend(_)) => return Ok(Some(completion)),
                completion @ (Completion::Return(_)
                | Completion::ReturnDirect(_)
                | Completion::Break { .. }
                | Completion::Continue { .. }) => return completion.into_result().map(|_| None),
            }
        }
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_template_concat(
        &mut self,
        state: &mut BytecodeState,
        part_count: usize,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let text = self.template_concat_text(state.stack.tail(part_count)?)?;
        let value = self.heap_string_value(&text)?;
        state.stack.drop_tail(part_count)?;
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_create_regexp(
        &mut self,
        state: &mut BytecodeState,
        pattern: &StaticString,
        flags: &StaticString,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        state
            .stack
            .push(self.create_regexp_literal(pattern.as_str(), flags.as_str())?);
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_unary_instruction(
        &mut self,
        state: &mut BytecodeState,
        instruction: &BytecodeInstruction,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        match instruction {
            BytecodeInstruction::Unary(op) => {
                let value = state.stack.pop()?;
                state.stack.push(self.eval_bytecode_unary(*op, &value)?);
                state.pc = next;
                Ok(None)
            }
            BytecodeInstruction::NumberUnary(op) => {
                let value = state.stack.pop()?;
                state
                    .stack
                    .push(self.eval_bytecode_number_unary(*op, &value)?);
                state.pc = next;
                Ok(None)
            }
            _ => Err(Error::runtime("bytecode unary instruction mismatch")),
        }
    }
}
