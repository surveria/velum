mod array_add_loop;
mod array_fill_loop;
mod block_lexical_loop;
mod for_loop;
mod loop_helpers;
mod string_concat_loop;
mod switch_for_loop;
mod try_catch;
mod update_expression_loop;
mod while_loop;

use std::rc::Rc;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeForInTarget, BytecodeInstruction,
        BytecodeNumericBinaryOp, BytecodeSwitchCase,
    },
    error::{Error, Result},
    runtime::Context,
    runtime::binding::scope::{BindingCell, BindingScope},
    runtime::control::Completion,
    syntax::{DeclKind, StaticName},
    value::Value,
};

use super::{
    linear::BytecodeLinearPlan,
    state::{
        BytecodeState, bytecode_loop_completion, init_completion_to_result, loop_label_matches,
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
        self.push_lexical_scope();
        let result = self.eval_bytecode_block(block);
        let removed = self.pop_lexical_scope();
        if removed.is_none() {
            return Err(Error::runtime("bytecode lexical scope disappeared"));
        }
        result
    }

    pub(super) fn eval_bytecode_maybe_scoped_block(
        &mut self,
        block: &BytecodeBlock,
        scoped: bool,
    ) -> Result<Completion> {
        if scoped {
            return self.eval_bytecode_scoped_block(block);
        }
        self.eval_bytecode_block(block)
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
        let mut last = Value::Undefined;
        let condition_plan = self.compile_bytecode_linear_plan(condition)?;
        let body_plan = self.compile_bytecode_linear_plan(body)?;
        let mut condition_state = BytecodeState::new();
        let mut body_state = BytecodeState::new();
        loop {
            match self.eval_bytecode_condition_with_state(
                condition,
                condition_plan.as_ref(),
                &mut condition_state,
            )? {
                BytecodeCondition::Value(true) => {}
                BytecodeCondition::Value(false) => break,
                BytecodeCondition::Completion(completion) => return Ok(Some(completion)),
            }
            self.step()?;
            match self.eval_bytecode_block_with_linear_plan(
                body,
                body_plan.as_ref(),
                &mut body_state,
            )? {
                Completion::Normal(value) => last = value,
                Completion::Continue(None) => {}
                Completion::Continue(Some(target)) if loop_label_matches(labels, &target) => {}
                Completion::Break { label: None, value } => {
                    last = value;
                    break;
                }
                Completion::Break {
                    label: Some(target),
                    value,
                } if loop_label_matches(labels, &target) => {
                    last = value;
                    break;
                }
                completion @ (Completion::Break { .. }
                | Completion::Continue(Some(_))
                | Completion::Throw(_)
                | Completion::Return(_)) => {
                    return Ok(Some(completion));
                }
            }
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_do_while(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
        body: &BytecodeBlock,
        condition: &BytecodeBlock,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let mut last = Value::Undefined;
        let body_plan = self.compile_bytecode_linear_plan(body)?;
        let condition_plan = self.compile_bytecode_linear_plan(condition)?;
        let mut body_state = BytecodeState::new();
        let mut condition_state = BytecodeState::new();
        loop {
            self.step()?;
            match self.eval_bytecode_block_with_linear_plan(
                body,
                body_plan.as_ref(),
                &mut body_state,
            )? {
                Completion::Normal(value) => last = value,
                Completion::Continue(None) => {}
                Completion::Continue(Some(target)) if loop_label_matches(labels, &target) => {}
                Completion::Break { label: None, value } => {
                    last = value;
                    break;
                }
                Completion::Break {
                    label: Some(target),
                    value,
                } if loop_label_matches(labels, &target) => {
                    last = value;
                    break;
                }
                completion @ (Completion::Break { .. }
                | Completion::Continue(Some(_))
                | Completion::Throw(_)
                | Completion::Return(_)) => {
                    return Ok(Some(completion));
                }
            }
            match self.eval_bytecode_condition_with_state(
                condition,
                condition_plan.as_ref(),
                &mut condition_state,
            )? {
                BytecodeCondition::Value(true) => {}
                BytecodeCondition::Value(false) => break,
                BytecodeCondition::Completion(completion) => return Ok(Some(completion)),
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
            let mut init_state = BytecodeState::new();
            init_completion_to_result(self.eval_bytecode_block_with_state(init, &mut init_state)?)?;
        }
        if let Some(fast_path) =
            self.compile_bytecode_for_loop_fast_path(parts.condition, parts.update, parts.body)?
            && self.bytecode_for_loop_fast_path_ready(&fast_path)?
        {
            return self.eval_bytecode_for_loop_fast_path(state, next, &fast_path);
        }
        let mut last = Value::Undefined;
        let condition_plan = if let Some(condition) = parts.condition {
            self.compile_bytecode_linear_plan(condition)?
        } else {
            None
        };
        let body_fast_path = self.compile_bytecode_for_body_fast_path(parts.body)?;
        let body_plan = if body_fast_path.is_none() {
            self.compile_bytecode_linear_plan(parts.body)?
        } else {
            None
        };
        let update_plan = if let Some(update) = parts.update {
            self.compile_bytecode_linear_plan(update)?
        } else {
            None
        };
        let mut condition_state = BytecodeState::new();
        let mut body_state = BytecodeState::new();
        let mut update_state = BytecodeState::new();
        loop {
            if let Some(condition) = parts.condition {
                match self.eval_bytecode_condition_with_state(
                    condition,
                    condition_plan.as_ref(),
                    &mut condition_state,
                )? {
                    BytecodeCondition::Value(true) => {}
                    BytecodeCondition::Value(false) => break,
                    BytecodeCondition::Completion(completion) => return Ok(Some(completion)),
                }
            }
            self.step()?;
            let body_completion = if let Some(fast_path) = body_fast_path.as_ref() {
                self.eval_bytecode_for_body_fast_path(fast_path)?
            } else {
                self.eval_bytecode_block_with_linear_plan(
                    parts.body,
                    body_plan.as_ref(),
                    &mut body_state,
                )?
            };
            match body_completion {
                Completion::Normal(value) => last = value,
                Completion::Continue(None) => {}
                Completion::Continue(Some(target)) if loop_label_matches(parts.labels, &target) => {
                }
                Completion::Break { label: None, value } => {
                    last = value;
                    break;
                }
                Completion::Break {
                    label: Some(target),
                    value,
                } if loop_label_matches(parts.labels, &target) => {
                    last = value;
                    break;
                }
                completion @ (Completion::Break { .. }
                | Completion::Continue(Some(_))
                | Completion::Throw(_)
                | Completion::Return(_)) => {
                    return Ok(Some(completion));
                }
            }
            if let Some(update) = parts.update {
                self.eval_bytecode_expression_with_plan(
                    update,
                    update_plan.as_ref(),
                    &mut update_state,
                )?;
            }
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_condition_with_state(
        &mut self,
        condition: &BytecodeBlock,
        plan: Option<&BytecodeLinearPlan<'_>>,
        state: &mut BytecodeState,
    ) -> Result<BytecodeCondition> {
        if let Some(completion) = self.eval_bytecode_linear_direct_condition(condition, plan)? {
            return Ok(match completion {
                Completion::Normal(value) => BytecodeCondition::Value(value.is_truthy()),
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::Break { .. }
                | Completion::Continue(_)) => BytecodeCondition::Completion(completion),
            });
        }
        match self.eval_bytecode_block_with_linear_plan(condition, plan, state)? {
            Completion::Normal(value) => Ok(BytecodeCondition::Value(value.is_truthy())),
            completion @ (Completion::Throw(_)
            | Completion::Return(_)
            | Completion::Break { .. }
            | Completion::Continue(_)) => Ok(BytecodeCondition::Completion(completion)),
        }
    }

    fn eval_bytecode_for_in(
        &mut self,
        state: &mut BytecodeState,
        labels: Option<&[StaticName]>,
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
            BytecodeForInTarget::PatternBinding { pattern, kind } => {
                self.eval_for_in_pattern_loop(keys, pattern, *kind, body, labels)?
            }
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
            if let Some(completion) = bytecode_loop_completion(&mut last, completion?, labels) {
                return Ok(completion);
            }
        }
        Ok(Completion::Normal(last))
    }

    fn eval_bytecode_for_in_assignment_loop(
        &mut self,
        keys: Vec<String>,
        body: &BytecodeBlock,
        labels: Option<&[StaticName]>,
        mut assign: impl FnMut(&mut Self, String) -> Result<()>,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        for key in keys {
            self.step()?;
            assign(self, key)?;
            if let Some(completion) =
                bytecode_loop_completion(&mut last, self.eval_bytecode_block(body)?, labels)
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
        scoped: bool,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let Some(start) = self.bytecode_switch_start_index(discriminant, cases)? else {
            state.last = Value::Undefined;
            state.pc = next;
            return Ok(None);
        };
        let completion = if scoped {
            self.push_lexical_scope();
            let completion = self.eval_bytecode_switch_cases(cases, start);
            let removed = self.pop_lexical_scope();
            if removed.is_none() {
                return Err(Error::runtime("bytecode switch lexical scope disappeared"));
            }
            completion?
        } else {
            self.eval_bytecode_switch_cases(cases, start)?
        };
        Ok(Self::store_or_return_completion(state, completion, next))
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
            if bytecode_switch_number_equal(test, discriminant) {
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
        cases: &[BytecodeSwitchCase],
        start: usize,
    ) -> Result<Completion> {
        let mut last = Value::Undefined;
        for case in cases.iter().skip(start) {
            match self.eval_bytecode_block(&case.body)? {
                Completion::Normal(value) => last = value,
                Completion::Break { label: None, value } => {
                    return Ok(Completion::Normal(value));
                }
                completion @ (Completion::Throw(_)
                | Completion::Return(_)
                | Completion::Break { .. }
                | Completion::Continue(_)) => return Ok(completion),
            }
        }
        Ok(Completion::Normal(last))
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

const fn bytecode_switch_number_equal(left: f64, right: f64) -> bool {
    if left.is_nan() || right.is_nan() {
        return false;
    }
    let left_bits = left.to_bits();
    let right_bits = right.to_bits();
    let left_magnitude = left_bits & !F64_SIGN_BIT;
    let right_magnitude = right_bits & !F64_SIGN_BIT;
    if left_magnitude == 0 && right_magnitude == 0 {
        return true;
    }
    left_bits == right_bits
}

const F64_SIGN_BIT: u64 = 1_u64 << 63;
