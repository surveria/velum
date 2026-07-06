use std::rc::Rc;

use crate::{
    ast::DeclKind,
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCatch, BytecodeForInTarget,
        BytecodeInstruction, BytecodeSwitchCase,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::completion::Completion,
    value::Value,
};

use super::state::{BytecodeState, bytecode_loop_completion, init_completion_to_result};

#[derive(Debug, Clone, Copy)]
struct BytecodeForParts<'a> {
    init: Option<&'a BytecodeBlock>,
    condition: Option<&'a BytecodeBlock>,
    update: Option<&'a BytecodeBlock>,
    body: &'a BytecodeBlock,
    scoped: bool,
}

impl<'a> BytecodeForParts<'a> {
    const fn new(
        init: Option<&'a BytecodeBlock>,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
        scoped: bool,
    ) -> Self {
        Self {
            init,
            condition,
            update,
            body,
            scoped,
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
            BytecodeInstruction::If {
                condition,
                consequent,
                alternate,
            } => {
                let condition = self.eval_bytecode_expression(condition)?;
                let completion = if condition.is_truthy() {
                    self.eval_bytecode_block(consequent)?
                } else if let Some(alternate) = alternate {
                    self.eval_bytecode_block(alternate)?
                } else {
                    Completion::Normal(Value::Undefined)
                };
                Ok(Self::store_or_return_completion(state, completion, next))
            }
            BytecodeInstruction::While { condition, body } => {
                self.eval_bytecode_while(state, condition, body, next)
            }
            BytecodeInstruction::For {
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
                    *scoped,
                );
                self.eval_bytecode_for(state, parts, next)
            }
            BytecodeInstruction::ForIn {
                target,
                object,
                body,
            } => self.eval_bytecode_for_in(state, target, object, body, next),
            BytecodeInstruction::Switch {
                discriminant,
                cases,
            } => self.eval_bytecode_switch(state, discriminant, cases, next),
            BytecodeInstruction::Try {
                body,
                catch,
                finally_body,
            } => self.eval_bytecode_try(state, body, catch.as_ref(), finally_body.as_ref(), next),
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
                state.pc = if value.is_truthy() { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::JumpIfFalseKeep(target) => {
                let value = state.stack.peek()?;
                state.pc = if value.is_truthy() { next } else { *target };
                Ok(None)
            }
            BytecodeInstruction::JumpIfTrueKeep(target) => {
                let value = state.stack.peek()?;
                state.pc = if value.is_truthy() { *target } else { next };
                Ok(None)
            }
            BytecodeInstruction::Complete(completion) => state.complete(*completion).map(Some),
            _ => Err(Error::runtime("bytecode control instruction mismatch")),
        }
    }

    fn store_or_return_completion(
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
        self.push_lexical_scope();
        let result = self.eval_bytecode_block(block);
        let removed = self.pop_lexical_scope();
        if removed.is_none() {
            return Err(Error::runtime("bytecode lexical scope disappeared"));
        }
        result
    }

    fn eval_bytecode_while(
        &mut self,
        state: &mut BytecodeState,
        condition: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let mut last = Value::Undefined;
        while self.eval_bytecode_expression(condition)?.is_truthy() {
            self.step()?;
            match self.eval_bytecode_block(body)? {
                Completion::Normal(value) => last = value,
                Completion::Continue => {}
                Completion::Break => {
                    state.last = last;
                    state.pc = next;
                    return Ok(None);
                }
                completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                    return Ok(Some(completion));
                }
            }
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_for(
        &mut self,
        state: &mut BytecodeState,
        parts: BytecodeForParts<'_>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        if parts.scoped {
            self.push_lexical_scope();
        }
        let result = self.eval_bytecode_for_loop(state, parts, next);
        if parts.scoped {
            let removed = self.pop_lexical_scope();
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
        if let Some(init) = parts.init {
            init_completion_to_result(self.eval_bytecode_block(init)?)?;
        }
        let mut last = Value::Undefined;
        loop {
            if let Some(condition) = parts.condition
                && !self.eval_bytecode_expression(condition)?.is_truthy()
            {
                break;
            }
            self.step()?;
            match self.eval_bytecode_block(parts.body)? {
                Completion::Normal(value) => last = value,
                Completion::Continue => {}
                Completion::Break => break,
                completion @ (Completion::Throw(_) | Completion::Return(_)) => {
                    return Ok(Some(completion));
                }
            }
            if let Some(update) = parts.update {
                self.eval_bytecode_expression(update)?;
            }
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_for_in(
        &mut self,
        state: &mut BytecodeState,
        target: &BytecodeForInTarget,
        object: &BytecodeBlock,
        body: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let object = self.eval_bytecode_expression(object)?;
        let keys = self.enumerable_keys(&object)?;
        let completion = match target {
            BytecodeForInTarget::Binding {
                name,
                kind: kind @ (DeclKind::Let | DeclKind::Const),
            } => self.eval_bytecode_for_in_lexical_binding(name, *kind, keys, body)?,
            BytecodeForInTarget::Binding {
                name,
                kind: DeclKind::Var,
            } => self.eval_bytecode_for_in_assignment_loop(keys, body, |context, key| {
                let value = context.heap_string_value(&key)?;
                context.assign_bytecode(name, value)
            })?,
            BytecodeForInTarget::Assignment(target) => {
                self.eval_bytecode_for_in_assignment_loop(keys, body, |context, key| {
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
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        self.ensure_extra_binding_capacity(0)?;
        let atom = self.intern_static_name_atom(name.name().name())?;
        let frame = self.compiled_local_binding_frame(name.name())?;
        let mutable = kind != DeclKind::Const;
        let mut scope = BindingScope::new();
        for key in keys {
            self.step()?;
            let value = self.heap_string_value(&key)?;
            let inserted = scope.insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, mutable, kind),
                frame.map(crate::runtime::CompiledBindingFrame::slot),
            )?;
            if let Some(frame) = frame {
                Self::mark_binding_scope_frame_slot(&mut scope, frame, inserted)?;
            }
            self.push_lexical_scope_with(scope);
            self.remember_active_static_binding(name.name(), atom)?;
            let completion = self.eval_bytecode_block(body);
            let Some(removed_scope) = self.pop_lexical_scope() else {
                return Err(Error::runtime("bytecode for-in lexical scope disappeared"));
            };
            scope = removed_scope;
            if let Some(completion) = bytecode_loop_completion(&mut last, completion?) {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_bytecode_for_in_assignment_loop(
        &mut self,
        keys: Vec<String>,
        body: &BytecodeBlock,
        mut assign: impl FnMut(&mut Self, String) -> Result<()>,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        for key in keys {
            self.step()?;
            assign(self, key)?;
            if let Some(completion) =
                bytecode_loop_completion(&mut last, self.eval_bytecode_block(body)?)
            {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_bytecode_switch(
        &mut self,
        state: &mut BytecodeState,
        discriminant: &BytecodeBlock,
        cases: &Rc<[BytecodeSwitchCase]>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let discriminant = self.eval_bytecode_expression(discriminant)?;
        let Some(start) = self.bytecode_switch_start_index(&discriminant, cases)? else {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(None);
        };
        self.push_lexical_scope();
        let completion = self.eval_bytecode_switch_cases(cases, start);
        let removed = self.pop_lexical_scope();
        if removed.is_none() {
            return Err(Error::runtime("bytecode switch lexical scope disappeared"));
        }
        Ok(Self::store_or_return_completion(state, completion?, next))
    }

    fn bytecode_switch_start_index(
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
            match self.eval_bytecode_block(&case.body)? {
                Completion::Normal(value) => last = value,
                Completion::Break => return Ok(Completion::Normal(last)),
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::Continue) => return Ok(completion),
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_bytecode_try(
        &mut self,
        state: &mut BytecodeState,
        body: &BytecodeBlock,
        catch: Option<&BytecodeCatch>,
        finally_body: Option<&BytecodeBlock>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let mut completion = self.eval_bytecode_scoped_block(body)?;
        if let (Completion::Throw(value), Some(catch)) = (&completion, catch) {
            completion = self.eval_bytecode_catch(catch, value.clone())?;
        }
        if let Some(finally_body) = finally_body {
            let finally_completion = self.eval_bytecode_scoped_block(finally_body)?;
            if !matches!(finally_completion, Completion::Normal(_)) {
                completion = finally_completion;
            }
        }
        Ok(Self::store_or_return_completion(state, completion, next))
    }

    fn eval_bytecode_catch(&mut self, catch: &BytecodeCatch, value: Value) -> Result<Completion> {
        let Some(param) = catch.param.as_ref() else {
            return self.eval_bytecode_scoped_block(&catch.body);
        };
        self.push_lexical_scope();
        let result = self.eval_bytecode_catch_scope(param, value, &catch.body);
        let removed = self.pop_lexical_scope();
        if removed.is_none() {
            return Err(Error::runtime("bytecode catch lexical scope disappeared"));
        }
        result
    }

    fn eval_bytecode_catch_scope(
        &mut self,
        param: &BytecodeBinding,
        value: Value,
        body: &BytecodeBlock,
    ) -> Result<Completion> {
        let atom = self.ensure_binding_capacity_static(param.name())?;
        let frame = self.compiled_local_binding_frame(param.name())?;
        let value = self.runtime_value(value)?;
        let inserted = self
            .active_bindings_mut()
            .insert_or_replace_at_optional_slot(
                atom,
                BindingCell::new(value, true, DeclKind::Let),
                frame.map(crate::runtime::CompiledBindingFrame::slot),
            )?;
        self.mark_active_binding_frame_slot(frame, inserted)?;
        self.remember_active_static_binding(param.name(), atom)?;
        self.eval_bytecode_scoped_block(body)
    }
}
