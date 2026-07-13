use crate::{
    bytecode::{BytecodeAddress, BytecodeBlock, BytecodeCatch, BytecodeDirectThrow},
    error::{Error, Result},
    runtime::{
        Context,
        bytecode::{
            control_continuation::{
                BytecodeControlRecord, BytecodeControlStateSlot, BytecodeTryPhase,
            },
            state::{BytecodeState, ScopeDisposalResumeBehavior},
        },
        control::{Completion, Suspension},
        resource_scope::ScopeDisposal,
    },
    syntax::DeclKind,
    value::Value,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct BytecodeTryParts<'a> {
    body: &'a BytecodeBlock,
    body_scoped: bool,
    body_direct_throw: Option<&'a BytecodeDirectThrow>,
    catch: Option<&'a BytecodeCatch>,
    finally_body: Option<&'a BytecodeBlock>,
    finally_scoped: bool,
}

impl<'a> BytecodeTryParts<'a> {
    pub(super) const fn new(
        body: &'a BytecodeBlock,
        body_scoped: bool,
        body_direct_throw: Option<&'a BytecodeDirectThrow>,
        catch: Option<&'a BytecodeCatch>,
        finally_body: Option<&'a BytecodeBlock>,
        finally_scoped: bool,
    ) -> Self {
        Self {
            body,
            body_scoped,
            body_direct_throw,
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
        if completion.suspends_execution() {
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
        if completion.suspends_execution() {
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
        if completion.suspends_execution() {
            return Ok(Some(completion));
        }
        if !matches!(completion, Completion::Normal(_)) {
            *control.try_state_mut()?.1 = Some(completion);
        }
        Ok(None)
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
        let resumes_destructure = state.has_destructure_continuation();
        let resumes_body = state.is_resuming() && !resumes_destructure;
        if !resumes_body && !resumes_destructure {
            self.push_lexical_scope()?;
            let hoist_result = catch.param_bindings.iter().try_for_each(|binding| {
                self.hoist_bytecode_lexical_binding(binding, DeclKind::Let)
            });
            if let Err(error) = hoist_result {
                let removed = self.pop_lexical_scope()?;
                if removed.is_none() {
                    return Err(Error::runtime("bytecode catch lexical scope disappeared"));
                }
                return Err(error);
            }
        }
        let result = if resumes_body {
            self.eval_bytecode_try_block_with_state(&catch.body, catch.body_scoped, state)
        } else {
            self.eval_bytecode_catch_scope(param, value, &catch.body, catch.body_scoped, state)
        };
        if result.as_ref().is_ok_and(Completion::suspends_execution) {
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
        param: &crate::bytecode::BytecodePattern,
        value: Value,
        body: &BytecodeBlock,
        body_scoped: bool,
        state: &mut BytecodeState,
    ) -> Result<Completion> {
        if !state.is_resuming() || state.has_destructure_continuation() {
            let value = if state.has_destructure_continuation() {
                None
            } else {
                Some(value)
            };
            match self.eval_resumable_destructure(
                state,
                param,
                crate::bytecode::BytecodeDestructureMode::Declaration(DeclKind::Let),
                value,
            )? {
                crate::runtime::bytecode::destructure::DestructureOutcome::Completed => {}
                crate::runtime::bytecode::destructure::DestructureOutcome::Abrupt(completion) => {
                    return Ok(completion);
                }
            }
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
        if state.has_scope_disposal() {
            return self.eval_bytecode_block_with_state(block, state);
        }
        let resumes = state.is_resuming();
        if !resumes {
            self.push_lexical_scope()?;
        }
        let result = self.eval_bytecode_block_with_state(block, state);
        if result.as_ref().is_ok_and(Completion::suspends_execution) {
            return result;
        }
        let Some(removed) = self.pop_lexical_scope()? else {
            return Err(Error::runtime("bytecode try lexical scope disappeared"));
        };
        match result {
            Ok(completion) => {
                match self.begin_dispose_binding_scope(removed, completion.clone())? {
                    ScopeDisposal::Complete(completion) => Ok(completion),
                    ScopeDisposal::Await(awaited) => {
                        state.store_scope_disposal(
                            completion,
                            ScopeDisposalResumeBehavior::Complete,
                        )?;
                        state.mark_await_suspended();
                        Ok(Completion::Suspend(Suspension::Await(awaited)))
                    }
                }
            }
            Err(error) => Err(error),
        }
    }
}
