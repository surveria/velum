use std::cmp::Ordering;

use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCompletion,
        BytecodeDynamicProperty, BytecodeInstruction, BytecodeNumericBinaryOp,
        BytecodeNumericCompareOp, BytecodeNumericEqualityOp,
    },
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::BindingCell,
        control::{Completion, runtime_exception_value},
        numeric::number_to_i32,
    },
    syntax::UpdateOp,
    value::Value,
};

use super::{
    array_add_loop::BytecodeForArrayAddFastPath,
    array_fill_loop::BytecodeForArrayFillFastPath,
    block_lexical_loop::BytecodeBlockLexicalLoopFastPath,
    loop_helpers::{fast_loop_compare, same_bytecode_binding},
    object_literal_loop::BytecodeObjectLiteralLoopFastPath,
    string_concat_loop::BytecodeForStringConcatLengthFastPath,
    switch_for_loop::BytecodeForSwitchFastPath,
    try_finally_loop::BytecodeForTryFinallyFastPath,
    update_expression_loop::BytecodeUpdateExpressionLoopFastPath,
};

#[derive(Debug)]
pub(super) struct BytecodeForLoopFastPath<'a> {
    pub(super) index: &'a BytecodeBinding,
    pub(super) index_cell: BindingCell,
    pub(super) compare: BytecodeNumericCompareOp,
    limit: BytecodeForLoopLimit<'a>,
    pub(super) update_step: f64,
    body: BytecodeForLoopBodyFastPath<'a>,
}

#[derive(Debug)]
enum BytecodeForLoopLimit<'a> {
    Literal(f64),
    ArrayLength {
        array: &'a BytecodeBinding,
        array_cell: BindingCell,
    },
}

#[derive(Debug)]
pub(super) enum BytecodeForLoopBodyFastPath<'a> {
    ArrayAdd(BytecodeForArrayAddFastPath<'a>),
    ArrayFill(BytecodeForArrayFillFastPath<'a>),
    MaskedArrayAdd(BytecodeForBodyFastPath<'a>),
    SwitchMaskedArrayAdd(BytecodeForSwitchFastPath<'a>),
    StringConcatLength(BytecodeForStringConcatLengthFastPath<'a>),
    UpdateExpression(BytecodeUpdateExpressionLoopFastPath<'a>),
    ObjectLiteral(BytecodeObjectLiteralLoopFastPath<'a>),
    BlockLexical(BytecodeBlockLexicalLoopFastPath<'a>),
    TryFinally(BytecodeForTryFinallyFastPath<'a>),
}

#[derive(Debug)]
pub(super) struct BytecodeForBodyFastPath<'a> {
    test: &'a BytecodeBinding,
    test_cell: BindingCell,
    test_mask: f64,
    test_mask_i32: i32,
    test_op: BytecodeNumericEqualityOp,
    test_right: f64,
    target: &'a BytecodeBinding,
    target_cell: BindingCell,
    array: &'a BytecodeBinding,
    array_cell: BindingCell,
    index: &'a BytecodeBinding,
    index_cell: BindingCell,
    index_mask: f64,
    index_mask_i32: i32,
    property: BytecodeDynamicProperty,
}

impl Context {
    pub(super) fn compile_bytecode_for_loop_fast_path<'a>(
        &mut self,
        condition: Option<&'a BytecodeBlock>,
        update: Option<&'a BytecodeBlock>,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForLoopFastPath<'a>>> {
        let (Some(condition), Some(update)) = (condition, update) else {
            return Ok(None);
        };
        let Some((condition_index, limit, compare)) =
            self.compile_bytecode_for_loop_condition_limit(condition)?
        else {
            return Ok(None);
        };
        let Some((update_read, update_write, update_step)) =
            Self::bytecode_for_loop_update_step(update)
        else {
            return Ok(None);
        };
        if !same_bytecode_binding(condition_index, update_read)
            || !same_bytecode_binding(condition_index, update_write)
        {
            return Ok(None);
        }
        let Some(index_cell) = self.get_binding_bytecode(condition_index)? else {
            return Ok(None);
        };
        let Some(body) = self.compile_bytecode_for_loop_body_fast_path(condition_index, body)?
        else {
            return Ok(None);
        };
        if self.builtin_value(condition_index.name().name())?.is_some() {
            return Ok(None);
        }
        Ok(Some(BytecodeForLoopFastPath {
            index: condition_index,
            index_cell,
            compare: *compare,
            limit,
            update_step,
            body,
        }))
    }

    fn bytecode_for_loop_update_step(
        update: &BytecodeBlock,
    ) -> Option<(&BytecodeBinding, &BytecodeBinding, f64)> {
        match update.instructions() {
            [
                BytecodeInstruction::LoadBinding(update_read),
                BytecodeInstruction::PushLiteral(Value::Number(update_step)),
                BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
                BytecodeInstruction::StoreBinding(update_write),
                BytecodeInstruction::StoreLast,
            ] => Some((update_read, update_write, *update_step)),
            [
                BytecodeInstruction::UpdateBinding {
                    name,
                    op: UpdateOp::Increment,
                    ..
                },
                BytecodeInstruction::StoreLast,
            ] => Some((name, name, 1.0)),
            [
                BytecodeInstruction::UpdateBinding {
                    name,
                    op: UpdateOp::Decrement,
                    ..
                },
                BytecodeInstruction::StoreLast,
            ] => Some((name, name, -1.0)),
            _ => None,
        }
    }

    fn compile_bytecode_for_loop_condition_limit<'a>(
        &self,
        condition: &'a BytecodeBlock,
    ) -> Result<
        Option<(
            &'a BytecodeBinding,
            BytecodeForLoopLimit<'a>,
            &'a BytecodeNumericCompareOp,
        )>,
    > {
        match condition.instructions() {
            [
                BytecodeInstruction::LoadBinding(condition_index),
                BytecodeInstruction::PushLiteral(Value::Number(limit)),
                BytecodeInstruction::NumberCompare(compare),
                BytecodeInstruction::StoreLast,
            ] => Ok(Some((
                condition_index,
                BytecodeForLoopLimit::Literal(*limit),
                compare,
            ))),
            [
                BytecodeInstruction::LoadBinding(condition_index),
                BytecodeInstruction::LoadBinding(array),
                BytecodeInstruction::ArrayLength { .. },
                BytecodeInstruction::NumberCompare(compare),
                BytecodeInstruction::StoreLast,
            ] => {
                let Some(array_cell) = self.get_binding_bytecode(array)? else {
                    return Ok(None);
                };
                Ok(Some((
                    condition_index,
                    BytecodeForLoopLimit::ArrayLength { array, array_cell },
                    compare,
                )))
            }
            _ => Ok(None),
        }
    }

    pub(super) fn bytecode_for_loop_fast_path_ready(
        &self,
        fast_path: &BytecodeForLoopFastPath<'_>,
    ) -> Result<bool> {
        if !matches!(
            fast_path.index_cell.value(fast_path.index.name())?,
            Value::Number(_)
        ) {
            return Ok(false);
        }
        match &fast_path.body {
            BytecodeForLoopBodyFastPath::ArrayAdd(body) => self
                .fast_loop_numeric_array_values_for_simple_add(body)
                .map(|values| values.is_some()),
            BytecodeForLoopBodyFastPath::ArrayFill(body) => {
                Self::bytecode_for_array_fill_fast_path_ready(body)
            }
            BytecodeForLoopBodyFastPath::MaskedArrayAdd(_) => Ok(true),
            BytecodeForLoopBodyFastPath::SwitchMaskedArrayAdd(body) => {
                self.bytecode_for_switch_fast_path_ready(body)
            }
            BytecodeForLoopBodyFastPath::StringConcatLength(body) => {
                Self::bytecode_for_string_concat_length_fast_path_ready(body)
            }
            BytecodeForLoopBodyFastPath::UpdateExpression(body) => {
                Self::update_expression_loop_fast_path_ready(body)
            }
            BytecodeForLoopBodyFastPath::ObjectLiteral(body) => {
                Self::object_literal_loop_fast_path_ready(body)
            }
            BytecodeForLoopBodyFastPath::BlockLexical(body) => {
                Self::block_lexical_loop_fast_path_ready(body)
            }
            BytecodeForLoopBodyFastPath::TryFinally(body) => {
                Self::try_finally_loop_fast_path_ready(body)
            }
        }
    }

    pub(super) fn eval_bytecode_for_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
    ) -> Result<Option<Completion>> {
        if let BytecodeForLoopBodyFastPath::ArrayAdd(body) = &fast_path.body
            && self.eval_bytecode_for_array_add_loop_fast_path(state, next, fast_path, body)?
        {
            return Ok(None);
        }
        if let BytecodeForLoopBodyFastPath::ArrayFill(body) = &fast_path.body
            && self.eval_bytecode_for_array_fill_loop_fast_path(state, next, fast_path, body)?
        {
            return Ok(None);
        }
        if let BytecodeForLoopBodyFastPath::SwitchMaskedArrayAdd(body) = &fast_path.body
            && self.eval_bytecode_for_switch_loop_fast_path(state, next, fast_path, body)?
        {
            return Ok(None);
        }
        if let BytecodeForLoopBodyFastPath::StringConcatLength(body) = &fast_path.body
            && self.eval_bytecode_for_string_concat_length_loop_fast_path(
                state, next, fast_path, body,
            )?
        {
            return Ok(None);
        }
        if let BytecodeForLoopBodyFastPath::UpdateExpression(body) = &fast_path.body
            && self.eval_update_expression_loop_fast_path(state, next, fast_path, body)?
        {
            return Ok(None);
        }
        if let BytecodeForLoopBodyFastPath::ObjectLiteral(body) = &fast_path.body
            && self.eval_object_literal_loop_fast_path(state, next, fast_path, body)?
        {
            return Ok(None);
        }
        if let BytecodeForLoopBodyFastPath::TryFinally(body) = &fast_path.body
            && self.eval_try_finally_loop_fast_path(state, next, fast_path, body)?
        {
            return Ok(None);
        }
        let mut last = Value::Undefined;
        let array_values = match &fast_path.body {
            BytecodeForLoopBodyFastPath::ArrayAdd(body) => {
                self.fast_loop_numeric_array_values_for_simple_add(body)?
            }
            BytecodeForLoopBodyFastPath::ArrayFill(_)
            | BytecodeForLoopBodyFastPath::BlockLexical(_)
            | BytecodeForLoopBodyFastPath::ObjectLiteral(_)
            | BytecodeForLoopBodyFastPath::StringConcatLength(_)
            | BytecodeForLoopBodyFastPath::TryFinally(_)
            | BytecodeForLoopBodyFastPath::UpdateExpression(_) => None,
            BytecodeForLoopBodyFastPath::MaskedArrayAdd(body) => {
                self.fast_loop_numeric_array_values(body)?
            }
            BytecodeForLoopBodyFastPath::SwitchMaskedArrayAdd(body) => {
                self.switch_loop_numeric_array_values(body)?
            }
        };
        loop {
            self.step()?;
            self.record_bytecode_linear_direct_run()?;
            if !self.fast_loop_condition(fast_path)? {
                break;
            }
            match self.eval_bytecode_for_loop_body_fast_path(fast_path, array_values.as_deref())? {
                Completion::Normal(value) => last = value,
                Completion::Continue(None) => {}
                completion @ (Completion::Break { .. }
                | Completion::Continue(Some(_))
                | Completion::Throw(_)
                | Completion::Return(_)) => return Ok(Some(completion)),
            }
            self.record_bytecode_linear_direct_run()?;
            self.fast_loop_update(fast_path)?;
        }
        state.last = last;
        state.pc = next;
        Ok(None)
    }

    fn eval_bytecode_for_switch_loop_fast_path(
        &mut self,
        state: &mut crate::runtime::bytecode::state::BytecodeState,
        next: BytecodeAddress,
        fast_path: &BytecodeForLoopFastPath<'_>,
        body: &BytecodeForSwitchFastPath<'_>,
    ) -> Result<bool> {
        let Some(array_values) = self.switch_loop_numeric_array_values(body)? else {
            return Ok(false);
        };
        let Value::Number(mut index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        let Value::Number(mut total) = body.target_cell.value(body.target.name())? else {
            return Ok(false);
        };
        let mut last = Value::Undefined;
        loop {
            self.record_bytecode_linear_direct_run()?;
            if !fast_loop_compare(fast_path.compare, index, self.fast_loop_limit(fast_path)?) {
                break;
            }
            self.step()?;
            if let Some(element) = Self::bytecode_for_switch_element(body, &array_values, index)? {
                total += element;
                last = self.checked_value(Value::Number(total))?;
            } else {
                last = Value::Undefined;
            }
            self.record_bytecode_linear_direct_run()?;
            index += fast_path.update_step;
        }
        let index_value = self.checked_value(Value::Number(index))?;
        fast_path
            .index_cell
            .assign(fast_path.index.name(), index_value)?;
        let total_value = self.checked_value(Value::Number(total))?;
        body.target_cell.assign(body.target.name(), total_value)?;
        state.last = last;
        state.pc = next;
        Ok(true)
    }

    fn compile_bytecode_for_loop_body_fast_path<'a>(
        &mut self,
        index: &'a BytecodeBinding,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForLoopBodyFastPath<'a>>> {
        if let Some(body) = self.compile_bytecode_for_array_add_fast_path(body)? {
            if !same_bytecode_binding(index, body.index) {
                return Ok(None);
            }
            return Ok(Some(BytecodeForLoopBodyFastPath::ArrayAdd(body)));
        }
        if let Some(body) = self.compile_bytecode_for_array_fill_fast_path(body)? {
            if !same_bytecode_binding(index, body.index) {
                return Ok(None);
            }
            return Ok(Some(BytecodeForLoopBodyFastPath::ArrayFill(body)));
        }
        if let Some(body) = self.compile_bytecode_for_body_fast_path(body)? {
            if !same_bytecode_binding(index, body.test) || !same_bytecode_binding(index, body.index)
            {
                return Ok(None);
            }
            if self.builtin_value(body.target.name().name())?.is_some() {
                return Ok(None);
            }
            return Ok(Some(BytecodeForLoopBodyFastPath::MaskedArrayAdd(body)));
        }
        if let Some(body) = self.compile_bytecode_for_switch_fast_path(index, body)? {
            if !same_bytecode_binding(index, body.discriminant) {
                return Ok(None);
            }
            if self.builtin_value(body.target.name().name())?.is_some() {
                return Ok(None);
            }
            return Ok(Some(BytecodeForLoopBodyFastPath::SwitchMaskedArrayAdd(
                body,
            )));
        }
        if let Some(body) = self.compile_bytecode_for_string_concat_length_fast_path(index, body)? {
            return Ok(Some(BytecodeForLoopBodyFastPath::StringConcatLength(body)));
        }
        if let Some(body) = self.compile_update_expression_loop_fast_path(index, body)? {
            return Ok(Some(BytecodeForLoopBodyFastPath::UpdateExpression(body)));
        }
        if let Some(body) = self.compile_object_literal_loop_fast_path(index, body)? {
            return Ok(Some(BytecodeForLoopBodyFastPath::ObjectLiteral(body)));
        }
        if let Some(body) = self.compile_try_finally_loop_fast_path(index, body)? {
            return Ok(Some(BytecodeForLoopBodyFastPath::TryFinally(body)));
        }
        self.compile_block_lexical_loop_fast_path(index, body)
            .map(|body| body.map(BytecodeForLoopBodyFastPath::BlockLexical))
    }

    pub(super) fn compile_bytecode_for_body_fast_path<'a>(
        &mut self,
        body: &'a BytecodeBlock,
    ) -> Result<Option<BytecodeForBodyFastPath<'a>>> {
        let [
            BytecodeInstruction::LoadBinding(test),
            BytecodeInstruction::PushLiteral(Value::Number(test_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::PushLiteral(Value::Number(test_right)),
            BytecodeInstruction::NumberEquality(test_op),
            BytecodeInstruction::JumpIfFalse(alternate),
            BytecodeInstruction::Complete(BytecodeCompletion::Continue(None)),
            BytecodeInstruction::Jump(end),
            BytecodeInstruction::PushUndefined,
            BytecodeInstruction::StoreLast,
            BytecodeInstruction::LoadBinding(target_read),
            BytecodeInstruction::LoadBinding(array),
            BytecodeInstruction::LoadBinding(index),
            BytecodeInstruction::PushLiteral(Value::Number(index_mask)),
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::BitAnd),
            BytecodeInstruction::ComputedMember { property },
            BytecodeInstruction::NumberBinary(BytecodeNumericBinaryOp::Add),
            BytecodeInstruction::StoreBinding(target_write),
            BytecodeInstruction::StoreLast,
        ] = body.instructions()
        else {
            return Ok(None);
        };
        if alternate.index() != 8 || end.index() != 10 {
            return Ok(None);
        }
        if !same_bytecode_binding(target_read, target_write) {
            return Ok(None);
        }
        let Ok(test_mask_i32) = number_to_i32(*test_mask, "&") else {
            return Ok(None);
        };
        let Ok(index_mask_i32) = number_to_i32(*index_mask, "&") else {
            return Ok(None);
        };
        let Some(test_cell) = self.get_binding_bytecode(test)? else {
            return Ok(None);
        };
        let Some(target_cell) = self.get_or_materialize_binding_bytecode(target_write)? else {
            return Ok(None);
        };
        if self.builtin_value(target_write.name().name())?.is_some() {
            return Ok(None);
        }
        let Some(array_cell) = self.get_binding_bytecode(array)? else {
            return Ok(None);
        };
        let Some(index_cell) = self.get_binding_bytecode(index)? else {
            return Ok(None);
        };
        Ok(Some(BytecodeForBodyFastPath {
            test,
            test_cell,
            test_mask: *test_mask,
            test_mask_i32,
            test_op: *test_op,
            test_right: *test_right,
            target: target_write,
            target_cell,
            array,
            array_cell,
            index,
            index_cell,
            index_mask: *index_mask,
            index_mask_i32,
            property: *property,
        }))
    }

    pub(super) fn eval_bytecode_for_body_fast_path(
        &mut self,
        fast_path: &BytecodeForBodyFastPath<'_>,
    ) -> Result<Completion> {
        self.eval_bytecode_for_body_fast_path_with_array(fast_path, None)
    }

    fn eval_bytecode_for_body_fast_path_with_array(
        &mut self,
        fast_path: &BytecodeForBodyFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Completion> {
        self.record_bytecode_linear_direct_run()?;
        self.step()?;
        self.eval_bytecode_for_body_fast_path_catching(fast_path, array_values)
    }

    fn eval_bytecode_for_loop_body_fast_path(
        &mut self,
        fast_path: &BytecodeForLoopFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Completion> {
        self.record_bytecode_linear_direct_run()?;
        match &fast_path.body {
            BytecodeForLoopBodyFastPath::ArrayAdd(body) => self
                .eval_bytecode_for_array_add_fast_path(body, array_values)
                .map(Completion::Normal),
            BytecodeForLoopBodyFastPath::ArrayFill(_) => Ok(Completion::Normal(Value::Undefined)),
            BytecodeForLoopBodyFastPath::MaskedArrayAdd(body) => {
                self.eval_bytecode_for_body_fast_path_catching(body, array_values)
            }
            BytecodeForLoopBodyFastPath::SwitchMaskedArrayAdd(body) => self
                .eval_bytecode_for_switch_fast_path(body, array_values)
                .map(Completion::Normal),
            BytecodeForLoopBodyFastPath::StringConcatLength(_)
            | BytecodeForLoopBodyFastPath::UpdateExpression(_)
            | BytecodeForLoopBodyFastPath::ObjectLiteral(_)
            | BytecodeForLoopBodyFastPath::TryFinally(_) => {
                Ok(Completion::Normal(Value::Undefined))
            }
            BytecodeForLoopBodyFastPath::BlockLexical(body) => {
                let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())?
                else {
                    return Ok(Completion::Normal(Value::Undefined));
                };
                Ok(Completion::Normal(
                    self.eval_block_lexical_loop_fast_path(body, index)?
                        .unwrap_or(Value::Undefined),
                ))
            }
        }
    }

    fn eval_bytecode_for_body_fast_path_catching(
        &mut self,
        fast_path: &BytecodeForBodyFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Completion> {
        match self.eval_bytecode_for_body_fast_path_inner(fast_path, array_values) {
            Ok(completion) => Ok(completion),
            Err(error) => {
                if let Some(value) = runtime_exception_value(&error) {
                    self.checked_value(value.clone())?;
                    return Ok(Completion::Throw(value));
                }
                Err(error)
            }
        }
    }

    fn eval_bytecode_for_body_fast_path_inner(
        &mut self,
        fast_path: &BytecodeForBodyFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Completion> {
        if let Some(test) = Self::fast_masked_number_equality(fast_path)? {
            if test {
                return Ok(Completion::Continue(None));
            }
            return self
                .eval_bytecode_for_body_fast_path_add(fast_path, array_values)
                .map(Completion::Normal);
        }
        let test_value = self.masked_binding_value(
            fast_path.test,
            &fast_path.test_cell,
            fast_path.test_mask,
            fast_path.test_mask_i32,
        )?;
        let test = self.eval_bytecode_number_equality(
            fast_path.test_op,
            &test_value,
            &Value::Number(fast_path.test_right),
        )?;
        if test.is_truthy() {
            return Ok(Completion::Continue(None));
        }
        self.eval_bytecode_for_body_fast_path_add(fast_path, array_values)
            .map(Completion::Normal)
    }

    fn eval_bytecode_for_body_fast_path_add(
        &mut self,
        fast_path: &BytecodeForBodyFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Value> {
        if let Some(value) = self.fast_numeric_snapshot_add(fast_path, array_values)? {
            return Ok(value);
        }
        if let Some(value) = self.fast_numeric_array_add(fast_path)? {
            return Ok(value);
        }
        let left = self.runtime_value(fast_path.target_cell.value(fast_path.target.name())?)?;
        let object = self.runtime_value(fast_path.array_cell.value(fast_path.array.name())?)?;
        let (property_value, direct_index) = self.masked_binding_index(
            fast_path.index,
            &fast_path.index_cell,
            fast_path.index_mask,
            fast_path.index_mask_i32,
        )?;
        let element = if let Some(value) = self.fast_array_index_value(&object, direct_index)? {
            value
        } else if let Some(value) =
            self.eval_dynamic_array_index_member(&object, &property_value)?
        {
            value
        } else {
            let key = self.dynamic_property_key(&property_value)?;
            self.get_cached_dynamic_property_value(&object, &key, fast_path.property.access())?
        };
        let value =
            self.eval_bytecode_number_binary(BytecodeNumericBinaryOp::Add, &left, &element)?;
        self.assign_fast_path_cell(fast_path.target, &fast_path.target_cell, value.clone())?;
        Ok(value)
    }

    fn fast_loop_numeric_array_values(
        &self,
        fast_path: &BytecodeForBodyFastPath<'_>,
    ) -> Result<Option<Vec<f64>>> {
        let Value::Object(id) = fast_path.array_cell.value(fast_path.array.name())? else {
            return Ok(None);
        };
        let Some(values) = self.objects.packed_array_values_if_array(id)? else {
            return Ok(None);
        };
        let mut numbers = Vec::with_capacity(values.len());
        for value in values {
            let Value::Number(number) = value else {
                return Ok(None);
            };
            numbers.push(number);
        }
        Ok(Some(numbers))
    }

    fn fast_loop_condition(&self, fast_path: &BytecodeForLoopFastPath<'_>) -> Result<bool> {
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(false);
        };
        Ok(fast_loop_compare(
            fast_path.compare,
            index,
            self.fast_loop_limit(fast_path)?,
        ))
    }

    pub(super) fn fast_loop_limit(&self, fast_path: &BytecodeForLoopFastPath<'_>) -> Result<f64> {
        match &fast_path.limit {
            BytecodeForLoopLimit::Literal(limit) => Ok(*limit),
            BytecodeForLoopLimit::ArrayLength { array, array_cell } => {
                let Value::Object(id) = array_cell.value(array.name())? else {
                    return Ok(0.0);
                };
                let Some(length) = self.objects.array_len_if_array(id)? else {
                    return Ok(0.0);
                };
                let length = u32::try_from(length)
                    .map_err(|_| Error::limit("array length exceeds loop fast path range"))?;
                Ok(f64::from(length))
            }
        }
    }

    fn fast_loop_update(&self, fast_path: &BytecodeForLoopFastPath<'_>) -> Result<()> {
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(());
        };
        let value = self.checked_value(Value::Number(index + fast_path.update_step))?;
        self.assign_fast_path_cell(fast_path.index, &fast_path.index_cell, value)
    }

    fn fast_masked_number_equality(
        fast_path: &BytecodeForBodyFastPath<'_>,
    ) -> Result<Option<bool>> {
        let Value::Number(number) = fast_path.test_cell.value(fast_path.test.name())? else {
            return Ok(None);
        };
        let masked = f64::from(number_to_i32(number, "&")? & fast_path.test_mask_i32);
        if masked.is_nan() || fast_path.test_right.is_nan() {
            return Ok(Some(matches!(
                fast_path.test_op,
                BytecodeNumericEqualityOp::NotEqual | BytecodeNumericEqualityOp::StrictNotEqual
            )));
        }
        let equal = matches!(
            masked.partial_cmp(&fast_path.test_right),
            Some(Ordering::Equal)
        );
        Ok(Some(match fast_path.test_op {
            BytecodeNumericEqualityOp::Equal | BytecodeNumericEqualityOp::StrictEqual => equal,
            BytecodeNumericEqualityOp::NotEqual | BytecodeNumericEqualityOp::StrictNotEqual => {
                !equal
            }
        }))
    }

    fn fast_numeric_array_add(
        &self,
        fast_path: &BytecodeForBodyFastPath<'_>,
    ) -> Result<Option<Value>> {
        let Value::Number(left) = fast_path.target_cell.value(fast_path.target.name())? else {
            return Ok(None);
        };
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(None);
        };
        let Value::Object(id) = fast_path.array_cell.value(fast_path.array.name())? else {
            return Ok(None);
        };
        let index = number_to_i32(index, "&")? & fast_path.index_mask_i32;
        let Ok(index) = usize::try_from(index) else {
            return Ok(None);
        };
        let Some(Value::Number(element)) = self.objects.array_index_value_if_array(id, index)?
        else {
            return Ok(None);
        };
        let value = self.checked_value(Value::Number(left + element))?;
        self.assign_fast_path_cell(fast_path.target, &fast_path.target_cell, value.clone())?;
        Ok(Some(value))
    }

    fn fast_numeric_snapshot_add(
        &self,
        fast_path: &BytecodeForBodyFastPath<'_>,
        array_values: Option<&[f64]>,
    ) -> Result<Option<Value>> {
        let Some(array_values) = array_values else {
            return Ok(None);
        };
        let Value::Number(left) = fast_path.target_cell.value(fast_path.target.name())? else {
            return Ok(None);
        };
        let Value::Number(index) = fast_path.index_cell.value(fast_path.index.name())? else {
            return Ok(None);
        };
        let index = number_to_i32(index, "&")? & fast_path.index_mask_i32;
        let Ok(index) = usize::try_from(index) else {
            return Ok(None);
        };
        let Some(element) = array_values.get(index).copied() else {
            return Ok(None);
        };
        let value = self.checked_value(Value::Number(left + element))?;
        self.assign_fast_path_cell(fast_path.target, &fast_path.target_cell, value.clone())?;
        Ok(Some(value))
    }

    fn masked_binding_value(
        &mut self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        mask: f64,
        mask_i32: i32,
    ) -> Result<Value> {
        let value = self.runtime_value(cell.value(binding.name())?)?;
        if let Value::Number(number) = value {
            let masked = number_to_i32(number, "&")? & mask_i32;
            return Ok(Value::Number(f64::from(masked)));
        }
        self.eval_bytecode_number_binary(
            BytecodeNumericBinaryOp::BitAnd,
            &value,
            &Value::Number(mask),
        )
    }

    fn masked_binding_index(
        &mut self,
        binding: &BytecodeBinding,
        cell: &BindingCell,
        mask: f64,
        mask_i32: i32,
    ) -> Result<(Value, Option<usize>)> {
        let value = self.runtime_value(cell.value(binding.name())?)?;
        if let Value::Number(number) = value {
            let index = number_to_i32(number, "&")? & mask_i32;
            return Ok((Value::Number(f64::from(index)), usize::try_from(index).ok()));
        }
        let property = self.eval_bytecode_number_binary(
            BytecodeNumericBinaryOp::BitAnd,
            &value,
            &Value::Number(mask),
        )?;
        Ok((property, None))
    }
}
