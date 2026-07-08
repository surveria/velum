use crate::{
    bytecode::{BytecodeAddress, BytecodeBinding, BytecodeDynamicProperty, BytecodeProperty},
    error::{Error, Result},
    runtime::Context,
    runtime::control::Completion,
    value::Value,
};

use super::for_of::ForOfStep;
use super::state::BytecodeState;

/// Result of expanding a mixed plain/spread value list: either the flattened
/// argument values or an abrupt completion raised by user iterator code.
enum SpreadExpansion {
    Values(Vec<Value>),
    Abrupt(Completion),
}

impl Context {
    pub(super) fn eval_bytecode_collect_spread_args(
        &mut self,
        state: &mut BytecodeState,
        spread_flags: &[bool],
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let values = state.stack.pop_many(spread_flags.len())?;
        match self.expand_spread_values(values, spread_flags)? {
            SpreadExpansion::Values(values) => {
                let packed = self.create_array_from_elements(values)?;
                state.stack.push(packed);
                state.pc = next;
                Ok(None)
            }
            SpreadExpansion::Abrupt(completion) => Ok(Some(completion)),
        }
    }

    pub(super) fn eval_bytecode_array_literal_spread(
        &mut self,
        state: &mut BytecodeState,
        spread_flags: &[bool],
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        self.eval_bytecode_collect_spread_args(state, spread_flags, next)
    }

    fn expand_spread_values(
        &mut self,
        values: Vec<Value>,
        spread_flags: &[bool],
    ) -> Result<SpreadExpansion> {
        if values.len() != spread_flags.len() {
            return Err(Error::runtime("spread argument arity mismatch"));
        }
        let mut expanded = Vec::with_capacity(values.len());
        for (value, spread) in values.into_iter().zip(spread_flags.iter().copied()) {
            if !spread {
                expanded.push(value);
                continue;
            }
            let mut source = self.for_of_source(value)?;
            loop {
                self.step()?;
                match self.for_of_step(&mut source)? {
                    ForOfStep::Value(value) => expanded.push(value),
                    ForOfStep::Done => break,
                    ForOfStep::Abrupt(completion) => {
                        return Ok(SpreadExpansion::Abrupt(completion));
                    }
                }
            }
        }
        Ok(SpreadExpansion::Values(expanded))
    }

    /// Reads the packed argument array produced by `CollectSpreadArgs` back
    /// into an owned argument vector.
    pub(super) fn spread_call_arguments(&mut self, packed: &Value) -> Result<Vec<Value>> {
        let Value::Object(id) = packed else {
            return Err(Error::runtime("spread argument pack is not an array"));
        };
        let Some(len) = self.objects.array_len_if_array(*id)? else {
            return Err(Error::runtime("spread argument pack is not an array"));
        };
        let mut args = Vec::with_capacity(len);
        for index in 0..len {
            args.push(self.get_property_value(packed, &index.to_string())?);
        }
        Ok(args)
    }

    pub(super) fn eval_bytecode_call_binding_spread(
        &mut self,
        state: &mut BytecodeState,
        callee: &BytecodeBinding,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let callee = self.eval_bytecode_identifier(callee)?;
        let completion = self.eval_call_completion(callee, &args, Value::Undefined)?;
        Ok(Self::push_spread_completion(state, completion, next))
    }

    pub(super) fn eval_bytecode_call_value_spread(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let callee = state.stack.pop()?;
        let completion = self.eval_call_completion(callee, &args, Value::Undefined)?;
        Ok(Self::push_spread_completion(state, completion, next))
    }

    pub(super) fn eval_bytecode_call_static_member_spread(
        &mut self,
        state: &mut BytecodeState,
        property: &BytecodeProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let this_value = state.stack.pop()?;
        let callee =
            self.get_static_property_value(&this_value, property.name(), property.access())?;
        let completion = self.eval_call_completion(callee, &args, this_value)?;
        Ok(Self::push_spread_completion(state, completion, next))
    }

    pub(super) fn eval_bytecode_call_computed_member_spread(
        &mut self,
        state: &mut BytecodeState,
        property: BytecodeDynamicProperty,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let key_value = state.stack.pop()?;
        let this_value = state.stack.pop()?;
        let key = self.dynamic_property_key(&key_value)?;
        let callee =
            self.get_cached_dynamic_property_value(&this_value, &key, property.access())?;
        let completion = self.eval_call_completion(callee, &args, this_value)?;
        Ok(Self::push_spread_completion(state, completion, next))
    }

    pub(super) fn eval_bytecode_construct_value_spread(
        &mut self,
        state: &mut BytecodeState,
        next: BytecodeAddress,
    ) -> Result<Option<Completion>> {
        let packed = state.stack.pop()?;
        let args = self.spread_call_arguments(&packed)?;
        let constructor = state.stack.pop()?;
        let value = self.eval_new_value(constructor, &args)?;
        state.stack.push(value);
        state.pc = next;
        Ok(None)
    }

    fn push_spread_completion(
        state: &mut BytecodeState,
        completion: Completion,
        next: BytecodeAddress,
    ) -> Option<Completion> {
        let Completion::Normal(value) = completion else {
            return Some(completion);
        };
        state.stack.push(value);
        state.pc = next;
        None
    }
}
