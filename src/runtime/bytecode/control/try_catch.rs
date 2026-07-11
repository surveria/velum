use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCatch, BytecodeCatchFastPath,
        BytecodeDirectThrow, BytecodeNumericBinaryOp, BytecodeTryFinallyFastPath,
    },
    error::{Error, Result},
    runtime::{
        Context,
        abstract_operations::{number_strict_equality, strict_equality},
        binding::scope::BindingCell,
        bytecode::{
            control_continuation::{
                BytecodeControlRecord, BytecodeControlStateSlot, BytecodeTryPhase,
            },
            state::BytecodeState,
        },
        control::Completion,
        numeric::number_to_i32,
    },
    syntax::{DeclKind, StaticString},
    value::Value,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct BytecodeTryParts<'a> {
    body: &'a BytecodeBlock,
    body_scoped: bool,
    body_direct_throw: Option<&'a BytecodeDirectThrow>,
    try_fast_path: Option<&'a BytecodeTryFinallyFastPath>,
    catch: Option<&'a BytecodeCatch>,
    finally_body: Option<&'a BytecodeBlock>,
    finally_scoped: bool,
}

impl<'a> BytecodeTryParts<'a> {
    pub(super) const fn new(
        body: &'a BytecodeBlock,
        body_scoped: bool,
        body_direct_throw: Option<&'a BytecodeDirectThrow>,
        try_fast_path: Option<&'a BytecodeTryFinallyFastPath>,
        catch: Option<&'a BytecodeCatch>,
        finally_body: Option<&'a BytecodeBlock>,
        finally_scoped: bool,
    ) -> Self {
        Self {
            body,
            body_scoped,
            body_direct_throw,
            try_fast_path,
            catch,
            finally_body,
            finally_scoped,
        }
    }
}

impl Context {
    pub(super) fn eval_bytecode_try(
        &mut self,
        state: &mut BytecodeState,
        parts: BytecodeTryParts<'_>,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let resumes = self.resumes_bytecode_control();
        if !resumes
            && let Some(fast_path) = parts.try_fast_path
            && let Some(completion) = self.eval_bytecode_try_finally_fast_path(fast_path)?
        {
            return Ok(Self::store_or_return_completion(state, completion, next));
        }
        let handle = self.push_bytecode_control(BytecodeControlRecord::try_record())?;
        let mut control = self.checkout_bytecode_control(handle)?;
        if let Some(completion) =
            self.eval_structured_try_body(handle, &mut control, parts, resumes)?
        {
            self.park_bytecode_control(handle, control)?;
            return Ok(Some(completion));
        }
        if let Some(completion) = self.eval_structured_catch(handle, &mut control, parts)? {
            self.park_bytecode_control(handle, control)?;
            return Ok(Some(completion));
        }
        if let Some(completion) = self.eval_structured_finally(handle, &mut control, parts)? {
            self.park_bytecode_control(handle, control)?;
            return Ok(Some(completion));
        }
        let completion = control
            .try_state_mut()?
            .1
            .take()
            .ok_or_else(|| Error::runtime("structured try completion disappeared"));
        let result =
            completion.map(|completion| Self::store_or_return_completion(state, completion, next));
        self.finish_bytecode_control_result(handle, result)
    }

    fn eval_structured_try_body(
        &mut self,
        handle: super::super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        parts: BytecodeTryParts<'_>,
        resumes: bool,
    ) -> Result<Option<Completion>> {
        if control.try_state_mut()?.0 != &BytecodeTryPhase::Body {
            return Ok(None);
        }
        let completion = if !resumes && let Some(direct_throw) = parts.body_direct_throw {
            match self.eval_bytecode_direct_throw(direct_throw) {
                Ok(completion) => completion,
                Err(error) => {
                    return self.finish_bytecode_control_result(handle, Err(error));
                }
            }
        } else {
            self.run_bytecode_control_segment(
                handle,
                control,
                BytecodeControlStateSlot::Body,
                |context, state| {
                    context.eval_bytecode_try_block_with_state(parts.body, parts.body_scoped, state)
                },
            )?
        };
        if matches!(completion, Completion::Suspended(_)) {
            return Ok(Some(completion));
        }
        let catches = matches!(completion, Completion::Throw(_)) && parts.catch.is_some();
        *control.try_state_mut()?.1 = Some(completion);
        *control.try_state_mut()?.0 = if catches {
            BytecodeTryPhase::Catch
        } else {
            BytecodeTryPhase::Finally
        };
        Ok(None)
    }

    fn eval_structured_catch(
        &mut self,
        handle: super::super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        parts: BytecodeTryParts<'_>,
    ) -> Result<Option<Completion>> {
        if control.try_state_mut()?.0 != &BytecodeTryPhase::Catch {
            return Ok(None);
        }
        let catch = parts
            .catch
            .ok_or_else(|| Error::runtime("structured catch definition disappeared"))?;
        let Some(Completion::Throw(value)) = control.try_state_mut()?.1.as_ref() else {
            return Err(Error::runtime("structured catch value disappeared"));
        };
        let value = value.clone();
        let completion = self.run_bytecode_control_segment(
            handle,
            control,
            BytecodeControlStateSlot::Catch,
            |context, state| context.eval_bytecode_catch(catch, value, state),
        )?;
        if matches!(completion, Completion::Suspended(_)) {
            return Ok(Some(completion));
        }
        *control.try_state_mut()?.1 = Some(completion);
        *control.try_state_mut()?.0 = BytecodeTryPhase::Finally;
        Ok(None)
    }

    fn eval_structured_finally(
        &mut self,
        handle: super::super::control_continuation::BytecodeControlHandle,
        control: &mut BytecodeControlRecord,
        parts: BytecodeTryParts<'_>,
    ) -> Result<Option<Completion>> {
        if control.try_state_mut()?.0 != &BytecodeTryPhase::Finally {
            return Ok(None);
        }
        let Some(finally_body) = parts.finally_body else {
            return Ok(None);
        };
        let completion = self.run_bytecode_control_segment(
            handle,
            control,
            BytecodeControlStateSlot::Finally,
            |context, state| {
                context.eval_bytecode_try_block_with_state(
                    finally_body,
                    parts.finally_scoped,
                    state,
                )
            },
        )?;
        if matches!(completion, Completion::Suspended(_)) {
            return Ok(Some(completion));
        }
        if !matches!(completion, Completion::Normal(_)) {
            *control.try_state_mut()?.1 = Some(completion);
        }
        Ok(None)
    }

    fn eval_bytecode_try_finally_fast_path(
        &mut self,
        fast_path: &BytecodeTryFinallyFastPath,
    ) -> Result<Option<Completion>> {
        let Some(index_cell) = self.get_binding_bytecode(&fast_path.index)? else {
            return Ok(None);
        };
        let Some(total_cell) = self.get_or_materialize_binding_bytecode(&fast_path.total)? else {
            return Ok(None);
        };
        let Value::Number(index) = index_cell.value(fast_path.index.name())? else {
            return Ok(None);
        };
        let Value::Number(mut total) = total_cell.value(fast_path.total.name())? else {
            return Ok(None);
        };
        let mask = number_to_i32(fast_path.index_mask, "try finally mask")?;
        let masked = f64::from(number_to_i32(index, "try finally index")? & mask);
        let branch_add = if number_strict_equality(masked, fast_path.throw_right) {
            fast_path.throw_value
        } else {
            fast_path.try_add
        };
        self.step()?;
        self.record_bytecode_linear_direct_run()?;
        total += branch_add;
        let branch_value = self.checked_value(Value::Number(total))?;
        total += fast_path.finally_add;
        let total_value = self.checked_value(Value::Number(total))?;
        self.assign_fast_path_cell(&fast_path.total, &total_cell, total_value)?;
        Ok(Some(Completion::Normal(branch_value)))
    }

    fn eval_bytecode_direct_throw(
        &mut self,
        direct_throw: &BytecodeDirectThrow,
    ) -> Result<Completion> {
        self.step()?;
        let value = match direct_throw {
            BytecodeDirectThrow::Literal(value) => self.runtime_value(value.clone())?,
            BytecodeDirectThrow::String(value) => self.static_string_value(value)?,
            BytecodeDirectThrow::Undefined => Value::Undefined,
        };
        self.step()?;
        Ok(Completion::Throw(value))
    }

    fn eval_bytecode_catch(
        &mut self,
        catch: &BytecodeCatch,
        value: Value,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        let Some(param) = catch.param.as_ref() else {
            return self.eval_bytecode_try_block_with_state(&catch.body, catch.body_scoped, state);
        };
        let resumes = state.is_resuming();
        if !resumes {
            self.push_lexical_scope()?;
        }
        let result = if resumes {
            self.eval_bytecode_try_block_with_state(&catch.body, catch.body_scoped, state)
        } else {
            self.eval_bytecode_catch_scope(
                param,
                value,
                &catch.body,
                catch.body_scoped,
                catch.body_fast_path.as_ref(),
                state,
            )
        };
        if result
            .as_ref()
            .is_ok_and(|completion| matches!(completion, Completion::Suspended(_)))
        {
            return result;
        }
        let removed = self.pop_lexical_scope()?;
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
        body_scoped: bool,
        fast_path: Option<&BytecodeCatchFastPath>,
        state: &mut BytecodeState,
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
        if let Some(fast_path) = fast_path {
            return self.eval_bytecode_catch_fast_path(fast_path);
        }
        self.eval_bytecode_try_block_with_state(body, body_scoped, state)
    }

    fn eval_bytecode_try_block_with_state(
        &mut self,
        block: &BytecodeBlock,
        scoped: bool,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        if !scoped {
            return self.eval_bytecode_block_with_state(block, state);
        }
        let resumes = state.is_resuming();
        if !resumes {
            self.push_lexical_scope()?;
        }
        let result = self.eval_bytecode_block_with_state(block, state);
        if result
            .as_ref()
            .is_ok_and(|completion| matches!(completion, Completion::Suspended(_)))
        {
            return result;
        }
        let removed = self.pop_lexical_scope()?;
        if removed.is_none() {
            return Err(Error::runtime("bytecode try lexical scope disappeared"));
        }
        result
    }

    fn eval_bytecode_catch_fast_path(
        &mut self,
        fast_path: &BytecodeCatchFastPath,
    ) -> Result<Completion> {
        match fast_path {
            BytecodeCatchFastPath::StrictStringIncrement {
                test,
                expected,
                target,
                addend,
            } => self.eval_bytecode_catch_string_increment(test, expected, target, *addend),
        }
    }

    fn eval_bytecode_catch_string_increment(
        &mut self,
        test: &BytecodeBinding,
        expected: &StaticString,
        target: &BytecodeBinding,
        addend: f64,
    ) -> Result<Completion> {
        self.step()?;
        let left = self.eval_bytecode_identifier(test)?;
        self.step()?;
        let right = self.static_string_value(expected)?;
        self.step()?;
        let matched = strict_equality(&left, &right);
        self.step()?;
        if !matched {
            self.step()?;
            self.step()?;
            return Ok(Completion::Normal(Value::Undefined));
        }

        self.step()?;
        let left = self.eval_bytecode_identifier(target)?;
        self.step()?;
        let right = Value::Number(addend);
        self.step()?;
        let value =
            self.eval_bytecode_number_binary(BytecodeNumericBinaryOp::Add, &left, &right)?;
        self.step()?;
        self.assign_bytecode_or_builtin(target, value.clone())?;
        self.step()?;
        self.step()?;
        Ok(Completion::Normal(value))
    }
}
