use crate::{
    bytecode::{
        BytecodeAddress, BytecodeBinding, BytecodeBlock, BytecodeCatch, BytecodeCatchFastPath,
        BytecodeDirectThrow, BytecodeNumericBinaryOp,
    },
    error::{Error, Result},
    runtime::{
        Context,
        binding::scope::BindingCell,
        bytecode::{coercion::strict_equality, state::BytecodeState},
        control::Completion,
    },
    syntax::{DeclKind, StaticString},
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
        let mut completion = if let Some(direct_throw) = parts.body_direct_throw {
            self.eval_bytecode_direct_throw(direct_throw)?
        } else {
            self.eval_bytecode_maybe_scoped_block(parts.body, parts.body_scoped)?
        };
        if let (Completion::Throw(value), Some(catch)) = (&completion, parts.catch) {
            completion = self.eval_bytecode_catch(catch, value.clone())?;
        }
        if let Some(finally_body) = parts.finally_body {
            let finally_completion =
                self.eval_bytecode_maybe_scoped_block(finally_body, parts.finally_scoped)?;
            if !matches!(finally_completion, Completion::Normal(_)) {
                completion = finally_completion;
            }
        }
        Ok(Self::store_or_return_completion(state, completion, next))
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

    fn eval_bytecode_catch(&mut self, catch: &BytecodeCatch, value: Value) -> Result<Completion> {
        let Some(param) = catch.param.as_ref() else {
            return self.eval_bytecode_maybe_scoped_block(&catch.body, catch.body_scoped);
        };
        self.push_lexical_scope();
        let result = self.eval_bytecode_catch_scope(
            param,
            value,
            &catch.body,
            catch.body_scoped,
            catch.body_fast_path.as_ref(),
        );
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
        body_scoped: bool,
        fast_path: Option<&BytecodeCatchFastPath>,
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
        self.eval_bytecode_maybe_scoped_block(body, body_scoped)
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
